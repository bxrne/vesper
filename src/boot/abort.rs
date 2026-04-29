use crate::arch;

/// Halt the hart forever. `wfi` parks the core at low power instead of
/// spinning; an interrupt would resume it but none are enabled, so this
/// is effectively a permanent stop.
#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    loop {
        arch::wfi();
    }
}
