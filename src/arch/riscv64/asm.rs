use core::arch::asm;

#[inline]
pub fn wfi() {
    unsafe { asm!("wfi") }
}
