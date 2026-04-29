use crate::arch::riscv64::trap;
use crate::drivers::virtio::device::{
    register_entropy_device, register_unknown_device, setup_block_device,
};
use crate::drivers::virtio::spec::{
    MMIO_VIRTIO_END, MMIO_VIRTIO_MAGIC, MMIO_VIRTIO_START, MMIO_VIRTIO_STRIDE, MmioOffsets,
    VIRTIO_DEVICE_BLOCK, VIRTIO_DEVICE_ENTROPY,
};
use crate::{print, println};

#[inline]
fn mmio_read32(ptr: *mut u32, off: MmioOffsets) -> Option<u32> {
    trap::set_mmio_guard(true);
    trap::clear_access_fault();
    let val = unsafe { ptr.add(off.scale32()).read_volatile() };
    let faulted = trap::take_access_fault();
    trap::set_mmio_guard(false);
    if faulted { None } else { Some(val) }
}

pub fn probe() {
    for addr in (MMIO_VIRTIO_START..=MMIO_VIRTIO_END).step_by(MMIO_VIRTIO_STRIDE) {
        print!("Virtio probing 0x{:08x}...", addr);
        let ptr = addr as *mut u32;

        let Some(magicvalue) = mmio_read32(ptr, MmioOffsets::MagicValue) else {
            println!("fault.");
            continue;
        };
        if magicvalue != MMIO_VIRTIO_MAGIC {
            println!("not virtio.");
            continue;
        }

        let Some(deviceid) = mmio_read32(ptr, MmioOffsets::DeviceId) else {
            println!("fault.");
            continue;
        };

        if deviceid == 0 {
            println!("not connected.");
            continue;
        }

        match deviceid {
            VIRTIO_DEVICE_BLOCK => {
                print!("block device...");
                if setup_block_device(ptr) {
                    println!("setup succeeded!");
                } else {
                    println!("setup failed.");
                }
            }
            VIRTIO_DEVICE_ENTROPY => {
                print!("entropy device...");
                register_entropy_device(ptr);
                println!("discovered.");
            }
            _ => {
                register_unknown_device(ptr);
                println!("unknown device type {}.", deviceid);
            }
        }
    }
}
