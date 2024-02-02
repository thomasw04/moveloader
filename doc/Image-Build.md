# Bootloader build instructions

Here are instructions on how to produce a final flashable image.

The idea is the following:

1. You build the containers that are necessary for building the bootloader and image-builder:

    ./scripts/setup_containers.sh

2. Now you can build the bootloader. The scripts mounts your current workspace, so changes to the files will be reflected in the build:

    ```sh
    ./scripts/build_bootloader.sh
    ```

    This will output the bootloader at `bootloader.bin`

3. Now we want to create an image using the image-builder. Let's say you have `osiris.bin` that you want to create an image from:

    ```sh
    ./scripts/build_image.sh write -b bootloader.bin -1 osiris.bin
    ```

    Here we specify only the first slot (the other slots will be filled with a copy of the exact same binary as well), but you can also add `-2` and `-3`. Note that this is done *in Docker* by mounting your current directory, so use only relative paths or adjust the script.

4. Now a file with exactly 2MB was generated at `output_image.bin`. This is the file we can flash onto our chip:

    ```sh
    st-flash --reset write output_image.bin 0x8000000
    ```

If `st-flash` doesn't believe you that your chip actually has 2MB of flash storage, you can add the `--flash=0x200000` flag to convince it.
