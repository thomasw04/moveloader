#![no_std]

pub mod crc;

// This is the page size in single-bank mode
pub const SINGLE_BANK_PAGE_SIZE: u32 = 0x2000;
pub const DUAL_BANK_PAGE_SIZE: u32 = 0x1000;

// These are useful for assertions
pub const MAX_PAGE_SIZE: u32 = max(SINGLE_BANK_PAGE_SIZE, DUAL_BANK_PAGE_SIZE);
pub const MIN_PAGE_SIZE: u32 = min(SINGLE_BANK_PAGE_SIZE, DUAL_BANK_PAGE_SIZE);
const_assert!(MAX_PAGE_SIZE % MIN_PAGE_SIZE == 0);
const_assert!(MIN_PAGE_SIZE < MAX_PAGE_SIZE);

#[cfg(target = "thumbv7em-none-eabihf")]
use static_assertions::assert_eq_size;
use static_assertions::{const_assert, const_assert_eq};

use core::convert::TryFrom;

const fn max(a: u32, b: u32) -> u32 {
    if a > b {
        a
    } else {
        b
    }
}

const fn min(a: u32, b: u32) -> u32 {
    if a < b {
        a
    } else {
        b
    }
}


pub const NUMBER_OF_IMAGES: usize = 3;

// We have 2MB of flash.
pub const FLASH_SIZE: u32 = 0x200000;

// 504KB is the Maximum size for an image.
// TODO: Test if we can actually use an image of that size when copied
// into RAM
pub const SLOT_SIZE: u32 = 63 * MAX_PAGE_SIZE;

// This is where the metadata is stored, like CRCs, image info etc.
// We keep two copies on different pages to ensure reliability when we overwrite one of them
// Note that with both single- and dual-bank mode, we choose the same address
pub const METADATA_1_ADDR: u32 = MAX_PAGE_SIZE;
pub const METADATA_2_ADDR: u32 = 2 * MAX_PAGE_SIZE;

// This is where images are copied to before being executed.
// This is a RAM address, so it's not persistent across reboots.
pub const RAM_ADDR: u32 = 0x20000000;
pub const RAM_SIZE: u32 = 0xa0000; // 640KB

// Start addresses where we copy the images to
pub const SLOT_1_ADDR: u32 = 3 * MAX_PAGE_SIZE;
pub const SLOT_2_ADDR: u32 = SLOT_1_ADDR + SLOT_SIZE;
pub const SLOT_3_ADDR: u32 = SLOT_2_ADDR + SLOT_SIZE;

//Array with all the addresses of the slots.
pub const SLOT_ADDRS: [u32; NUMBER_OF_IMAGES] = [SLOT_1_ADDR, SLOT_2_ADDR, SLOT_3_ADDR];

mod asserts {
    use super::*;
    use core::mem::size_of;
    use static_assertions::const_assert;

    const_assert!(MIN_PAGE_SIZE > size_of::<Metadata>() as u32);

    const_assert!(METADATA_1_ADDR + size_of::<Metadata>() as u32 <= SLOT_1_ADDR);
    const_assert!(METADATA_1_ADDR + size_of::<Metadata>() as u32 <= METADATA_2_ADDR);
    const_assert!(METADATA_2_ADDR + size_of::<Metadata>() as u32 <= SLOT_1_ADDR);

    const_assert!(SLOT_1_ADDR + SLOT_SIZE <= SLOT_2_ADDR);
    const_assert!(SLOT_2_ADDR + SLOT_SIZE <= SLOT_3_ADDR);
    const_assert!(SLOT_3_ADDR + SLOT_SIZE <= FLASH_SIZE);

    const_assert!(MAX_PAGE_SIZE % MIN_PAGE_SIZE == 0);

    const_assert!(SLOT_1_ADDR % MIN_PAGE_SIZE == 0);
    const_assert!(SLOT_2_ADDR % MIN_PAGE_SIZE == 0);
    const_assert!(SLOT_3_ADDR % MIN_PAGE_SIZE == 0);

