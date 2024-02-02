use stm32l4::stm32l4r5::{self, Peripherals}; // logs messages to the host stderr; requires a debugger

// Notes:
// This implementation is based on two PDFs:
// - Datasheet: https://www.st.com/resource/en/datasheet/stm32l4r5vi.pdf
// - Reference Manual: https://www.st.com/resource/en/reference_manual/rm0432-stm32l4-series-advanced-armbased-32bit-mcus-stmicroelectronics.pdf
//   - Here especially Chapter 44 Independent watchdog (IWDG), page 1527
// - The LSI RC oscillator runs at ~29.5kHz - ~34kHz, so timing isn't super accurate, but this isn't too important
//   - Source: Datasheet, P196, Table 62. LSI oscillator characteristics
// - Watchdog timings assume that we run at 32kHz, so a different LSI frequency will change timing slightly
//   - See Datasheet, Table 97. IWDG min/max timeout period at 32 kHz (LSI)
// -> In general, there is some timing uncertainty, but this isn't important for us
//

/// This function must be called exactly once at boot to set up the watchdog.
/// It assumes that it was called after a reset.
/// Afterwards, feed() must be called between operations to prevent the hardware from resetting.
///
/// NOTE: this implementation explicitly ignores checking the IWDG_SR status.
/// The reason for that is that we assume that the bootloader starts after a reset,
/// and the status register is all zeroes when we start.
/// So basically in this function the WVU, RVU and PVU bits are assumed to be
/// zero on entry.
///
/// I would prefer not to add loops that wait for them to be cleared, even if
/// that would be a bit more correct - because if those happen to run infinitely,
/// we will not get reset by the watchdog, which would be very bad
pub fn setup_and_start() {
    // We want to set up the watchdog as described in "Configuring the IWDG when the window option is disabled"
    // TODO: not sure if the "Hardware watchdog" feature is enabled in the device option bits
    //   - if it is, the watchdog is already enabled at power on
    // TODO: Think about whether we want to continue the watchdog in low-power states - probably yes,
    // as we probably don't want to enter those in the bootloader anyways, so a reset would be desirable then

    let iwdg = unsafe { &*stm32l4r5::IWDG::ptr() };

    // 1. Enable the IWDG by writing 0x0000 CCCC in the IWDG key register (IWDG_KR)
    iwdg.kr.write(|w| w.key().start());

    // 2. Enable register access by writing 0x0000 5555 in the IWDG key register (IWDG_KR)
    // This gives us access to PR, RLR, WINR, as outlined in 44.3.5 Register access protection
    iwdg.kr.write(|w| w.key().enable());

    // 3. Write the prescaler by programming the IWDG prescaler register (IWDG_PR) from 0 to 7.
    // - Prescaler divider should be 256 (highest value) -> this gives us ~32 seconds until a watchdog reset
    //   - Datasheet Table 97: PR[2:0] should be "6 or 7" for /256 divider
    // Since we must keep the other bits at previous value, use modify, not write
    // TODO: There are 2 different ways of setting 256: figure out if there is a difference
    iwdg.pr.modify(|_, w| w.pr().divide_by256());

    // 4. Write the IWDG reload register (IWDG_RLR)
    // Now we can define which value is written on watchdog feed (0xAAAA written to KR)
    // We choose the max possible value 0xFFF, as combined with the divider, we
    // will get about ~32 seconds of time for operations between feeds.
    // This is the same as the reset value, but write it just to be sure and follow the exact setup procedure
    iwdg.rlr.modify(|_, w| w.rl().bits(0xFFF));

    // 5. Wait for the registers to be updated (IWDG_SR = 0x0000 0000).
    // This can take "up to five LSI/Prescale clock cycles"
    // TODO: Calculate how long this actually is and maybe reset (or continue) if we grossly exceed a counter
    // That way we would prevent an endless loop in case an update never happens (which would be a hardware fault?)
    loop {
        let r = iwdg.sr.read();
        // We want all updates to complete.
        // TODO: Technically we don't need WVU because we never set it.
        // Decide whether to keep its check or not
        if r.pvu().bit_is_clear() && r.rvu().bit_is_clear() && r.wvu().bit_is_clear() {
            // Now the whole IWDG_SR register is 0
            break;
        }
    }

    // Now 6. Refresh the counter value, which is equivalent to a normal watchdog feed
    // Note: as outlined in 44.3.5 Register access protection, we enabled modification in step 2.
    // Now we write a different value to KR, which locks our registers (PR, RLR, WINR) again!
    iwdg.kr.write(|w| w.key().reset());
}

/// Tell the watchdog that we are still alive by resetting it to the IWDG_RLR value.
/// Feed must be called at least every 30 seconds to ensure we don't run into a reset.
pub fn feed() {
    let iwdg = unsafe { &*stm32l4r5::IWDG::ptr() };

    // "Whenever the key value 0x0000 AAAA is written in the IWDG key register (IWDG_KR),
    // the IWDG_RLR value is reloaded in the counter and the watchdog reset is prevented."
    iwdg.kr.write(|w| w.key().reset());
}

// TODO: There is a way to tell whether the last reset was triggered by the watchdog (RCC_CSR, Bit 29).
// If we have a use for this information, we could add a helper function
