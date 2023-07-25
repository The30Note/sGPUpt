use log::{debug, error, info};
use std::io::prelude::*;
use std::path::Path;
use std::fs::File;
use std::fs;
use std::collections::HashMap;
use std::process::Command;
use git2::{self, Repository, ApplyOptions, Diff, ApplyLocation, RepositoryState};
use git2::build::RepoBuilder;
use std::os::unix::fs::symlink;

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

    /*
    // Get PCI Devices
    debug!("Getting PCI Devices");
    let pci_devices = get_pci_devices();
    */


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

    //Security Checks
    security_checks(cpu_flags);

    //Qemu Stuff
    let qemu_url = "https://github.com/qemu/qemu.git";
    let qemu_path = Path::new("./qemu/");
    let qemu_tag = "v8.0.3";
    let qemu_patch_diff = std::fs::read(Path::new("./qemu.patch")).unwrap();
    let qemu_name = "qemu";


    match repo_clone(qemu_name, qemu_url, qemu_path, qemu_tag,) {
        Ok(_) => println!("QEMU :> "),
        Err(e) => eprintln!("Failed to clone QEMU repository: {:?}", e),
    }

    match repo_patch(qemu_name, qemu_path, qemu_patch_diff) {
        Ok(()) => {
            // The patch was successfully applied
            println!("Qemu Patch applied successfully.");
        }
        Err(err) => {
            // Handling the error returned by the repo_patch function
            eprintln!("Error Patching Qemu: {}", err);
        }
    }

    //Edk2 Stuff
    let edk2_url = "https://github.com/tianocore/edk2.git";
    let edk2_path = Path::new("./edk2/");
    let edk2_tag = "edk2-stable202011";
    let edk2_patch_diff = std::fs::read(Path::new("./edk2.patch")).unwrap();
    let edk2_name = "edk2";

    match repo_clone(edk2_name, edk2_url, edk2_path, edk2_tag,) {
        Ok(_) => println!("Edk2 :> "),
        Err(e) => eprintln!("Failed to clone Edk2 repository: {:?}", e),
    }

    
    match repo_patch(edk2_name, edk2_path, edk2_patch_diff) {
        Ok(()) => {
            // The patch was successfully applied
            println!("Edk2 Patch applied successfully.");
        }
        Err(err) => {
            // Handling the error returned by the repo_patch function
            eprintln!("Error Patching Edk2: {}", err);
        }
    }
}

fn security_checks(cpu_flags: Vec<&str>) {

    // Check if running as root
    if std::env::var("SUDO_USER").is_err() { error!("This script requires root privileges!"); }
    
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
                    "IOMMUGroup:" => pci_device.iommugroup = value.parse::<u8>().unwrap_or(0),
                    _ => {}
                }
            }
        }

        devices.push(pci_device);
    }

    devices
}

fn repo_clone(repo_name: &str, repo_url: &str, repo_path: &Path, repo_tag: &str,) -> Result<(), Box<dyn std::error::Error>> {

    let mut repo_clone = true;
    
    if Path::new(repo_path).exists() {

        println!("Would you like to re-clone the {} GitHub repository? (y/N)", repo_name);
    
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                let response = input.trim().to_lowercase();
                if response == "yes" || response == "y" {
                    fs::remove_dir_all(repo_path);
                    repo_clone = true;
                } else {
                    repo_clone = false;
                    // let repo = Repository::open(repo_path)?; not needed
                }
            }
            Err(_) => error!("Error reading input. Exiting the program."), //is this needed?
        }        
    }

    if repo_clone {
        // Git 
        info!("Cloning {}, this may take a while.", repo_name);
        let repo = RepoBuilder::new().clone(repo_url, repo_path)?;

        // Git Checkout
        info!("Checking out {} tag for {}", repo_tag, repo_name);
        repo.checkout_tree(&repo.revparse_single(repo_tag)?, None)?;
    }

    Ok(())
}

// fn repo_patch(repo_name: &str, repo_path: &Path, repo_patch_diff: Vec<u8>) {
//     if Path::new(&format!("{}/{}_patch_marker", &repo_path.display(), repo_name)).exists() {
//         error!("{} has already been patched.", repo_name)
//     } else {
//         // Apply Patch File
//         debug!("Opening {} Repo", repo_name);
//         let repo = Repository::open(repo_path).unwrap();
//         debug!("Appling {} patch", repo_name);
//         repo.apply(&Diff::from_buffer(&repo_patch_diff).unwrap(), ApplyLocation::WorkDir, None).unwrap(); // TODO handle errors
//         match File::create(format!("{}/{}_patch_marker", &repo_path.display(), repo_name)) {
//             Ok(mut file) => {
//                 match file.write_all(b"") {
//                     Ok(_) => {
//                         info!("{}_patch_marker created", repo_name);
//                     },
//                     Err(e) => {
//                         todo!();
//                     }
//                 }
//             },
//             Err(_) => todo!()
//         }
//     }
// }

fn repo_patch(repo_name: &str, repo_path: &Path, repo_patch_diff: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(&format!("{}/{}_patch_marker", &repo_path.display(), repo_name)).exists() {
        return Err(format!("{} has already been patched.", repo_name).into());
    } else {
        // Apply Patch File
        debug!("Opening {} Repo", repo_name);
        let repo = Repository::open(repo_path)?;
        debug!("Applying {} patch", repo_name);
        repo.apply(&Diff::from_buffer(&repo_patch_diff)?, ApplyLocation::WorkDir, None)?;

        let patch_marker_path = format!("{}/{}_patch_marker", &repo_path.display(), repo_name);
        File::create(&patch_marker_path)?.write_all(b"")?;
        info!("{}_patch_marker created", repo_name);
    }

    Ok(())
}