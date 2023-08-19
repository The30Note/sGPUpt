use log::{debug, error, info};
use std::env::current_dir;
use std::io::prelude::*;
use std::path::Path;
use std::fs::File;
use std::fs;
use std::process::Command;
use git2::{self, Repository, Diff, ApplyLocation};
use git2::build::RepoBuilder;

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

impl Default for PciDevice {
    fn default() -> Self {
        Self {
            bus: 0,
            device: 0,
            function: 0,
            class: String::new(),
            vendor: String::new(),
            device_name: String::new(),
            svendor: String::new(),
            sdevice: String::new(),
            iommugroup: 0,
        }
    }
}

struct Repo {
    url: Box<str>,
    path: Box<Path>,
    tag: Box<str> ,
    patch_diff: Vec<u8>,
    name: Box<str>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Init Logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace")).init();


    // Get PCI Devices
    debug!("Getting PCI Devices");
    let pci_devices: Vec<PciDevice> = get_pci_devices();



    /*
    // Print pci devices
    for device in pci_devices {
        debug!("{:?}", device);
    }
    */

    // Get cpuinfo
    let cpu_info: procfs::CpuInfo = procfs::CpuInfo::new()?;
    let cpu_name: &str = cpu_info.model_name(0).unwrap();
    let cpu_flags: Vec<&str> = cpu_info.flags(0).unwrap();
    let cpu_vendor: &str = cpu_info.vendor_id(0).unwrap();
    let cpu_threads_per_core: i32 = 2; // Figure out how to get # of threads per core
    let cpu_threads: usize = cpu_info.cpus.len();

    //Security Checks
    security_checks(cpu_flags)?;

    //Qemu Stuff
    let qemu_repo = Repo {
        url: "https://github.com/qemu/qemu.git".into(),
        path: Path::new("./qemu/").into(),
        tag: "v8.0.3".into(),
        patch_diff: std::fs::read(Path::new("./qemu.patch")).unwrap(),
        name: "qemu".into(),
    };

    //Clone -> Patch -> Compile Qemu
    repo_clone(&qemu_repo)?;
    repo_patch(&qemu_repo)?;
    qemu_compile(&qemu_repo, cpu_threads)?;

    //Edk2 Stuff

    let edk2_repo = Repo {
        url: "https://github.com/tianocore/edk2.git".into(),
        path: Path::new("./edk2/").into(),
        tag: "edk2-stable202011".into(),
        patch_diff: std::fs::read(Path::new("./edk2.patch")).unwrap(),
        name: "edk2".into(),
    };
    repo_clone(&edk2_repo)?;
    repo_patch(&edk2_repo)?;
    edk2_compile(&edk2_repo, cpu_threads)?;


    Ok(())
}

fn security_checks(cpu_flags: Vec<&str>) -> Result<(), Box<dyn std::error::Error>> {

    // Check if running as root
    if std::env::var("SUDO_USER").is_err() { 
        error!("This script requires root privileges!");
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
    if Path::new("/sys/class/iommu/").read_dir().unwrap().any(|entry: Result<fs::DirEntry, std::io::Error>| entry.is_ok()) {
        info!("IOMMU is enabled");
    } else {
        error!("This system doesn't support IOMMU, please enable it then run this script again!");
    }

    Ok(())
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
        let mut pci_device = PciDevice::default();

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

fn repo_clone(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {

    let mut clone = true;
    
    if Path::new(&*repo.path).exists() {

        println!("Would you like to re-clone the {} GitHub repository? (y/N)", repo.name);
    
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                let response = input.trim().to_lowercase();
                if response == "yes" || response == "y" {
                    fs::remove_dir_all(&repo.path)?;
                    clone = true;
                } else {
                    clone = false;
                    // let repo = Repository::open(repo_path)?; not needed
                }
            }
            Err(_) => error!("Error reading input. Exiting the program."), //is this needed?
        }        
    }

    if clone {
        // Git 
        info!("Cloning {}, this may take a while.", repo.name);
        let repo_clone = RepoBuilder::new().clone(&*repo.url, &*repo.path)?;

        // Git Checkout
        info!("Checking out {} tag for {}", repo.tag, repo.name);
        repo_clone.checkout_tree(&repo_clone.revparse_single(&*repo.tag)?, None)?;
    }

    Ok(())
}

fn repo_patch(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(&format!("{}/{}_patch_marker", &repo.path.display(), repo.name)).exists() {
        return Err(format!("{} has already been patched.", repo.name).into());
    } else {
        // Apply Patch File
        debug!("Opening {} Repo", repo.name);
        let repo_clone = Repository::open(&repo.path)?;
        debug!("Applying {} patch", repo.name);
        repo_clone.apply(&Diff::from_buffer(&repo.patch_diff)?, ApplyLocation::WorkDir, None)?;

        let patch_marker_path = format!("{}/{}_patch_marker", &repo.path.display(), repo.name);
        File::create(&patch_marker_path)?.write_all(b"")?;
        info!("{}_patch_marker created", repo.name);
    }

    Ok(())
}

fn qemu_compile(qemu: &Repo, cpu_threads: usize) -> Result<(), Box<dyn std::error::Error>> {

    // Configure Qemu for build
    Command::new("./configure")
        .current_dir(&qemu.path)
        .arg("--enable-spice")
        .arg("--disable-werror")
        .spawn()?;

    Command::new("make")
        .current_dir(&qemu.path)
        .arg(format!("-j{}", cpu_threads))
        .arg("-C")
        .arg("BaseTools")
        .spawn()?;

    Ok(())
}

fn edk2_compile(edk2: &Repo, cpu_threads: usize) -> Result<(), Box<dyn std::error::Error>> {

    // Configure Qemu for build
    Command::new("make")
        .current_dir(&edk2.path)
        .arg(format!("-j{}", cpu_threads))
        .arg("-C")
        .arg("BaseTools")
        .spawn()?;

    Command::new(". edksetup.sh")
        .current_dir(&edk2.path)
        .spawn()?;

    Command::new(". build.sh")
        .current_dir(edk2.path.join("/OvmfPkg"))
        .arg("-p").arg("./OvmfPkgX64.dsc")
        .arg("-a").arg("X64")
        .arg("-b").arg("RELEASE")
        .arg("-t").arg("GCC5")
        .spawn()?;
    Ok(())
}