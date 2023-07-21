use log::{debug, error, info};
use std::path::Path;
use std::collections::HashMap;
use std::process::Command;
use git2::{self, Repository, ApplyOptions, Diff};
use git2::build::RepoBuilder;
use git2::ApplyLocation;

#[derive(Debug)]
struct PciDevice {
    bus: u8,
    device: u8,
    function: u8,
    class: String,
    vendor: String,
    device_name: String,
    svendor: String,
    sdevice: String,
    iommugroup: u8,
}

fn main() {

    // Init Logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace")).init();

    //Get PCI Devices
    debug!("Getting PCI Devices");
    let pci_devices = get_pci_devices();

    /*
    // Print pci devices
    for device in pci_devices {
        debug!("{:?}", device);
    }
    */

    // Get cpuinfo
    let cpu_info = procfs::CpuInfo::new().unwrap();
    let cpu_name = cpu_info.model_name(0).unwrap();
    let cpu_flags = cpu_info.flags(0).unwrap();
    let cpu_vendor = cpu_info.vendor_id(0).unwrap();
    let cpu_threads_per_core = 2; // Figure out how to get # of threads per core
    let cpu_threads = cpu_info.cpus.len();

    //let mut cpu_group_cores: Vec<String> = vec![];

    // Check if running as root
    if std::env::var("SUDO_USER").is_ok() == false { error!("This script requires root privileges!"); }

    // Get cpu cores that start cpu groups; Dont ask me
    debug!("Get CPU group cores");
    for cpu in cpu_info.cpus.iter() {
        // Ill do this later
    }
    
    // svm / vmx check
    debug!("SVM / VMX Check");
    if cpu_flags.contains(&"svm") {
        info!("CPU supports svm");
    } else if cpu_flags.contains(&"vmx") {
        info!("CPU supports vmx");
    } else {
        error!("This system doesn't support virtualization, please enable it then run this script again!")
    }
    
    // Check if system is installed in UEFI mode
    debug!("UEFI Check");
    if Path::new("/sys/firmware/efi").exists() {
        info!("System installed in UEFI mode");
    } else {
        error!("This system isn't installed in UEFI mode!");
    }

    // IOMMU check
    debug!("IOMMU Check");
    if Path::new("/sys/class/iommu/").read_dir().unwrap().any(|entry| entry.is_ok()) {
        info!("IOMMU is enabled");
    } else {
        error!("This system doesn't support IOMMU, please enable it then run this script again!");
    }

    let qemu_url = "https://github.com/qemu/qemu.git";
    let qemu_path = Path::new("./qemu/");
    let qemu_tag = "v8.0.3";
    let qemu_patch = std::fs::read(Path::new("./qemu.patch")).unwrap();

    match qemu_clone(qemu_url, qemu_path, qemu_tag, qemu_patch) {
        Ok(_) => println!("QEMU repository cloned successfully."),
        Err(e) => eprintln!("Failed to clone QEMU repository: {:?}", e),
    }
}

// Purely for testing
fn print_hashmap<K: std::fmt::Debug + std::fmt::Display, V: std::fmt::Debug + std::fmt::Display>(hashmap: &HashMap<K, V>) {
    for (key, value) in hashmap.iter() {
        println!("{}: {}", key, value);
    }
}


fn get_pci_devices() -> Vec<PciDevice> {
    let output = Command::new("lspci")
    .arg("-vmm")
    .output()
    .expect("Failed to run lspci");

    let output_str = std::str::from_utf8(&output.stdout).unwrap().to_string();

    let mut devices: Vec<PciDevice> = Vec::new();

    let device_blocks: Vec<&str> = output_str.trim_end_matches('\n').split("\n\n").collect();

    for device_block in device_blocks {
        let mut pci_device = PciDevice {
            bus: 0,
            device: 0,
            function: 0,
            class: String::new(),
            vendor: String::new(),
            device_name: String::new(),
            svendor: String::new(),
            sdevice: String::new(),
            iommugroup: 0,
        };

        for line in device_block.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0];
                let value = parts[1..].join(" ");

                match key {
                    "Slot:" => {
                        let bus_dev_func: Vec<&str> = value.split(|c| c == '.' || c == ':').collect();
                        if bus_dev_func.len() >= 3 {
                            pci_device.bus = u8::from_str_radix(bus_dev_func[0], 16).unwrap_or(0);
                            pci_device.device = u8::from_str_radix(bus_dev_func[1], 16).unwrap_or(0);
                            pci_device.function = u8::from_str_radix(bus_dev_func[2], 16).unwrap_or(0);
                        }
                    }
                    "Class:" => pci_device.class = value,
                    "Vendor:" => pci_device.vendor = value,
                    "Device:" => pci_device.device_name = value,
                    "SVendor:" => pci_device.svendor = value,
                    "SDevice:" => pci_device.sdevice = value,
                    "IOMMUGroup:" => pci_device.iommugroup = u8::from_str_radix(&value, 10).unwrap_or(0),
                    _ => {}
                }
            }
        }

        devices.push(pci_device);
    }
    return devices
}

fn qemu_clone(qemu_url: &str, qemu_path: &Path, qemu_tag: &str, qemu_patch: Vec<u8>) -> Result<(), git2::Error> {

    // Clone the repository
    let qemu_repo = RepoBuilder::new()
        .clone(qemu_url, qemu_path)?;
    // Checkout the specified tag
    let object = qemu_repo.revparse_single(qemu_tag)?;
    qemu_repo.checkout_tree(&object, None)?;
    qemu_repo.apply(&Diff::from_buffer(&qemu_patch)?, ApplyLocation::WorkDir, Some(&mut ApplyOptions::new()))?;

    Ok(())
}   