    const_assert!(SLOT_SIZE % MIN_PAGE_SIZE == 0);
}

// First of all, make sure all things we compile are using the same byte order, in this case little endian
#[cfg(not(target_endian = "little"))]
compile_error!(
    r#"It looks like you're compiling for a non-little endian target.
We currently only support little endian targets to ensure the actual bootloader and built image use the same layout.
Note that both the bootloader and image builder must use the same endianness."#
);

// All structs that are written to the image MUST:
// - be repr(C)
// - have an assertion for their size and alignment
// - only contain types that have their size checked below (test_size, test_align)
// THIS MEANS THAT USIZE or ISIZE MUST NOT BE USED IN THESE STRUCTS

#[repr(C)]
#[cfg_attr(not(target = "thumbv7em-none-eabihf"), derive(Debug, Default, Clone, Copy, PartialEq, Eq))]
pub struct ImageMetadata {
    pub version: u32,
    pub crc: u32,
    pub boot_counter: u32,
    pub length: u32,
}

#[repr(C, align(8))]
#[cfg_attr(not(target = "thumbv7em-none-eabihf"), derive(Debug, Clone, Copy, PartialEq, Eq))]
pub struct Metadata {
    // version MUST NEVER BE 0 or 0xffffffff, as these are values that can happen
    // when the flash is erased
    pub version: u32,
    pub bootcounter: u32,
    pub preferred_image: u32,
    pub images: [ImageMetadata; NUMBER_OF_IMAGES],
    // a CRC over the previous part of the metadata struct, but not the CRC field
    pub crc: u32,
}

