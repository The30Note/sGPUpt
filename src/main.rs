use log::{debug, error, info};
use std::path::Path;
use std::collections::HashMap;

fn main() {

    // Init Logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace")).init();

    // Check if running as root
    if std::env::var("SUDO_USER").is_ok() == false { error!("This script requires root privileges!"); }

    // Get cpuinfo
    let cpu_info = procfs::CpuInfo::new().unwrap();
    let cpu_name = cpu_info.model_name(0).unwrap();
    let cpu_flags = cpu_info.flags(0).unwrap();
    let cpu_vendor = cpu_info.vendor_id(0).unwrap();
    let mut cpu_core_groups: Vec<String>;

    // TODO: get cpu groups
    // for cpu in cpu_info.cpus.iter() {
    //     if cpu.iter().nth(0) == "0" {cpu_core_groups.push(cpu.iter().nth(1))}
    // }

    

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

// Purely for testing
fn print_hashmap<K: std::fmt::Debug + std::fmt::Display, V: std::fmt::Debug + std::fmt::Display>(hashmap: &HashMap<K, V>) {
    for (key, value) in hashmap.iter() {
        println!("{}: {}", key, value);
    }
}
