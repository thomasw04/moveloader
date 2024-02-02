use core::ops::Range;

/// Calculate the number of pages that is spanned by a given start address and length.
/// Since the start address is assumed to be page aligned, we only need the length.
/// Basically, if you have start=0x4000 and length=0x3000, you will get 2 pages.
pub const fn page_span(length: u32, page_size: u32) -> u32 {
    debug_assert!(length <= interface::FLASH_SIZE, "length must be <= FLASH_SIZE");

    (length + page_size - 1) / page_size
}

mod asserts {
    use super::page_span;
    use interface::{MAX_PAGE_SIZE, MIN_PAGE_SIZE, SLOT_1_ADDR, SLOT_2_ADDR};
    use static_assertions::const_assert_eq;

    // We want to copy, in pages, but not more than is necessary.
    // E.g. if we have 1.5 pages, we want to copy 2 pages.

    // So assuming a page size of 0x2000, page indexes starting at 0:
    // | addr   | length | start_page (inclusive) | last_page (exclusive) | count |
    // | 0x4000 | 0x3000 |                      2 |                     4 |    2 |
    // | 0x4000 | 0x4000 |                      2 |                     4 |    2 |
    // | 0x4000 | 0x4001 |                      2 |                     5 |    3 |

    // Note that a range is x for start <= x < end, so the last page is exclusive.
    const_assert_eq!(page_span(1, MAX_PAGE_SIZE), 1);
    const_assert_eq!(page_span(0x3000, MAX_PAGE_SIZE), 2);
    const_assert_eq!(page_span(0x4000, MAX_PAGE_SIZE), 2);
    const_assert_eq!(page_span(0x4001, MAX_PAGE_SIZE), 3);
    const_assert_eq!((SLOT_2_ADDR - SLOT_1_ADDR) / MAX_PAGE_SIZE, page_span(SLOT_2_ADDR - SLOT_1_ADDR, MAX_PAGE_SIZE));

    const_assert_eq!(page_span(1, MIN_PAGE_SIZE), 1);
    const_assert_eq!(page_span(0x1800, MIN_PAGE_SIZE), 2);
    const_assert_eq!(page_span(0x2000, MIN_PAGE_SIZE), 2);
    const_assert_eq!(page_span(0x2001, MIN_PAGE_SIZE), 3);
    const_assert_eq!((SLOT_2_ADDR - SLOT_1_ADDR) / MIN_PAGE_SIZE, page_span(SLOT_2_ADDR - SLOT_1_ADDR, MIN_PAGE_SIZE));
}

#[cfg(kani)]
mod verification {
    use super::*;
    use interface::{MAX_PAGE_SIZE, MIN_PAGE_SIZE};
    use kani::*;

    #[kani::proof]
    fn verify_page_span_min() {
        // Given an length <= FLASH_SIZE, we get a valid page span.
        let length: u32 = any();
        assume(length <= interface::FLASH_SIZE);

        let span = page_span(length, MIN_PAGE_SIZE);

        assert!(span * MIN_PAGE_SIZE >= length);
        assert!(span <= interface::FLASH_SIZE / MIN_PAGE_SIZE);
    }

    #[kani::proof]
    fn verify_page_span_max() {
        // Given an length <= FLASH_SIZE, we get a valid page span.
        let length: u32 = any();
        assume(length <= interface::FLASH_SIZE);

        let span = page_span(length, MAX_PAGE_SIZE);

        assert!(span * MAX_PAGE_SIZE >= length);
        assert!(span <= interface::FLASH_SIZE / MAX_PAGE_SIZE);
    }
}
