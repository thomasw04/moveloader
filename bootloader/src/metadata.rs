use interface::crc::calc_crc32;
use interface::{
    ImageMetadata, Metadata, U32Ext, METADATA_1_ADDR, METADATA_2_ADDR,
    SLOT_ADDRS,
};

use crate::flash::{Error, Flash};

fn read_metadata(addr: *const Metadata) -> Metadata {
    cortex_m::asm::dmb();

    let meta = unsafe { core::ptr::read(addr) };
    cortex_m::asm::dmb();

    meta
}

fn write_metadata(flash: &mut Flash, meta: &Metadata, addr: *mut usize) -> Result<Metadata, Error> {
    cortex_m::asm::dmb();

    static_assertions::const_assert_eq!(
        core::mem::size_of::<Metadata>(),
        core::mem::size_of::<[u64; 8]>(),
    );
    static_assertions::const_assert_eq!(
        core::mem::align_of::<Metadata>(),
        core::mem::align_of::<[u64; 8]>(),
    );
    unsafe {
        let array = core::mem::transmute::<&Metadata, &[u64; 8]>(meta);

        // Erase and program the flash.
        // If this error happens, we can't really do anything but a reset.
        // FLASH_CR Lock bit 31: "In case of an unsuccessful unlock operation,
        // this bit remains set until the next system reset."
        flash.unlock_flash()?;

        // TODO: Instead of using ? operator, we must relock the flash

        // Erase the page where addr is on
        // An error should only happen if we gave an invalid page address,
        // which is not possible if addr is in 0 <= addr < FLASH_SIZE
        // TODO: Check that the correct page is erased - I am semi-confident now that it works
        // or at least erasing page 1 as calculated here leads to 0x1000... being 0xff
        // TODO: this highly depends on the flash page size and dual/single-bank mode
        flash.erase_page(flash.address_to_page_number(addr as u32))?;

        // Write the actual data
        flash.write_dwords(addr, array)?;

        // Lock the flash again
        flash.lock_flash();
    }

    cortex_m::asm::dmb();

    // TODO: Either directly return MetaData (and thus trust that writing worked),
    // or read it back and with a correctness check (and maybe loop write it?)
    Ok(read_metadata(addr as *const Metadata))
}

pub fn select_image(meta: &Metadata) -> u32 {
    if let Some(image_meta) = meta.images.get(meta.preferred_image as usize) {
        //If crc is okay. Jump to our preferred image.
        if verify_image(image_meta, SLOT_ADDRS[meta.preferred_image as usize] as *const u8) {
            return meta.preferred_image;
        }
    }

    //Try to find a image with a valid crc.
    for (i, image_meta) in meta.images.iter().enumerate() {
        if verify_image(image_meta, SLOT_ADDRS[i] as *const u8) {
            return i as u32;
        }
    }

    todo!("Random image boot not implemented yet.");
}

// TODO: prefer to put into an ImageMetadata impl block, and make naming a bit more clear: e.g. is_valid
pub fn verify_image(image_meta: &ImageMetadata, addr: *const u8) -> bool {
    // Ensure u32 has same size as an *const u8
    #[cfg(not(kani))]
    static_assertions::const_assert_eq!(
        core::mem::size_of::<u32>(),
        core::mem::size_of::<*const u8>(),
    );

    let crc = calc_crc32(addr, image_meta.length.to_usize());
    crc == image_meta.crc
}

/// Selects which metadata to use and only returns valid metadata.
/// In case one metadata is invalid or outdated, it will be fixed automatically.
/// If both are invalid, it will return None.
/// The second tuple element returns whether or not fixing metadata was successful.
/// In case it was not necessary or possible, it will return Ok(()).
pub fn select_metadata(flash: &mut Flash) -> (Option<Metadata>, Result<(), Error>) {
    let metadata_one = read_metadata(METADATA_1_ADDR as *const Metadata);
    let metadata_two = read_metadata(METADATA_2_ADDR as *const Metadata);

    let MetadataSelectResult { meta, write_addr } = internal_select(
        metadata_one,
        metadata_one.is_valid(),
        metadata_two,
        metadata_two.is_valid(),
    );

    // If we got metadata, that is good
    if let Some(metadata) = meta {
        // We might have to overwrite write_addr, as the metadata there is outdated or corrupted
        if let Some(write_addr) = write_addr {
            let result = write_metadata(flash, &metadata, write_addr as *mut usize);
            (Some(metadata), result.map(|_| ()))
        } else {
            // Nothing to do -- all metadata is valid
            (Some(metadata), Ok(()))
        }
    } else {
        (None, Ok(()))
    }
}

struct MetadataSelectResult {
    // meta is the metadata that should be used for image selection.
    // If this is None, no valid metadata was found.
    meta: Option<Metadata>,

    // If write_addr is not None, we should write `meta` to write_addr.
    // This could be due to:
    //   - meta having a newer version than metadata at write_addr
    //   - meta being the only valid metadata
    write_addr: Option<usize>,
}

