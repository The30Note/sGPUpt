use log::{debug, error, info};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

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
    qemu_patch(&qemu_repo)?;
    qemu_compile(&qemu_repo, cpu_threads)?;

    //Edk2 Stuff

    let edk2_repo = Repo {
        url: "https://github.com/tianocore/edk2.git".into(),
        path: Path::new("./edk2/").into(),
        tag: "edk2-stable202011".into(),
        patch_diff: std::fs::read(Path::new("./edk2.patch")).unwrap(),
        name: "edk2".into(),
    };

    //Clone -> Patch -> Compile edk2
    repo_clone(&edk2_repo)?;
    edk2_patch(&edk2_repo)?;
    edk2_compile(&edk2_repo, cpu_threads)?;

    const PACKAGES: &[&str] = &[
    "qemu-kvm",
    "virt-manager",
    "virt-viewer",
    "libvirt-daemon-system",
    "libvirt-clients",
    "bridge-utils",
    "swtpm",
    "mesa-utils",
    "git",
    "ninja-build",
    "nasm",
    "iasl",
    "pkg-config",
    "libglib2.0-dev",
    "libpixman-1-dev",
    "meson",
    "build-essential",
    "uuid-dev",
    "python-is-python3",
    "libspice-protocol-dev",
    "libspice-server-dev",
    "flex",
    "bison",
    ];
    install_packages(PACKAGES)?;


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

fn qemu_patch(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(&format!("{}/{}_patch_marker", &repo.path.display(), repo.name)).exists() {
        return Err(format!("{} has already been patched.", repo.name).into());
    } else {
        

        //This is a shitty way of doing this but lazy
        replace_string_in_file(&repo.path, "block/bochs.c",
        ".format_name\t= \"bochs\",",
        ".format_name\t= \"woots\",")?;

        replace_string_in_file(&repo.path, "hw/i386/fw_cfg.c",
        "* DMA control register is located at FW_CFG_DMA_IO_BASE + 4\n */",
        "* DMA control register is located at FW_CFG_DMA_IO_BASE + 4")?;
        replace_string_in_file(&repo.path, "hw/i386/fw_cfg.c",
        "/* device present, functioning, decoding, not shown in UI */",
        "/* device present, functioning, decoding, not shown in UI ")?;
        replace_string_in_file(&repo.path, "hw/i386/fw_cfg.c",
        "aml_append(scope, dev);",
        "aml_append(scope, dev); */")?;
        
        replace_string_in_file(&repo.path, "hw/scsi/scsi-disk.c",
        "s->vendor = g_strdup(\"QEMU\");",
        "s->vendor = g_strdup(\"<WOOT>\");")?;
        replace_string_in_file(&repo.path, "hw/scsi/scsi-disk.c",
        "s->product = g_strdup(\"QEMU HARDDISK\");",
        "s->product = g_strdup(\"WDC WD20EARS\");")?;
        replace_string_in_file(&repo.path, "hw/scsi/scsi-disk.c",
        "s->product = g_strdup(\"QEMU CD-ROM\");",
        "s->product = g_strdup(\"TOSHIBA DVD-ROM\");")?;
        
        replace_string_in_file(&repo.path, "hw/smbios/smbios.c",
        "t->bios_characteristics_extension_bytes[1] = 0x14;",
        "t->bios_characteristics_extension_bytes[1] = 0x08;")?;

        replace_string_in_file(&repo.path, "hw/usb/dev-wacom.c",
        "QEMU PenPartner tablet",
        "WOOT PenPartner tablet")?;
        replace_string_in_file(&repo.path, "hw/usb/dev-wacom.c",
        "QEMU PenPartner Tablet",
        "WOOT PenPartner Tablet")?;
        replace_string_in_file(&repo.path, "hw/usb/dev-wacom.c",
        "[STR_MANUFACTURER]     = \"QEMU\",",
        "[STR_MANUFACTURER]     = \"WOOT\",")?;

        replace_string_in_file(&repo.path, "include/hw/acpi/aml-build.h",
        "#define ACPI_BUILD_APPNAME6 \"BOCHS \"\n#define ACPI_BUILD_APPNAME4 \"BXPC\"",
        "#define ACPI_BUILD_APPNAME6 \"ALASKA \"\n#define ACPI_BUILD_APPNAME4 \"RCKS\"")?;
        
        replace_string_in_file(&repo.path, "target/i386/kvm/kvm.c",
        "KVMKVMKVM\\0\\0\\0",
        "GenuineIntel")?;

        let patch_marker_path = format!("{}/{}_patch_marker", &repo.path.display(), repo.name);
        File::create(&patch_marker_path)?.write_all(b"")?;
        info!("{}_patch_marker created", repo.name);
    }

    Ok(())
}

