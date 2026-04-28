use core::panic::PanicInfo;

use crate::boot::abort::abort;
use crate::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    match info.location() {
        Some(loc) => println!("kernel panic at {}: {}", loc, info.message()),
        None => println!("kernel panic: {}", info.message()),
    }
    abort();
}
