# image-builder

This tool creates an initial microcontroller image by combining a built bootloader and OS binaries into a single file.

Your workflow should be:

1. Build the operating system images you want to use (RODOS, ...)
2. Build the bootloader image (e.g. the MOVEloader)
3. Combine them into one image using this tool
4. Flash the resulting file onto a microcontroller

## Installation

Run the following in the `image-builder` subdirectory to make the `image-builder` binary available in `$PATH`:

    cargo install --path .

## Usage

To create an image from a built bootloader, use:

    image-builder write -b bootloader/target/thumbv7em-none-eabi/release/stm-bootloader -1 image.bin

Three slots (-1,-2,-3) are available for different OS images. Only the first is mandatory; the others default to it if not specified.

The command generates an `output_image.bin` file (or your `-o` specification), which can be read back with basic CRC verification:

    image-builder read -i output_image.bin

Now you can flash the image using st-flash:

    st-flash --reset --flash=0x200000 write output_image.bin 0x8000000

The `--flash` option forces it to accept that your chip actually has 2MB of flash. You should verify that this is the case.
