# sGPUpt
sGPUpt is designed for desktop VFIO users looking to passthrough their only GPU.

### [Functionality]
* Automated CPU pinning
* Validates IOMMU groups
* Installs required packages
* Compiles spoofed QEMU & EDK2 OVMF
* Creates hooks for single GPU passthrough
* Creates a VM based on your system specs with additional features and optimizations

**❗ Please check below to ensure compatibility with your hardware and distribution. ❗**

# Distro Support
| Distro            | Status |
| ----------------- | ------ |
| Arch Linux        |   ✔️   |
| Manjaro           |   ✔️   |
| Garuda            |   ✔️   |
| EndeavourOS       |   ✔️   |
| Fedora (36,37)    |   ✔️   |
| Nobara (37)       |   ✔️   |
| AlmaLinux (9.1)   |   ✔️   |
| Debian (any)      |   ❌   |
| Ubuntu (22.04+)   |   ✔️   |
| Kubuntu (22.04+)  |   ✔️   |
| Pop!_OS (22.04)   |   ✔️   |
| Linux Mint (21.1) |   ✔️   |

# Hardware Support
|   CPU + GPU     |  Status | Additional Information                                           |
| --------------- | ------- | ---------------------------------------------------------------- |
| AMD + Nvidia    |    ✔️   | -                                                                |
| AMD + AMD       |    ✔️   | XT models require [vendor-reset](https://github.com/gnif/vendor-reset) |
| AMD + Intel     |    ❌   | -                                                                |
| Intel + Nvidia  |    ✔️   | -                                                                |
| Intel + AMD     |    ✔️   | XT models require [vendor-reset](https://github.com/gnif/vendor-reset) |
| Intel + Intel   |    ❌   | -                                                                |

# Prerequisites
* Enable AMD-V/VT-x in your BIOS
* Disable CSM in your BIOS
* Disable ReBAR in your BIOS.

if you have an Intel CPU then add these parameters to grub:
>intel_iommu=on iommu=pt

If you use an Arch-based distro you may need to load the VFIO modules early
> [Arch Wiki](https://wiki.archlinux.org/title/PCI_passthrough_via_OVMF#mkinitcpio)

# Usage
```
curl -O https://raw.githubusercontent.com/lexi-src/sGPUpt/master/sGPUpt.sh
chmod +x sGPUpt.sh
sudo ./sGPUpt.sh
```

# Troubleshooting

### [Black screen/BIOS screen]
Known reasons this can occur:
* The hooks are failing because a program is still using the GPU.
* If you have an older card that doesn't have UEFI support you'll need a VBIOS.

### [Device is not isolated]
Your GPU is grouped with other devices which means you're unable to pass it unless you use a kernel with the ACS patch.

* ACS for Debian - Install [XanMod](https://xanmod.org/) kernel then add **pcie_acs_override=downstream,multifunction** to grub.
* ACS for Arch - **sudo pacman -S linux-zen** then add **pcie_acs_override=downstream,multifunction** to grub.

⚠️ **NOTE**: The ACS patch has inherent security risks and *can* damage hardware. ⚠️
