const DEFAULT_POLYNOM: u32 = 0x82F63B78;
const CRC_INITIAL_VALUE: u32 = 0xFFFFFFFF;
const CRC_FINAL_XOR_VALUE: u32 = 0xFFFFFFFF;

/// Calculate CRC32-C on a memory buffer
pub fn calc_crc32(message: *const u8, length: usize) -> u32 {
    let polynom = DEFAULT_POLYNOM;

    if message.is_null() {
        return 0;
    }
    let mut crc: u32 = CRC_INITIAL_VALUE;

    let mem = unsafe { core::slice::from_raw_parts(message, length) };

    for i in mem.iter().take(length) {
        crc ^= *i as u32;
        for _j in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ polynom;
            } else {
                crc >>= 1;
            }
        }
    }

    crc ^ CRC_FINAL_XOR_VALUE
}

#[cfg(test)]
mod tests {
    use core::ptr::null;

    use crate::Metadata;

    use super::*;

    // Values tested with http://www.sunshine2k.de/coding/javascript/crc/crc_js.html
    // Settings: CRC32-C, polynomial 0x1EDC6F41 (bit reverse of DEFAULT_POLYNOM)

    #[test]
    fn test_null() {
        let crc = calc_crc32(null(), 0);
        assert_eq!(crc, 0);
    }

    #[test]
    fn test_crc_a() {
        let data: [u8; 8] = [0x61, 0x65, 0x6e, 0x67, 0x65, 0x6c, 0x6b, 0x65];
        let crc = calc_crc32(data.as_ptr(), data.len());
        assert_eq!(crc, 0x7909E7C4);
    }

    #[test]
    fn test_crc() {
        let data: [u8; 4] = [0x00, 0x01, 0x02, 0x03];
        let crc = calc_crc32(data.as_ptr(), data.len());
        assert_eq!(crc, 0xD9331AA3);
    }

    #[test]
    fn test_amogus() {
        let data: [u8; 6] =
            "amogus".as_bytes().try_into().expect("Failed to convert string to array");
        let crc = calc_crc32(data.as_ptr(), data.len());
        assert_eq!(crc, 0x438F5AB0);
    }

    #[test]
    fn test_long_text() {
        let data : [u8; 387] = "I'd just like to interject for a moment. What you're refering to as Linux, is in fact, GNU/Linux, or as I've recently taken to calling it, GNU plus Linux. Linux is not an operating system unto itself, but rather another free component of a fully functioning GNU system made useful by the GNU corelibs, shell utilities and vital system components comprising a full OS as defined by POSIX.".as_bytes().try_into().expect("Failed to convert string to array");
        let crc = calc_crc32(data.as_ptr(), data.len());
        assert_eq!(crc, 0xCBBF20D6);
    }

    #[test]
    fn test_erased_metadata() {
        // One of the most common scenarios we should expect is that
        // we have erased a metadata page, but it was not written to due to power loss.
        // In that case, we will CRC over
        const SIZE: usize = core::mem::size_of::<Metadata>();

        let data: [u8; SIZE] = [0xff; SIZE];

        // Transmute to Metadata
        let metadata = unsafe { core::mem::transmute::<_, Metadata>(data) };

        // We need an CRC algorithm that does not result in default values for erased pages
        let crc = metadata.calc_crc();
        assert_ne!(crc, 0xffffffff);
        assert_ne!(crc, 0);
        assert!(!metadata.is_valid());
    }

    #[test]
    fn test_zeroed_metadata() {
        const SIZE: usize = core::mem::size_of::<Metadata>();
        let data: [u8; SIZE] = [0; SIZE];
        let metadata = unsafe { core::mem::transmute::<_, Metadata>(data) };

        // We need an CRC algorithm that does not result in default values for erased pages
        let crc = metadata.calc_crc();
        assert_ne!(crc, 0xffffffff);
        assert_ne!(crc, 0);
        assert!(!metadata.is_valid());
    }
}

#[cfg(kani)]
mod verification {
    use super::*;
    use kani::*;

    #[kani::proof]
    fn verify_crc32() {
        // We need to set some upper limit for kani to not run forever
        // In the real world, our data is larger than this!
        const LIMIT: usize = 128;
        let data : [u8; LIMIT] = any();
        assume(data.len() <= LIMIT);

        let crc = calc_crc32(data.as_ptr(), data.len());

        if data.len() == 0 {
            assert_eq!(crc, 0);
        }
    }
}
