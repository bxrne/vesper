//! Accessors for symbols defined in `src/lds/virt.lds`.
//!
//! Linker symbols don't have a value the way ordinary statics do — the
//! address of the symbol *is* the value. Each helper returns that
//! address as a plain `usize`.

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
    static _heap_start: u8;
    static _heap_size: u8;
    static _memory_end: u8;
}

#[inline]
pub fn text_start() -> usize {
    &raw const _text_start as usize
}
#[inline]
pub fn text_end() -> usize {
    &raw const _text_end as usize
}
#[inline]
pub fn rodata_start() -> usize {
    &raw const _rodata_start as usize
}
#[inline]
pub fn rodata_end() -> usize {
    &raw const _rodata_end as usize
}
#[inline]
pub fn data_start() -> usize {
    &raw const _data_start as usize
}
#[inline]
pub fn data_end() -> usize {
    &raw const _data_end as usize
}
#[inline]
pub fn bss_start() -> usize {
    &raw const _bss_start as usize
}
#[inline]
pub fn bss_end() -> usize {
    &raw const _bss_end as usize
}
#[inline]
pub fn stack_start() -> usize {
    &raw const _stack_start as usize
}
#[inline]
pub fn stack_end() -> usize {
    &raw const _stack_end as usize
}
#[inline]
pub fn heap_start() -> usize {
    &raw const _heap_start as usize
}
#[inline]
pub fn heap_size() -> usize {
    &raw const _heap_size as usize
}
#[inline]
pub fn memory_end() -> usize {
    &raw const _memory_end as usize
}
