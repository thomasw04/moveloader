use interface::crc::calc_crc32;
use interface::{
    ImageMetadata, Metadata, FLASH_SIZE, METADATA_1_ADDR, METADATA_2_ADDR, NUMBER_OF_IMAGES,
    SLOT_1_ADDR, SLOT_2_ADDR, SLOT_3_ADDR, SLOT_SIZE,
};
use std::io::{Error, ErrorKind};

use crate::byte_utils::{set_buf_from_to, struct_to_bytes};
use crate::verification;

pub fn calc_crc(data: &[u8]) -> u32 {
    calc_crc32(data.as_ptr(), data.len())
}

// Output a file with the following layout (end is exclusive):
// These values are exemplary and are defined in the interface crate.
// 0x0 - 0x1000: Binary blob of the bootloader
// 0x1000 - 0x2000: Metadata 1 (padded until end)
// 0x2000 - 0x3000: Metadata 2 (padded until end)
// 3x image slots
pub fn generate_buffer(
    bootloader_bin: &Vec<u8>, image_1_bin: &Vec<u8>, image_2_bin: &Vec<u8>, image_3_bin: &Vec<u8>,
) -> Result<Vec<u8>, Error> {
    let mut data = vec![0u8; FLASH_SIZE as usize];

    if let Err(e) = verification::is_likely_valid_binary_buf(bootloader_bin) {
        return Err(Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Bootloader image is not a valid binary: {}", e),
        ));
    }

    // Write bootloader binary at beginning
    set_buf_from_to(&mut data, 0, METADATA_1_ADDR, bootloader_bin).map_err(|_| {
        Error::new(ErrorKind::Other, "Failed to write bootloader binary to output buffer")
    })?;

    // Now generate metadata and write images to their respective slots
    let image_data: Vec<(&Vec<u8>, u32)> =
        vec![(&image_1_bin, SLOT_1_ADDR), (&image_2_bin, SLOT_2_ADDR), (&image_3_bin, SLOT_3_ADDR)];

    let mut image_metadata: Vec<ImageMetadata> = vec![];

    for (idx, &(image, addr)) in image_data.iter().enumerate() {
        if image.len() > SLOT_SIZE as usize {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Image size is too large: {} > {}", image.len(), SLOT_SIZE as usize),
            ));
        }

        if let Err(e) = verification::is_likely_valid_binary_buf(image) {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Image {} (start={:#x}) is not a valid binary: {}", idx, addr, e),
            ));
        }

        if let Err(e) = verification::is_likely_valid_os_image_buf(image) {
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Image {} (start={:#x}) is likely an invalid OS image: {}", idx, addr, e),
            ));
        }

        image_metadata.push(ImageMetadata {
            version: 1,
            crc: calc_crc(image),
            boot_counter: 0,
            length: image.len() as u32,
        });

        set_buf_from_to(&mut data, addr, addr + image.len() as u32, image).map_err(|_| {
            Error::new(
                ErrorKind::Other,
                format!("Failed to write image to output buffer at address {:#x}", addr),
            )
        })?;
    }

    let mut images: [ImageMetadata; NUMBER_OF_IMAGES] = Default::default();
    images.copy_from_slice(&image_metadata[..]);

    let mut metadata = Metadata { version: 1, bootcounter: 0, preferred_image: 0, images, crc: 0 };
    metadata.set_crc();

    let metadata_bytes = struct_to_bytes(&metadata);
    set_buf_from_to(&mut data, METADATA_1_ADDR, METADATA_2_ADDR, &metadata_bytes)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to write metadata to output buffer"))?;

    set_buf_from_to(&mut data, METADATA_2_ADDR, SLOT_1_ADDR, &metadata_bytes)
        .map_err(|_| Error::new(ErrorKind::Other, "Failed to write metadata to output buffer"))?;

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use interface::{METADATA_IMAGE_DATA_OFFSET, SLOT_SIZE};
    use std::mem;

    fn generate_bootloader_binary(len: usize) -> Vec<u8> {
        let mut real_bootloader = include_bytes!("../testdata/bootloader.bin").to_vec();
        if len < real_bootloader.len() {
            real_bootloader.truncate(len);
        } else {
            real_bootloader.resize(len, 0);
        }

        real_bootloader
    }

    #[test]
    fn test_crc_vec() {
        let data: Vec<u8> = vec![0x61, 0x65, 0x6e, 0x67, 0x65, 0x6c, 0x6b, 0x65];
        let crc = calc_crc(&data);
        assert_eq!(crc, 0x7909E7C4);
    }

    #[test]
    fn reject_too_large_bootloader() {
        let bootloader = generate_bootloader_binary(METADATA_1_ADDR as usize + 1);
        let image_1 = vec![2u8; SLOT_SIZE as usize];
        let image_2 = vec![3u8; SLOT_SIZE as usize];
        let image_3 = vec![4u8; SLOT_SIZE as usize];

        let result = generate_buffer(&bootloader, &image_1, &image_2, &image_3);
        assert!(result.is_err());
    }

    #[test]
    fn reject_too_large_image() {
        let bootloader = generate_bootloader_binary(METADATA_1_ADDR as usize - 5);
        let image_1 = vec![2u8; SLOT_SIZE as usize + 1];
        let image_2 = vec![3u8; SLOT_SIZE as usize];
        let image_3 = vec![4u8; SLOT_SIZE as usize];

        let result = generate_buffer(&bootloader, &image_1, &image_2, &image_3);
        assert!(result.is_err());
    }

    #[test]
    fn test_full_buffer() -> Result<(), String> {
        let bootloader = generate_bootloader_binary(6105);

        let real_binary = include_bytes!("../testdata/main_ram.bin");
        let mut image_1 = vec![2u8; SLOT_SIZE as usize];
        image_1[..real_binary.len()].copy_from_slice(real_binary);
        let mut image_2 = vec![3u8; SLOT_SIZE as usize];
        image_2[..real_binary.len()].copy_from_slice(real_binary);
        let mut image_3 = vec![4u8; SLOT_SIZE as usize];
        image_3[..real_binary.len()].copy_from_slice(real_binary);

        verify_generated_buffer(bootloader, image_1, image_2, image_3)
    }

    #[test]
    fn test_partial_buffers() -> Result<(), String> {
        let bootloader = generate_bootloader_binary(METADATA_1_ADDR as usize - 5);
        let real_binary = include_bytes!("../testdata/main_ram.bin");

        let mut image_1 = vec![2u8; real_binary.len() + 12345];
        image_1[..real_binary.len()].copy_from_slice(real_binary);

        let mut image_2 = vec![3u8; SLOT_SIZE as usize];
        image_2[..real_binary.len()].copy_from_slice(real_binary);

        let mut image_3 = vec![4u8; 319581];
        image_3[..real_binary.len()].copy_from_slice(real_binary);

        verify_generated_buffer(bootloader, image_1, image_2, image_3)
    }

    #[test]
    fn fail_invalid_random_binary() {
        let bootloader_bin = generate_bootloader_binary(METADATA_1_ADDR as usize);

        let real_binary = include_bytes!("../testdata/main_flash.bin").to_vec();
        let fake_binary = include_bytes!("../testdata/urandom.bin").to_vec();

        assert!(verification::is_likely_valid_binary_buf(&real_binary).is_ok());
        assert!(verification::is_likely_valid_binary_buf(&fake_binary).is_err());

        assert!(generate_buffer(&bootloader_bin, &real_binary, &real_binary, &fake_binary).is_err());
    }

    #[test]
    fn fail_invalid_elf_binary() {
        let bootloader_bin = generate_bootloader_binary(METADATA_1_ADDR as usize);

        let real_binary = include_bytes!("../testdata/main_flash.bin").to_vec();
        let fake_binary = include_bytes!("../testdata/main_ram.elf").to_vec();

        assert!(verification::is_likely_valid_binary_buf(&real_binary).is_ok());
        assert!(verification::is_likely_valid_binary_buf(&fake_binary).is_err());

        assert!(generate_buffer(&bootloader_bin, &real_binary, &fake_binary, &real_binary).is_err());
    }

    #[test]
    fn fail_invalid_same_instruction() {
        let bootloader_bin = generate_bootloader_binary(METADATA_1_ADDR as usize);

        let real_binary = include_bytes!("../testdata/main_flash.bin").to_vec();
        let fake_binary = vec![0x00; real_binary.len()];

        assert!(verification::is_likely_valid_binary_buf(&real_binary).is_ok());
        assert!(verification::is_likely_valid_binary_buf(&fake_binary).is_err());

        assert!(generate_buffer(&bootloader_bin, &real_binary, &real_binary, &fake_binary).is_err());
    }

    // This function tests the generated buffer against the expected layout
    // It assumes the bootloader is only ones, and the images are only twos, threes and fours
    fn verify_generated_buffer(
        bootloader: Vec<u8>, image_1: Vec<u8>, image_2: Vec<u8>, image_3: Vec<u8>,
    ) -> Result<(), String> {
        let image_info: Vec<(&Vec<u8>, u32)> =
            vec![(&image_1, SLOT_1_ADDR), (&image_2, SLOT_2_ADDR), (&image_3, SLOT_3_ADDR)];

        // Assert each image is <= SLOT_SIZE bytes
        for (image_data, _) in image_info.iter() {
            if image_data.len() > SLOT_SIZE as usize {
                return Err(format!("Image size is too large: {}", image_data.len()));
            }
        }

        let buf = generate_buffer(&bootloader, &image_1, &image_2, &image_3);
        let generated_buffer = buf.map_err(|e| format!("Failed to generate buffer: {}", e))?;

        if generated_buffer.len() != FLASH_SIZE as usize {
            return Err("Generated buffer size is incorrect".to_string());
        }

        for i in 0..bootloader.len() {
            if generated_buffer[i] != bootloader[i] {
                return Err("Bootloader mismatch in generated buffer".to_string());
            }
        }
        for i in bootloader.len()..METADATA_1_ADDR as usize {
            if generated_buffer[i] != 0u8 {
                return Err("Unexpected non-zero byte in metadata area".to_string());
            }
        }

        for &metadata_addr in &[METADATA_1_ADDR, METADATA_2_ADDR] {
            let first_bytes = [
                1, 0, 0, 0, // version
                0, 0, 0, 0, // bootcounter
                0, 0, 0, 0, // preferred_image
            ];
            assert_eq!(first_bytes.len() as u32, METADATA_IMAGE_DATA_OFFSET);

            for i in metadata_addr..metadata_addr + METADATA_IMAGE_DATA_OFFSET {
                if generated_buffer[i as usize] != first_bytes[(i - metadata_addr) as usize] {
                    return Err("Metadata first bytes mismatch".to_string());
                }
            }

            if image_info.len() != NUMBER_OF_IMAGES as usize {
                return Err("Incorrect number of images".to_string());
            }

            let mut image_metadata_buf: Vec<u8> = vec![];

            for (image_data, _) in image_info.iter() {
                let image_metadata = ImageMetadata {
                    version: 1,
                    crc: calc_crc(image_data),
                    boot_counter: 0,
                    length: image_data.len() as u32,
                };

                let image_metadata_bytes = struct_to_bytes(&image_metadata);
                image_metadata_buf.extend_from_slice(&image_metadata_bytes);
            }

            if image_metadata_buf.len() != mem::size_of::<ImageMetadata>() * NUMBER_OF_IMAGES {
                return Err("Incorrect image metadata buffer size".to_string());
            }

            for i in metadata_addr + METADATA_IMAGE_DATA_OFFSET
                ..metadata_addr + METADATA_IMAGE_DATA_OFFSET + image_metadata_buf.len() as u32
            {
                let expect =
                    image_metadata_buf[(i - metadata_addr - METADATA_IMAGE_DATA_OFFSET) as usize];
                let actual = generated_buffer[i as usize];
                if expect != actual {
                    return Err(format!(
                        "Image metadata mismatch in generated buffer at {:#x}: expected {:#x} != {:#x} (actual)",
                        i, expect, actual
                    ));
                }
            }
        }

        for i in 0..NUMBER_OF_IMAGES {
            let (image_data, image_addr) = image_info[i];
            if i > 0 {
                let (_, prev_image_addr) = image_info[i - 1];
                if image_addr != prev_image_addr + SLOT_SIZE {
                    return Err("Image address mismatch".to_string());
                }
            }

            for j in 0..image_data.len() {
                if generated_buffer[(image_addr + j as u32) as usize] != image_data[j] {
                    return Err("Image data mismatch in generated buffer".to_string());
                }
            }

            for j in image_data.len()..SLOT_SIZE as usize {
                if generated_buffer[(image_addr + j as u32) as usize] != 0u8 {
                    return Err("Unexpected non-zero byte in image slot area".to_string());
                }
            }
        }

        for i in SLOT_3_ADDR + SLOT_SIZE..FLASH_SIZE {
            if generated_buffer[i as usize] != 0u8 {
                return Err("Unexpected non-zero byte in flash area".to_string());
            }
        }

        Ok(())
    }
}
