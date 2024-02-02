use interface::{DUAL_BANK_PAGE_SIZE, FLASH_SIZE, SINGLE_BANK_PAGE_SIZE};
use static_assertions::{const_assert, const_assert_eq};
use stm32l4::stm32l4r5;

#[derive(Debug)]
pub enum Error {
    UnlockFailed,
    Busy,
    Illegal,
    InvalidPage,
}

// TODO: Make sure this part from the documentation is fine for us:
// The Flash erase and programming is only
// possible in the voltage scaling range 1. The VOS[1:0] bits in the PWR_CR1 must be
// programmed to 01b.

pub struct Flash {
    flash: stm32l4r5::FLASH,
}

impl Flash {
    const FLASH_KEY1: u32 = 0x4567_0123;
    const FLASH_KEY2: u32 = 0xCDEF_89AB;

    pub fn new(flash: stm32l4r5::FLASH) -> Self {
        Flash { flash }
    }

    pub fn is_dualbank(&self) -> bool {
        // Since we are on an 2MB device, we need to care about the DBANK bit (Bit 22),
        // while <= 1MB devices would have to check DB1M (Bit 21)
        // stm32l4 crate doesn't have a function for DBANK, so do it manually
        // Note that it does have one for DB1M named "dualbank", which is the wrong one
        // to check on a 2MB device.
        const BIT_22_BITMASK: u32 = 1 << 22;
        static_assertions::const_assert!(
            BIT_22_BITMASK == 0x00400000,
        );
        let dual_bank_bit = self.flash.optr.read().bits() & BIT_22_BITMASK;

        // Make sure we get a compile error here in case
        // this gets built for a different chip in the future
        const_assert!(
            FLASH_SIZE > 0x100000,
        );

        return dual_bank_bit != 0;
    }

    pub fn page_size(&self) -> u32 {
        if self.is_dualbank() {
            DUAL_BANK_PAGE_SIZE
        } else {
            SINGLE_BANK_PAGE_SIZE
        }
    }

    pub fn status(&self) -> Result<(), Error> {
        let sr = self.flash.sr.read();

        if sr.bsy().bit_is_set() {
            Err(Error::Busy)
        } else if sr.pgaerr().bit_is_set() || sr.progerr().bit_is_set() || sr.wrperr().bit_is_set()
        {
            Err(Error::Illegal)
        } else {
            Ok(())
        }
    }

    /// Unlock the flash according to the unlock sequence (see 3.3.5 Flash program and erase operations).
    /// This **MUST** be called before any flash write or erase operation!
    /// Afterwards, the user **SHOULD** lock the flash.
    pub fn unlock_flash(&mut self) -> Result<(), Error> {
        unsafe {
            self.flash.keyr.write(|w| w.keyr().bits(Flash::FLASH_KEY1));
            self.flash.keyr.write(|w| w.keyr().bits(Flash::FLASH_KEY2));
        }

        // Lock bit:
        // When set, the FLASH_CR register is locked. It is cleared by
        // hardware after detecting the unlock sequence.
        // In case of an unsuccessful unlock operation, this bit remains set until the next
        // system reset
        if self.flash.cr.read().lock().bit_is_clear() {
            Ok(())
        } else {
            Err(Error::UnlockFailed)
        }
    }

    /// Locks the flash after a write or erase operation, protecting it from accidental writes.
    pub fn lock_flash(&mut self) {
        // From the documentation:
        // > The FLASH_CR register cannot be written when the BSY bit in the Flash status register
        // > (FLASH_SR) is set. Any attempt to write to it with the BSY bit set will cause the AHB bus to
        // > stall until the BSY bit is cleared
        // This is fine for us, since we would want to wait for the flash to finish anyway.
        self.flash.cr.modify(|_, w| w.lock().clear_bit());
    }

    fn clear_programming_flags(&mut self) {
        // Page 131, "Programming errors"
        self.flash.sr.modify(|_, w| {
            w
                .progerr().clear_bit()
                .sizerr().clear_bit()
                .pgaerr().clear_bit()
                .pgserr().clear_bit()
                .wrperr().clear_bit()
                .miserr().clear_bit()
                .fasterr().clear_bit()
          });
    }

    /// Returns the page number for a given address.
    pub fn address_to_page_number(&self, address: u32) -> u32 {
        debug_assert!(address < FLASH_SIZE, "Address out of range");

        address / self.page_size()
    }