fn edk2_patch(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(&format!("{}/{}_patch_marker", &repo.path.display(), repo.name)).exists() {
        return Err(format!("{} has already been patched.", repo.name).into());
    } else {
        
        //This is a shitty way of doing this but lazy
        replace_string_in_file(&repo.path, "MdeModulePkg/MdeModulePkg.dec",
        "gEfiMdeModulePkgTokenSpaceGuid.PcdAcpiDefaultOemTableId|0x20202020324B4445|UINT64|0x30001035",
        "gEfiMdeModulePkgTokenSpaceGuid.PcdAcpiDefaultOemTableId|0x20202020324B4544|UINT64|0x30001035")?;
        replace_string_in_file(&repo.path, "OvmfPkg/AcpiTables/Dsdt.asl",
        "DefinitionBlock (\"Dsdt.aml\", \"DSDT\", 1, \"INTEL \", \"OVMF    \", 4)",
        "DefinitionBlock (\"Dsdt.aml\", \"DSDT\", 1, \"INTEL \", \"WOOT    \", 4)")?;

        replace_string_in_file(&repo.path, "OvmfPkg/AcpiTables/Platform.h",
        "#define EFI_ACPI_OEM_ID           'O','V','M','F',' ',' '   // OEMID 6 bytes long\n#define EFI_ACPI_OEM_TABLE_ID     SIGNATURE_64('O','V','M','F','E','D','K','2') // OEM table id 8 bytes long\n#define EFI_ACPI_OEM_REVISION     0x20130221\n#define EFI_ACPI_CREATOR_ID       SIGNATURE_32('O','V','M','F')\n#define EFI_ACPI_CREATOR_REVISION 0x00000099",
        "#define EFI_ACPI_OEM_ID           'W','O','O','T',' ',' '   // OEMID 6 bytes long\n#define EFI_ACPI_OEM_TABLE_ID     SIGNATURE_64('W','O','O','T','N','O','O','B') // OEM table id 8 bytes long\n#define EFI_ACPI_OEM_REVISION     0x20201230\n#define EFI_ACPI_CREATOR_ID       SIGNATURE_32('N','O','O','B')\n#define EFI_ACPI_CREATOR_REVISION 0x00000098")?;

        replace_string_in_file(&repo.path, "OvmfPkg/AcpiTables/Ssdt.asl",
        "DefinitionBlock (\"Ssdt.aml\", \"SSDT\", 1, \"REDHAT \", \"OVMF    \", 4)",
        "DefinitionBlock (\"Ssdt.aml\", \"SSDT\", 1, \"<WOOT> \", \"WOOT    \", 4)")?;


        replace_string_in_file(&repo.path, "OvmfPkg/SmbiosPlatformDxe/SmbiosPlatformDxe.c",
        "  \"EFI Development Kit II / OVMF\\0\"     /* Vendor */ \n  \"0.0.0\\0\"                             /* BiosVersion */ \n  \"02/06/2015\\0\"                        /* BiosReleaseDate */",
        "  \"American Megatrends Inc. NOOP\\0\"     /* Vendor */ \n  \"1.6.0\\0\"                             /* BiosVersion */ \n  \"12/01/2020\\0\"                        /* BiosReleaseDate */")?;


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

//Replace string in file
fn replace_string_in_file(base_dir: &Box<std::path::Path>, sub_dir: &str, old_string: &str, new_string: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = std::fs::read_to_string(base_dir.join(sub_dir))?;
    file = file.replace(old_string, new_string);
    std::fs::write(base_dir.join(sub_dir), file)?;
    Ok(())
}

// Install packages with apt, force reinstall and add -y flag
fn install_packages(packages: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Installing packages");
    let mut command = Command::new("apt");
    command.arg("install");
    command.arg("--reinstall");
    command.arg("-y");
    for package in packages {
        command.arg(package);
    }
    command.spawn()?;
    Ok(())
}

fn configure_apparmor() -> std::io::Result<()> {
    if !std::path::Path::new("/etc/apparmor.d/disable/usr.sbin.libvirtd").exists() {
        Command::new("ln")
            .arg("-s")
            .arg("/etc/apparmor.d/usr.sbin.libvirtd")
            .arg("/etc/apparmor.d/disable/")
            .output()?;
        
        Command::new("apparmor_parser")
            .arg("-R")
            .arg("/etc/apparmor.d/usr.sbin.libvirtd")
            .output()?;
    }
    
    Ok(())
}

fn libvirt_group() {
    let output = Command::new("getent")
        .arg("group")
        .arg("libvirt")
        .output()
        .expect("Failed to execute command");

    if output.stdout.is_empty() {
        Command::new("groupadd")
            .arg("libvirt")
            .output()
            .expect("Failed to execute command");
    }
}