use std::{
    fs::File,
    io::{self, Read},
};

/// Convert a struct to a byte array - only little endian systems are supported
pub fn struct_to_bytes<T>(data: &T) -> Vec<u8> {
    #[cfg(not(target_endian = "little"))]
    compile_error!(
        r#"This function must only be used on little endian systems. This compile-time check makes sure you don't do anything unexpected."#
    );

    let size = core::mem::size_of::<T>();
    let ptr = data as *const T as *const u8;

    let mut result = Vec::with_capacity(size);

    unsafe {
        let slice = std::slice::from_raw_parts(ptr, size);
        result.extend_from_slice(slice);
    }

    result
}

/// Convert a byte array to a struct - only little endian systems are supported
pub fn bytes_to_struct<T>(data: &[u8]) -> T {
    #[cfg(not(target_endian = "little"))]
    compile_error!(
        r#"This function must only be used on little endian systems. This compile-time check makes sure you don't do anything unexpected."#
    );

    assert!(data.len() >= core::mem::size_of::<T>());

    let mut result = core::mem::MaybeUninit::<T>::uninit();
    let ptr = result.as_mut_ptr() as *mut u8;

    unsafe {
        ptr.copy_from_nonoverlapping(data.as_ptr(), core::mem::size_of::<T>());
        result.assume_init()
    }
}

/// Read a file into a byte vec
pub fn read_file(file_path: &std::path::PathBuf) -> io::Result<Vec<u8>> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();

    // Read entire file
    file.read_to_end(&mut buffer)?;

    Ok(buffer)
}

/// Set a buffer from from..from+src.len() to src and zero the rest until to
/// It also makes sure that src is not larger than target[from..to]
/// from to is with from inclusive and to exclusive
pub fn set_buf_from_to(
    target: &mut Vec<u8>, from: u32, to: u32, src: &Vec<u8>,
) -> Result<(), std::io::Error> {
    let from = from as usize;
    let to = to as usize;
    let src_len = src.len();
    let target_len = target.len();

    if from + src_len > to || to > target_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Invalid write from {} to {} of length {} (in buffer of size {})",
                from, to, src_len, target_len
            ),
        ));
    }

    // Basically write src to target[from..] and zero the rest
    target[from..from + src_len].copy_from_slice(src);
    target[from + src_len..to].fill(0);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_to_bytes() {
        #[repr(C)]
        struct TestStruct {
            a: u32,
            b: u16,
        }

        let test_struct = TestStruct { a: 1, b: 2 };
        let bytes = struct_to_bytes(&test_struct);

        assert_eq!(bytes, vec![1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn test_bytes_to_struct() {
        #[repr(C)]
        struct TestStruct {
            a: u32,
            b: u16,
        }

        let bytes = vec![1, 0, 0, 0, 2, 0, 0, 0];
        let test_struct: TestStruct = bytes_to_struct(&bytes);

        assert_eq!(test_struct.a, 1);
        assert_eq!(test_struct.b, 2);
    }

    #[test]
    fn test_back_to_back() {
        #[repr(C)]
        struct TestStruct {
            a: u32,
            b: u16,
        }

        let test_struct_1 = TestStruct { a: 1, b: 2 };
        let bytes = struct_to_bytes(&test_struct_1);
        let test_struct_2: TestStruct = bytes_to_struct(&bytes);

        assert_eq!(test_struct_1.a, 1);
        assert_eq!(test_struct_1.b, 2);

        assert_eq!(test_struct_2.a, 1);
        assert_eq!(test_struct_2.b, 2);
    }

    #[test]
    fn test_set_buf_from_to() {
        let mut target = vec![1u8; 10];
        let src = vec![2u8, 3u8, 4u8];

        set_buf_from_to(&mut target, 2, 6, &src).unwrap();

        assert_eq!(target, vec![1, 1, 2, 3, 4, 0, 1, 1, 1, 1]);
    }

    #[test]
    fn test_set_buf_from_to_invalid() {
        let mut target = vec![1u8; 10];
        let src = vec![2u8, 3u8, 4u8];

        assert!(set_buf_from_to(&mut target, 8, 10, &src).is_err());
    }
}
