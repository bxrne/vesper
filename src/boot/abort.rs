use crate::arch;

#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    loop {
        arch::wfi();
    }
}
