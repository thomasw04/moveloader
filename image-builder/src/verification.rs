use std::fmt::{self};

#[derive(Debug)]
pub enum BinaryFileError {
    ReadFileError(Box<std::io::Error>),
    HasELFHeader,
    ManyUndefinedInstructions { total_instructions: usize, undefined_lines: usize, ratio: f64 },
    NotEnoughUniqueInstructions { unique_instructions: usize },
    UnexpectedInterruptVectorTable { entrypoint_address: u32 },
}

impl std::error::Error for BinaryFileError {}

impl std::fmt::Display for BinaryFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryFileError::ReadFileError(err) => write!(f, "Error while reading file: {}", err),
            BinaryFileError::HasELFHeader => write!(f, "Error: File has ELF magic bytes, but expected raw binary"),
            BinaryFileError::ManyUndefinedInstructions {
                total_instructions: total_lines,
                undefined_lines,
                ratio,
            } => write!(
                f,
                "Error: Too many undefined instructions ({}%). Total lines: {}, Undefined lines: {}",
                *ratio * 100f64, total_lines, undefined_lines
            ),
            BinaryFileError::NotEnoughUniqueInstructions {
                unique_instructions
            } => write!(
                f,
                "Error: Too few unique instruction mnemonics (have {}, but expected at least {})",
                unique_instructions, BINARY_MIN_UNIQUE_INSTRUCTIONS
            ),
            BinaryFileError::UnexpectedInterruptVectorTable {
                entrypoint_address
            } => write!(
                f,
                "Error: Unexpected interrupt vector table, entrypoint address is at 0x{:08x}, but expected >= 0x{:08x} (RAM start) and within RAM of size 0x{:08x}",
                entrypoint_address, RAM_ADDR, RAM_SIZE
            ),
        }
    }
}

impl From<std::io::Error> for BinaryFileError {
    fn from(value: std::io::Error) -> Self {
        Self::ReadFileError(Box::new(value))
    }
}

pub fn is_likely_valid_binary_buf(bytes: &[u8]) -> Result<(), BinaryFileError> {
    // If it has an ELF header or magic bytes, someone tries flashing an ELF binary.
    // That does not work, as we need only the instruction bytes
    match bytes.get(0..4) {
        Some(&[0x7f, 0x45, 0x4c, 0x46]) => return Err(BinaryFileError::HasELFHeader),
        _ => {
            // No elf header in sight. That's good
        }
    }

    // Now take a look at the instructions we read from the file.
    // If it has a large number of undefined instructions, we raise concern
    let binary_stats = get_thumb_instruction_stats(bytes)?;
    let total = binary_stats.total_instructions();
    let ratio = binary_stats.undefined_count as f64 / total as f64;
    if ratio > BINARY_UNDEFINED_INSTRUCTIONS_THRESHOLD {
        return Err(BinaryFileError::ManyUndefinedInstructions {
            total_instructions: total,
            undefined_lines: binary_stats.undefined_count,
            ratio,
        });
    }

    let unique = binary_stats.unique_instruction_count();
    if unique < BINARY_MIN_UNIQUE_INSTRUCTIONS {
        return Err(BinaryFileError::NotEnoughUniqueInstructions { unique_instructions: unique });
    }

    Ok(())
}

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

use interface::{RAM_ADDR, RAM_SIZE};
use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Default, Debug)]
struct InstructionStats {
    counts: HashMap<String, usize>,
    pub undefined_count: usize,
}

impl InstructionStats {
    fn add_instruction(&mut self, instruction: &str) {
        *self.counts.entry(instruction.to_string()).or_insert(0) += 1;
    }

    fn add_undefined(&mut self) {
        self.undefined_count += 1;
    }

    pub fn total_instructions(&self) -> usize {
        self.undefined_count + self.counts.values().sum::<usize>()
    }

    pub fn unique_instruction_count(&self) -> usize {
        self.counts.len()
    }
}

static OBJDUMP_LINE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*([\da-f]+):\s*([\da-f]+)\s*(\S+)").expect("regex has error"));

static BINARY_UNDEFINED_INSTRUCTIONS_THRESHOLD: f64 = 0.01;
static BINARY_MIN_UNIQUE_INSTRUCTIONS: usize = 15;