#[cfg(kani)]
impl kani::Arbitrary for Metadata {
    fn any() -> Self {
        Metadata {
            version: kani::any_where(|&x| x > 0 && x < 0xffffffff),
            bootcounter: kani::any(),
            preferred_image: kani::any(),
            images: kani::any(),
            crc: kani::any(),
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for ImageMetadata {
    fn any() -> Self {
        ImageMetadata {
            version: kani::any(),
            boot_counter: kani::any(),
            length: kani::any(),
            crc: kani::any(),
        }
    }
}

// Where the images field is offset, in bytes, from the start of the metadata struct
pub const METADATA_IMAGE_DATA_OFFSET: u32 = unsafe {
    let metadata = core::mem::MaybeUninit::<Metadata>::uninit();
    let base_ptr = metadata.as_ptr() as *const u8;
    let images_ptr = &(*metadata.as_ptr()).images as *const _ as *const u8;
    images_ptr.offset_from(base_ptr) as u32
};

impl Metadata {
    pub fn is_valid(&self) -> bool {
        self.crc == self.calc_crc()
    }

    pub fn calc_crc(&self) -> u32 {
        const METADATA_WITHOUT_CRC_SIZE: usize =
            core::mem::size_of::<Metadata>() - core::mem::size_of::<u32>();

        // statically assert that crc is actually the last field and there is no padding
        static_assertions::const_assert_eq!(
            core::mem::size_of::<Metadata>(),
            METADATA_WITHOUT_CRC_SIZE + core::mem::size_of::<u32>()
        );

        crc::calc_crc32(self as *const _ as *const u8, METADATA_WITHOUT_CRC_SIZE)
    }

    pub fn set_crc(&mut self) {
        self.crc = self.calc_crc();
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn metadata_crc() {
        let mut metadata = Metadata {
            version: 1,
            bootcounter: 0,
            preferred_image: 0,
            images: [ImageMetadata::default(); NUMBER_OF_IMAGES],
            crc: 0xabcdef,
        };

        let crc_prev = metadata.calc_crc();
        assert_eq!(metadata.crc, 0xabcdef);

        metadata.set_crc();
        assert_eq!(metadata.crc, crc_prev);

        let crc_new = metadata.calc_crc();

        assert_eq!(crc_prev, crc_new);
    }
}

// ------------------
// Static checks
// This ensures that the bootloader and the image builder agree on the memory layout
// ------------------
#[allow(dead_code)]
mod base_checks {
    use crate::{MAX_PAGE_SIZE, METADATA_IMAGE_DATA_OFFSET, SLOT_1_ADDR};
    use core::mem::{align_of, size_of};

    use static_assertions::const_assert;

    use crate::{ImageMetadata, Metadata, METADATA_1_ADDR, METADATA_2_ADDR, NUMBER_OF_IMAGES};

    //Check that Metadata is at least on the second page. (Not on the same page as bootloader)
    const_assert!(METADATA_1_ADDR >= MAX_PAGE_SIZE);
    const_assert!(METADATA_2_ADDR >= MAX_PAGE_SIZE);

    const_assert!(METADATA_2_ADDR > METADATA_1_ADDR);

    //Check that the Execution Slot is at least one page apart from the Metadata.
    const_assert!(SLOT_1_ADDR - METADATA_1_ADDR >= MAX_PAGE_SIZE);
    const_assert!(SLOT_1_ADDR - METADATA_2_ADDR >= MAX_PAGE_SIZE);

    // ------------------
    // Metadata size/align check
    // ------------------
    const_assert!(size_of::<ImageMetadata>() == 16);
    const_assert!(
        size_of::<ImageMetadata>() * NUMBER_OF_IMAGES + METADATA_IMAGE_DATA_OFFSET as usize + 4
            == size_of::<Metadata>()
    );

    // Hardware testing scripts rely on this size
    const_assert!(size_of::<Metadata>() == 64);

    const_assert!(align_of::<ImageMetadata>() == 4);
    const_assert!(align_of::<Metadata>() == 8);

    const_assert!(align_of::<Metadata>() == align_of::<[u64; 8]>());

    // ------------------
    // Type size check
    // ------------------
    const_assert!(size_of::<i8>() == 1);
    const_assert!(size_of::<i16>() == 2);
    const_assert!(size_of::<i32>() == 4);
    const_assert!(size_of::<i64>() == 8);
    const_assert!(size_of::<i128>() == 16);

    const_assert!(size_of::<u8>() == 1);
    const_assert!(size_of::<u16>() == 2);
    const_assert!(size_of::<u32>() == 4);
    const_assert!(size_of::<u64>() == 8);
    const_assert!(size_of::<u128>() == 16);

    const_assert!(size_of::<f32>() == 4);
    const_assert!(size_of::<f64>() == 8);

    const_assert!(size_of::<bool>() == 1);
    const_assert!(size_of::<char>() == 4);
    const_assert!(size_of::<[u32; 3]>() == 12);
    const_assert!(size_of::<()>() == 0);

    // ------------------
    // Type align check
    // ------------------
    const_assert!(align_of::<i8>() == 1);
    const_assert!(align_of::<i16>() == 2);
    const_assert!(align_of::<i32>() == 4);
    const_assert!(align_of::<i64>() == 8);

    const_assert!(align_of::<u8>() == 1);
    const_assert!(align_of::<u16>() == 2);
    const_assert!(align_of::<u32>() == 4);
    const_assert!(align_of::<u64>() == 8);

    const_assert!(align_of::<f32>() == 4);
    const_assert!(align_of::<f64>() == 8);

    const_assert!(align_of::<bool>() == 1);
    const_assert!(align_of::<char>() == 4);
    const_assert!(align_of::<[u32; 3]>() == 4);
    const_assert!(align_of::<()>() == 1);
}

pub trait U32Ext {
    fn to_usize(self) -> usize;
}

impl U32Ext for u32 {
    /// Convert a u32 to a usize.
    /// A const_assert ensures that this is always possible.
    fn to_usize(self) -> usize {
        #[cfg(target = "thumbv7em-none-eabihf")]
        assert_eq_size!(usize, u32);

        //This should be not possible to happen.
        //When it does happen we just return 0 and hope that the show goes on.
        usize::try_from(self).unwrap_or(0)
    }
}
