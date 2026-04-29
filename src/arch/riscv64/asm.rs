use core::arch::asm;

/// Park the hart at low power until the next interrupt. Used by
/// idle/halt loops to avoid spinning at 100 % CPU.
#[inline]
pub fn wfi() {
    unsafe { asm!("wfi") }
}