    pub fn erase_page(&mut self, page_number: u32) -> Result<(), Error> {
        // According to "3.3.6 Flash main memory erase sequences"

        // 1. Check that no Flash memory operation is ongoing by checking the BSY bit in FLASH_SR
        self.wait()?;

        // 2. Check and clear all error programming flags due to a previous programming. If not, PGSERR is set
        self.clear_programming_flags();

        // Step Nr. 3 differentiates between dual- and single-bank mode
        if self.is_dualbank() {
            // Dual-Bank mode, we have 512 pages with size 0x1000 bytes
            const_assert_eq!(512 * 0x1000, FLASH_SIZE);

            if page_number >= 512 {
                return Err(Error::InvalidPage);
            }

            // Select either bank 0 or 1, and inside of that, the page number
            // Note that the manual calls them Bank 1 and Bank 2, but we call them 0 and 1
            let bank = page_number / 256;
            let page_number = page_number % 256;
            // This shows that this calculation does what we want:
            const_assert_eq!(0 / 256, 0);
            const_assert_eq!(127 / 256, 0);
            const_assert_eq!(128 / 256, 0);
            const_assert_eq!(255 / 256, 0);
            const_assert_eq!(256 / 256, 1);
            const_assert_eq!(511 / 256, 1);

            // We are in Dual-Bank mode, pages are 0x1000 bytes long
            self.flash.cr.modify(|_, w| unsafe {
                // set the PER bit
                w.per().set_bit()
                // Select the bank (false => Bank 1, true => Bank 2)
                .bker().bit(bank == 1)
                // and select the page to erase (PNB)
                .pnb().bits(page_number as u8)
            });
        } else {
            // Single-Bank mode, we have 256 pages with size 0x2000 bytes
            const_assert_eq!(256 * 0x2000, FLASH_SIZE);

            if page_number >= 256 {
                return Err(Error::InvalidPage);
            }

            self.flash.cr.modify(|_, w| unsafe {
                w
                    // Set the PER bit
                    .per().set_bit()
                    // Select the page to erase
                    .pnb().bits(page_number as u8)
                    // The BKER bit [...] must be kept cleared
                    .bker().clear_bit()
            });
        }

        // 4. Set the STRT bit in the FLASH_CR register
        self.flash.cr.modify(|_, w| w.start().set_bit());

        // 5. Wait for the BSY bit to be cleared in the FLASH_SR register.
        // If a programming error happened, wait will return an error
        let result = self.wait();

        // Disable page erase again - this shouldn't be strictly necessary
        self.flash.cr.modify(|_, w| w.per().clear_bit());

        result
    }

    /// This must only be called when the following is true:
    /// - The flash is unlocked
    /// - The target page(s) have been erased before
    pub fn write_dwords(&mut self, mut address: *mut usize, array: &[u64]) -> Result<(), Error> {
        // See reference manual, "3.3.7 Flash main memory programming sequences"
        // We do "Standard programming"

        // 1. Check that no Flash main memory operation is ongoing
        self.wait()?;

        // 2. Check and clear all error programming flags due to a previous programming
        self.clear_programming_flags();

        // 3. Set the PG bit in the FLASH_CR register
        self.flash.cr.modify(|_, w| w.pg().set_bit());

        // 4. Perform the data write operation at the desired memory address, inside main memory block or OTP area
        for dword in array {
            unsafe {
                core::ptr::write_volatile(address, *dword as usize);
                core::ptr::write_volatile(address.add(1), (*dword >> 32) as usize);
                address = address.add(2);
            }

            // 5. Wait until the BSY bit is cleared in the FLASH_SR register
            self.wait()?;

            // 6. Check that EOP flag is set in the FLASH_SR register
            // (meaning that the programming operation has succeed), and clear it by software.
            // TODO: what do we do if this is not set? Try again? Give up?
            if self.flash.sr.read().eop().bit_is_set() {
                self.flash.sr.modify(|_, w| w.eop().clear_bit());
            }
        }

        // 7. Clear the PG bit in the FLASH_SR register if there no more programming request anymore.
        self.flash.cr.modify(|_, w| w.pg().clear_bit());

        Ok(())
    }

    pub fn wait(&mut self) -> Result<(), Error> {
        while self.flash.sr.read().bsy().bit_is_set() {}
        self.status()
    }
}
