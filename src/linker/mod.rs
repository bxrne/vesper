use core::ptr::addr_of;

unsafe extern "C" {
    static _text_start: u8;
    static _text_end: u8;
    static _rodata_start: u8;
    static _rodata_end: u8;
    static _data_start: u8;
    static _data_end: u8;
    static _bss_start: u8;
    static _bss_end: u8;
    static _stack_start: u8;
    static _stack_end: u8;
    static _heap_start: usize;
    static _heap_size: usize;
    static _memory_end: u8;
}

#[inline]
fn sym_addr(sym: &'static u8) -> usize {
    sym as *const u8 as usize
}

#[inline]
pub fn text_start() -> usize {
    unsafe { sym_addr(&_text_start) }
}
#[inline]
pub fn text_end() -> usize {
    unsafe { sym_addr(&_text_end) }
}
#[inline]
pub fn rodata_start() -> usize {
    unsafe { sym_addr(&_rodata_start) }
}
#[inline]
pub fn rodata_end() -> usize {
    unsafe { sym_addr(&_rodata_end) }
}
#[inline]
pub fn data_start() -> usize {
    unsafe { sym_addr(&_data_start) }
}
#[inline]
pub fn data_end() -> usize {
    unsafe { sym_addr(&_data_end) }
}
#[inline]
pub fn bss_start() -> usize {
    unsafe { sym_addr(&_bss_start) }
}
#[inline]
pub fn bss_end() -> usize {
    unsafe { sym_addr(&_bss_end) }
}
#[inline]
pub fn stack_start() -> usize {
    unsafe { sym_addr(&_stack_start) }
}
#[inline]
pub fn stack_end() -> usize {
    unsafe { sym_addr(&_stack_end) }
}
#[inline]
pub fn heap_start() -> usize {
    addr_of!(_heap_start) as *const usize as usize
}
#[inline]
pub fn heap_size() -> usize {
    addr_of!(_heap_size) as *const usize as usize
}
#[inline]
pub fn memory_end() -> usize {
    unsafe { sym_addr(&_memory_end) }
}