// This function implements the core logic of which metadata block to use.
// It is without side effects so we can verify it with Kani.
// If you find out how to make Kani assume something about method calls, we can
// move the is_valid calls into this function.
fn internal_select(
    md1: Metadata, md1_valid: bool, md2: Metadata, md2_valid: bool,
) -> MetadataSelectResult {
    match (md1_valid, md2_valid) {
        (true, true) => {
            // Both are valid - if one of them is newer, we use it and overwrite the old one
            if md1.version > md2.version {
                //Metadata one is newer. Use it.
                MetadataSelectResult {
                    meta: Some(md1),
                    // Overwrite metadata two
                    write_addr: Some(METADATA_2_ADDR as usize),
                }
            } else if md1.version < md2.version {
                //Metadata two is newer. Use it.
                MetadataSelectResult {
                    meta: Some(md2),
                    // Overwrite metadata one
                    write_addr: Some(METADATA_1_ADDR as usize),
                }
            } else {
                debug_assert!(md1.version == md2.version, "Both metadata versions are the same");

                //Both have to same version. All good.
                MetadataSelectResult {
                    meta: Some(md1),
                    // No need to overwrite anything
                    write_addr: None,
                }
            }
        }
        (true, false) => {
            //Only metadata one is valid. Use it to overwrite two
            MetadataSelectResult { meta: Some(md1), write_addr: Some(METADATA_2_ADDR as usize) }
        }
        (false, true) => {
            //Only metadata two is valid. Use it to overwrite one
            MetadataSelectResult { meta: Some(md2), write_addr: Some(METADATA_1_ADDR as usize) }
        }
        (false, false) => {
            // This is bad - we need to trigger a failsafe boot
            MetadataSelectResult { meta: None, write_addr: None }
        }
    }
}

#[cfg(kani)]
mod verification {
    use super::*;
    use interface::NUMBER_OF_IMAGES;
    use kani::*;

    // These are a bit more in the style of typical tests

    // First of all: all metadata is valid - try equal, less and greater version
    #[kani::proof]
    fn metadata_both_valid_same_version() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version == md2.version);

        // If both are valid, we should use md1 and not overwrite anything
        let result = internal_select(md1, true, md2, true);
        assert_eq!(result.meta, Some(md1));

        // This is important to save flash cycles
        assert_eq!(result.write_addr, None);
    }

    #[kani::proof]
    fn metadata_both_valid_diff_version_md1_newer() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version > md2.version);

        // If both are valid, we should use the newer one
        let result = internal_select(md1, true, md2, true);
        assert_eq!(result.meta, Some(md1));
        // ... and overwrite the older one
        assert_eq!(result.write_addr, Some(METADATA_2_ADDR as usize));
    }

    #[kani::proof]
    fn metadata_both_valid_diff_version_md2_newer() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version < md2.version);

        // Now we must use md2
        let result = internal_select(md1, true, md2, true);
        assert_eq!(result.meta, Some(md2));
        assert_eq!(result.write_addr, Some(METADATA_1_ADDR as usize));
    }

    // Now for cases where md1 is broken, again with equal, less and greater version
    #[kani::proof]
    fn metadata_one_broken_same_version() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version == md2.version);

        let result = internal_select(md1, false, md2, true);
        assert_eq!(result.meta, Some(md2));
        // We should fix metadata one
        assert_eq!(result.write_addr, Some(METADATA_1_ADDR as usize));
    }

    #[kani::proof]
    fn metadata_one_broken_diff_version_md1_newer() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version > md2.version);

        let result = internal_select(md1, false, md2, true);
        assert_eq!(result.meta, Some(md2));
        assert_eq!(result.write_addr, Some(METADATA_1_ADDR as usize));
    }

    #[kani::proof]
    fn metadata_one_broken_diff_version_md2_newer() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version < md2.version);

        let result = internal_select(md1, false, md2, true);
        assert_eq!(result.meta, Some(md2));
        assert_eq!(result.write_addr, Some(METADATA_1_ADDR as usize));
    }

    // Now for cases where md2 is broken, again with equal, less and greater version
    #[kani::proof]
    fn metadata_two_broken_same_version() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version == md2.version);

        let result = internal_select(md1, true, md2, false);
        assert_eq!(result.meta, Some(md1));
        // We should fix metadata two
        assert_eq!(result.write_addr, Some(METADATA_2_ADDR as usize));
    }

    #[kani::proof]
    fn metadata_two_broken_diff_version_md1_newer() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version > md2.version);

        let result = internal_select(md1, true, md2, false);
        assert_eq!(result.meta, Some(md1));
        assert_eq!(result.write_addr, Some(METADATA_2_ADDR as usize));
    }

    #[kani::proof]
    fn metadata_two_broken_diff_version_md2_newer() {
        let md1: Metadata = any();
        let md2: Metadata = any();
        assume(md1.version < md2.version);

        let result = internal_select(md1, true, md2, false);
        assert_eq!(result.meta, Some(md1));
        assert_eq!(result.write_addr, Some(METADATA_2_ADDR as usize));
    }

    // When both are broken, we should not return any metadata
    #[kani::proof]
    fn metadata_both_broken() {
        let md1: Metadata = any();
        let md2: Metadata = any();

        let result = internal_select(md1, false, md2, false);
        assert_eq!(result.meta, None);
        assert_eq!(result.write_addr, None);
    }
}
