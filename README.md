# Moveloader

Documentation for users of the bootloader is available in the [**User Guide**](doc/User-Guide.md). To build the bootloader or pack it into a flash image with OS images, check out the [Build Guide](doc/Image-Build.md).

To make sure you have configured a device correctly for the bootloader to work, you can run a [hardware test](doc/Hardware-Testing.md).

When writing code for the bootloader, you should use the following chip documentation for reference:

- [Datasheet](https://www.st.com/resource/en/datasheet/stm32l4r5vi.pdf)
- [Reference Manual](https://www.st.com/resource/en/reference_manual/rm0432-stm32l4-series-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)

To develop, you can either use the Dev Container configuration file in this repository (TL;DR: open VSCode, install the "Dev Containers" extension and then open this directory in VSCode, then "Reopen in Container"), or follow the setup guide below.

## Install and setup (Linux)

Disclaimer: I tested this on Ubuntu-22.04 LTS

First you will need the following dependencies:

```sh
sudo apt-get install build-essential cmake libusb-1.0 libusb-1.0-0-dev gdb-multiarch gcc-arm-none-eabi openocd
```

Note: On older Ubuntu (14.04/16.04) gdb-arm-none-eabi.

Then you need to create a symlink for the cross-debugger:
```sh
sudo ln -s /usr/bin/gdb-multiarch /usr/bin/arm-none-eabi-gdb
```

If you want to debug on QEMU you will also need this:

```sh
sudo apt-get install qemu-system-arm
```
On Arch: qemu-arch-extra)

Then download rustup:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

after rustup is successfully installed:

```sh
rustup target add thumbv7em-none-eabi
```

Now install the st-flash fork:

```sh
git clone --single-branch -b fix-stm32l4r5 https://github.com/bauen1/stlink.git /tmp/stlink
cd /tmp/stlink
```

```sh
make clean
make release
sudo make install
sudo ldconfig
```

```sh
sudo rm -r /tmp/stlink
```

Now you should be able to get the bootloader running:

**I strongly recommend using vscode (Could be alot of pain otherwise)**
1. If you made the reasonable choice to use vscode just install the extension (extensions.json).
2. Plug in your STM32L4R5
3. Run the Debug (OpenOCD) configuration.
4. Programm should be compiled + flashed + debugger started -> Profit.


## Useful commands

Use:
```
cargo size --bin stm-bootloader -- -A
```
To get a detailed view of your binary size (use --release for release)

Or:
```
cargo readobj --bin stm-bootloader -- --file-headers
```
For inspecting the file headers of the generated binary (use --release for release)
