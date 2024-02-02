use std::io::Error;

use crate::{byte_utils, generate::generate_buffer};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct WriteArguments {
    /// The path to the bootloader binary
    #[clap(short, long, default_value = "bootloader.bin")]
    bootloader_path: std::path::PathBuf,

    /// The path to the first image binary.
    /// This is usually a compiled RODOS image
    #[arg(short = '1', long)]
    image_1_path: std::path::PathBuf,

    /// The path to the second image binary (if not given, use the first)
    #[arg(short = '2', long)]
    image_2_path: Option<std::path::PathBuf>,

    /// The path to the third image binary (if not given, use the first)
    #[arg(short = '3', long)]
    image_3_path: Option<std::path::PathBuf>,

    /// The path to the output file
    #[arg(short, long, default_value = "output_image.bin")]
    output_path: std::path::PathBuf,
}

/// Write an image with the given options
pub fn write(options: WriteArguments) -> Result<(), Error> {
    let bootloader_bin = byte_utils::read_file(&options.bootloader_path)?;
    println!("Read bootloader of size {}", bootloader_bin.len());

    let image_1_bin = byte_utils::read_file(&options.image_1_path)?;
    println!("Read first image of size {}", image_1_bin.len());

    let image_2_bin = if let Some(second_image_path) = options.image_2_path {
        let img = byte_utils::read_file(&second_image_path)?;
        println!("Read image 2 of size {}", img.len());
        img
    } else {
        println!("No second image provided, reusing first image");
        image_1_bin.clone()
    };

    let image_3_bin = if let Some(third_image_path) = options.image_3_path {
        let img = byte_utils::read_file(&third_image_path)?;
        println!("Read image 3 of size {}", img.len());
        img
    } else {
        println!("No third image provided, reusing first image");
        image_1_bin.clone()
    };

    let data = generate_buffer(&bootloader_bin, &image_1_bin, &image_2_bin, &image_3_bin)?;

    std::fs::write(&options.output_path, &data)?;

    // Read it back in and verify it
    let reread_data = byte_utils::read_file(&options.output_path)?;

    if reread_data != data {
        // try to delete, but ignore errors
        let _ = std::fs::remove_file(&options.output_path);

        return Err(Error::new(std::io::ErrorKind::Other, "Written data does not match read data"));
    }

    println!("Successfully wrote and verified {}", options.output_path.display());

    Ok(())
}