fn split_objdump_instruction_line(input: &str) -> Result<(String, String, String), &'static str> {
    // Match the input string against the regex pattern
    if let Some(captures) = OBJDUMP_LINE_REGEX.captures(input) {
        // Extract capture groups
        let group1 =
            captures.get(1).map(|m| m.as_str().to_string()).ok_or("Missing capture group 1")?;
        let group2 =
            captures.get(2).map(|m| m.as_str().to_string()).ok_or("Missing capture group 2")?;
        let group3 =
            captures.get(3).map(|m| m.as_str().to_string()).ok_or("Missing capture group 3")?;

        // Return a tuple of three strings
        Ok((group1, group2, group3))
    } else {
        // Return an error if there is no match
        Err("Input string does not match the expected pattern")
    }
}

fn get_thumb_instruction_stats(buffer: &[u8]) -> Result<InstructionStats, std::io::Error> {
    // Write to temp file
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(buffer)?;

    let fp = file.into_temp_path();
    let file_path = fp
        .to_str()
        .expect("Weird: named temp file path contains path character not allowed on platform");

    let child = Command::new("arm-none-eabi-objdump")
        .args(["-D", "-m", "arm", "-b", "binary", "-M", "force-thumb", "-EL", file_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let output = child.wait_with_output()?;

    let disassembly = String::from_utf8_lossy(&output.stdout);

    let mut stats = InstructionStats::default();

    for line in disassembly.lines() {
        if line.contains("<UNDEFINED>") {
            stats.add_undefined();
        } else if let Ok((_, _, instruction_name)) = split_objdump_instruction_line(line) {
            stats.add_instruction(&instruction_name);
        }
    }

    Ok(stats)
}

/// Checks if the vtable of an OS image points towards a valid address.
/// This function should be used **IN ADDITION TO THE OTHER BINARY CHECK** for OS images
pub fn is_likely_valid_os_image_buf(bytes: &[u8]) -> Result<(), BinaryFileError> {
    // The bootloader reads an address from the vtable and jumps to it,
    // so let's make sure that address is within RAM
    let entrypoint_address = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if !(RAM_ADDR..RAM_ADDR + RAM_SIZE).contains(&entrypoint_address) {
        return Err(BinaryFileError::UnexpectedInterruptVectorTable { entrypoint_address });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_invalid_binary() {
        let binary_data = include_bytes!("../testdata/urandom.bin");

        let results = get_thumb_instruction_stats(binary_data);

        assert!(results.is_ok());

        let results = results.unwrap();

        assert!(results.undefined_count > 10);
        assert!(
            results.undefined_count as f64 / (results.total_instructions() as f64)
                > BINARY_UNDEFINED_INSTRUCTIONS_THRESHOLD
        );
    }

    #[test]
    fn detect_raw_binary() {
        let binary_data = include_bytes!("../testdata/main_flash.bin");

        let results = get_thumb_instruction_stats(binary_data);

        assert!(results.is_ok());

        let results = results.unwrap();

        assert!(results.undefined_count < 10);
        assert!(
            results.undefined_count as f64 / (results.total_instructions() as f64)
                < BINARY_UNDEFINED_INSTRUCTIONS_THRESHOLD
        );
    }

    #[test]
    fn wrong_address_in_flash() {
        let binary_data = include_bytes!("../testdata/main_flash.bin");

        let result = is_likely_valid_os_image_buf(binary_data);

        assert!(result.is_err());

        assert!(matches!(
            result.unwrap_err(),
            BinaryFileError::UnexpectedInterruptVectorTable { entrypoint_address: _ }
        ));
    }

    #[test]
    fn wrong_address_outside_ram() {
        let mut binary_data = include_bytes!("../testdata/main_flash.bin").to_vec();

        // Change the entrypoint address to RAM_ADDR + RAM_SIZE + 4
        const ENTRY_ADDR: u32 = RAM_ADDR + RAM_SIZE + 4;
        let nbytes = ENTRY_ADDR.to_le_bytes();
        binary_data[4..(4 + 4)].copy_from_slice(&nbytes[..4]);

        let result = is_likely_valid_os_image_buf(&binary_data.to_vec());

        assert!(result.is_err());

        match result.unwrap_err() {
            BinaryFileError::UnexpectedInterruptVectorTable { entrypoint_address } => {
                assert_eq!(entrypoint_address, ENTRY_ADDR);
            }
            _ => {
                panic!("Unexpected error type");
            }
        }
    }

    #[test]
    fn correct_address_in_ram() {
        let binary_data = include_bytes!("../testdata/main_ram.bin");

        let result = is_likely_valid_os_image_buf(binary_data);

        assert!(result.is_ok());
    }
}
