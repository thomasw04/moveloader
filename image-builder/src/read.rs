use std::io::Error;

use crate::generate::calc_crc;
use crate::{byte_utils, verification};
use clap::Parser;

use crate::byte_utils::bytes_to_struct;
use interface::{
    Metadata, FLASH_SIZE, METADATA_1_ADDR, METADATA_2_ADDR, NUMBER_OF_IMAGES, SLOT_1_ADDR,
    SLOT_2_ADDR, SLOT_3_ADDR,
};

#[derive(Parser, Debug)]
pub struct ReadArguments {
    /// The path to the generated image file
    #[arg(short, long, default_value = "output_image.bin")]
    image_file: std::path::PathBuf,
}

/// Read an image with the given options
pub fn read(options: ReadArguments) -> Result<(), Error> {
    let bootloader_bin = byte_utils::read_file(&options.image_file)?;

    // First of all, make sure that it is exactly 2MB
    if bootloader_bin.len() as u32 != FLASH_SIZE {
        return Err(Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Image size is not 2MB, but {} bytes", bootloader_bin.len()),
        ));
    }

    let mut errors: Vec<String> = Vec::new();
    let metadata_1: Metadata = bytes_to_struct::<Metadata>(
        &bootloader_bin
            [METADATA_1_ADDR as usize..METADATA_1_ADDR as usize + std::mem::size_of::<Metadata>()],
    );
    let expected_crc = metadata_1.calc_crc();
    if metadata_1.crc != expected_crc {
        errors.push(format!(
            "Metadata 1 CRC is invalid: image specified {:#x}, but calculated value is {:#x}",
            metadata_1.crc, expected_crc
        ));
    }

    let metadata_2: Metadata = bytes_to_struct::<Metadata>(
        &bootloader_bin
            [METADATA_2_ADDR as usize..METADATA_2_ADDR as usize + std::mem::size_of::<Metadata>()],
    );
    if metadata_2.crc != metadata_2.calc_crc() {
        errors.push(format!(
            "Metadata 2 CRC is invalid: image specified {:#x}, but calculated value is {:#x}",
            metadata_2.crc,
            metadata_2.calc_crc()
        ));
    }

    let slot_starts = [SLOT_1_ADDR, SLOT_2_ADDR, SLOT_3_ADDR];

    for i in 0..NUMBER_OF_IMAGES {
        for (metadata_idx, &metadata) in [metadata_1, metadata_2].iter().enumerate() {
            let mut has_error = false;

            let img_metadata = &metadata.images[i];

            let start = slot_starts[i] as usize;
            let end = start + img_metadata.length as usize;

            if end > bootloader_bin.len() {
                errors.push(format!(
                    "Metadata {}: Image {} length is out of bounds: start: {:#x}, end: {:#x}, image length: {:#x}",
                    metadata_idx + 1,
                    i,
                    start,
                    end,
                    img_metadata.length
                ));
                continue;
            }

            // Calculate CRC on the image
            let crc = calc_crc(&bootloader_bin[start..end]);

            if crc != img_metadata.crc {
                errors.push(format!("Metadata {}: Image {} CRC is invalid: metadata specified {:#x}, but calculated value is {:#x}", metadata_idx + 1, i, img_metadata.crc, crc));
                has_error = true;
            }

            // Make sure we have the kind of instructions we expect and
            // not too many undefined instructions - a high ratio could indicate a
            // corrupted file
            let vec: Vec<u8> = bootloader_bin[start..end].to_vec();
            if let Err(e) = verification::is_likely_valid_binary_buf(&vec) {
                errors.push(format!(
                    "Metadata {}: Image {} is not a valid binary: {}",
                    metadata_idx + 1,
                    i,
                    e
                ));
                has_error = true;
            }

            if let Err(e) = verification::is_likely_valid_os_image_buf(&vec) {
                errors.push(format!(
                    "Metadata {}: Image {} is not a valid OS image: {}",
                    metadata_idx + 1,
                    i,
                    e
                ));
                has_error = true;
            }

            if !has_error {
                println!(
                    "Metadata {}: Image {} looks like a valid binary and matches the metadata CRC",
                    metadata_idx + 1,
                    i
                );
            }
        }
    }

    // Check if metadata is the same
    if metadata_1 != metadata_2 {
        errors.push(format!(
            "Metadata 1 and 2 are not the same:\nMetadata 1 at {:#x}: {:#?}\nMetadata 2 at {:#x}: {:#?}",
            METADATA_1_ADDR, metadata_1, METADATA_2_ADDR, metadata_2
        ));
    }

    if !errors.is_empty() {
        println!("Errors found:");
        for error in errors {
            println!(" - {}", error);
        }
        return Err(Error::new(std::io::ErrorKind::InvalidData, "Errors found in image"));
    }

    println!("All metadata pages are the same");
    println!("{:#?}", metadata_1);
    println!("All CRCs match the data they are pointing to.");
    Ok(())
}
