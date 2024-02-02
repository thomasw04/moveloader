# Bootloader Documentation

This document describes how to use the bootloader and its features.

## Development Goals

- Resilience (e.g. power loss while writing flash does not result in broken state)

## General Information

The bootloader boots images that are stored on the flash. It only boots images it could verify to be correct (a CRC from the metadata stored on flash equal to the CRC calculated from the image).

The bootloader defines the layout of images on the flash. The order of data on the flash storage is approximately like this (Note: to look up the *actual* layout, check [interface/src/lib.rs](interface/src/lib.rs)):

- At address `0`, the bootloader code starts. This is where the chip will start executing (both on power up or reset)
- Two pages of versioned metadata. If they differ, we can select the newest metadata with a valid CRC
- Three slots of size `0x7E000` (~504kB) for OS images.

To update an image, an OS (e.g. RODOS) must first write the image to the flash storage (at one of `SLOT_{1,2,3}_ADDR`). Afterwards, it must overwrite *one* of the metadata slots, including the CRC. Make sure the version integer is higher than before, otherwise your metadata might get overwritten during a fixup.

### System information

The bootloader expects the following system setup:

- It must run on a STM32L4R5 chip with exactly 2MB of flash storage (see 3.4.2 Option bytes programming in the reference manual)
- The flash should be in single-bank mode (this *MUST* be set up before the bootloader starts - set the option byte correctly while programming)
  - This means we have 256 pages of size `0x2000`

When starting, the bootloader copies a valid OS image (defined in one of the metadata blocks) to RAM, starting at address `0x20000000` (this is also the RAM start address of an STM32L4R5 chip).

### Building

Instructions on how to build the bootloader and flashable images are available in the [Build Guide](Image-Build.md).
