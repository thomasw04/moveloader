#![no_std]
#![no_main]

use cortex_m_rt::entry;

use flash::Flash;
use interface::{
    crc::calc_crc32, U32Ext, NUMBER_OF_IMAGES, RAM_ADDR, SLOT_ADDRS, SLOT_SIZE,
};

use metadata::{select_image, select_metadata};
// pick a panicking behavior
// use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_itm as _; // logs messages over ITM; requires ITM support
#[cfg(debug_assertions)]
use panic_semihosting as _;

// However, this doesn't make any sense once deployed - if we have any kind of error,
// we should want to reset and restart our device - in the hope that we survive until
// we have booted the next image.
// TODO: come up with a better strategy that prevents bootloops
#[cfg(not(debug_assertions))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::peripheral::SCB::sys_reset();
    }
}

// TODO: use #[exception] to overwrite exception handlers, as otherwise:
// "If not overridden all exception handlers default to an infinite loop." - https://docs.rs/cortex-m-rt/latest/cortex_m_rt/#features

use stm32l4::stm32l4r5::{self, Peripherals, RTC}; // logs messages to the host stderr; requires a debugger

mod flash;
mod metadata;
mod pages;
mod watchdog;

fn failsafe_boot() -> ! {
    todo!("Failsafe boot not implemented yet.");
}

fn jump_to_image(core: &mut cortex_m::Peripherals) -> ! {
    cortex_m::asm::dmb();

    unsafe {
        //Jump to begin of ram.
        let exec = *((RAM_ADDR + 4) as *const usize);
        //Set vtable to begin of the flash memory.
        core.SCB.vtor.write(RAM_ADDR as u32);

        // "Privileged software can write to the VTOR to relocate the vector table start
        // address to a different memory location, in the range 0x00000080 to 0x3FFFFF80"
        static_assertions::const_assert!(RAM_ADDR < 0x3FFFFF80);

        // The initial stack pointer is defined in the linker script (e.g. stm32l4r5-ram.ld) like this:
        //    /* Highest address of the user mode stack */
        //    _estack = 0x20050000;
        // So here we just write the msp the OS image will expect, then we jump to exec
        cortex_m::asm::bootstrap(0x20050000 as *const u32, exec as *const u32);
    }
}

fn copy_image_to_ram(flash: &Flash, addr: u32, length: usize) -> Result<(), ()> {
    let crc_before = calc_crc32(addr as *const u8, length);

    let page_size = flash.page_size();

    debug_assert!(addr % page_size == 0, "Copy start address must be page aligned");

    for _ in 0..3 {
        cortex_m::asm::dmb();

        let src_start = addr;
        let dst_start = RAM_ADDR;

        // Split the copy into chunks of pages. Reset the watchdog after each page.
        for i in 0..pages::page_span(length as u32, page_size) {
            // TODO: Make sure this is always < 30 seconds
            unsafe {
                core::ptr::copy_nonoverlapping(
                    (src_start + (i * page_size)) as *const u8,
                    (dst_start + (i * page_size)) as *mut u8,
                    page_size as usize,
                );
            }
            cortex_m::asm::dmb();

            watchdog::feed();
        }

        // Now read it back and calculate CRC again
        if crc_before == calc_crc32(RAM_ADDR as *mut u8, length) {
            // Image in RAM and correct, nice!
            return Ok(());
        }
    }

    return Err(());
}

//Check rtc backup register for index + magic value. If it is there, we return the index and clear the register.
fn is_soft(rtc: &RTC) -> Option<u32> {
    //Check if register contains magic value.
    if rtc.bkpr[0].read().bits() == 0x5457 {
        //Get index and clear register.
        let index = rtc.bkpr[1].read().bits();
        rtc.bkpr[0].write(|w| unsafe { w.bits(0) });
        rtc.bkpr[1].write(|w| unsafe { w.bits(0) });

        //Check if index is valid.
        if index < NUMBER_OF_IMAGES as u32 {
            Some(index)
        } else {
            None
        }
    } else {
        None
    }
}

// TODO: look into:
// - BFB2 bit in flash optr register
fn run() -> ! {
    //Make sure that we know where we are.
    watchdog::setup_and_start();

    let mut core_peripherals = stm32l4r5::CorePeripherals::take().unwrap();
    let peripherals = stm32l4r5::Peripherals::take().unwrap();

    let mut flash = Flash::new(peripherals.FLASH);

    //First check if we are in a soft reboot. e.g. "reboot into image without setting it permanent."
    if let Some(index) = is_soft(&peripherals.RTC) {
        copy_image_to_ram(&flash, SLOT_ADDRS[index as usize], SLOT_SIZE as usize);
        jump_to_image(&mut core_peripherals);

        // We should never reach this after jump_to_image
        #[allow(unreachable_code)]
        {
            unreachable!("Jump to image failed.");
        }
    }

    let (metadata, fix_result) = select_metadata(&mut flash);
    if let Err(_) = fix_result {
        // TODO: Think about what we should do
    }

    // TODO: in case fix_result is an Err(), then something went wrong while fixing up
    // the older or corrupted metadata. Maybe we should try to fix this?

    match metadata {
        Some(metadata) => {
            let index = select_image(&metadata);

            copy_image_to_ram(
                &flash,
                SLOT_ADDRS[index as usize],
                metadata.images[index as usize].length.to_usize(),
            );
            jump_to_image(&mut core_peripherals);
        }
        None => {
            //No valid metadata found. Try to boot failsafe image.
            failsafe_boot();
        }
    }

    // TODO: Do a reset instead, that way we are at least recoverable (?)
    // Does the watchdog help us here?
}

#[entry]
fn main() -> ! {
    run()
}
