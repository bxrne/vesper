use core::mem::size_of;
use core::ptr::{null_mut, read_volatile, write_volatile};
use core::sync::atomic::{Ordering, fence};

use crate::arch::riscv64::trap;
use crate::drivers::virtio::spec::{
    AvailRing, Descriptor, Header, MmioOffsets, Request, UsedRing, VIRTIO_ACKNOWLEDGE,
    VIRTIO_BLK_F_RO, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, VIRTIO_DESC_F_NEXT, VIRTIO_DESC_F_WRITE,
    VIRTIO_DRIVER, VIRTIO_DRIVER_OK, VIRTIO_FAILED, VIRTIO_FEATURES_OK, VIRTIO_MMIO_IRQ_BASE,
    VIRTIO_RING_SIZE, align_up, virtqueue_avail_bytes, virtqueue_desc_bytes, virtqueue_used_bytes,
};
use crate::mm::alloc::page_frame::{PAGE_SIZE, allocate, deallocate, zallocate};
use crate::print;

const MAX_VIRTIO_DEVICES: usize = 8;
const STATUS_PENDING: u8 = 0xff;

#[derive(Copy, Clone)]
pub enum DeviceType {
    Block,
    Entropy,
    Unknown,
}

#[derive(Copy, Clone)]
pub struct VirtioDevice {
    pub kind: DeviceType,
}

#[derive(Copy, Clone)]
struct QueueState {
    desc: *mut Descriptor,
    avail: *mut AvailRing,
    used: *mut UsedRing,
    free: [bool; VIRTIO_RING_SIZE],
    req_head: [*mut Request; VIRTIO_RING_SIZE],
}

impl QueueState {
    const fn empty() -> Self {
        Self {
            desc: null_mut(),
            avail: null_mut(),
            used: null_mut(),
            free: [true; VIRTIO_RING_SIZE],
            req_head: [null_mut(); VIRTIO_RING_SIZE],
        }
    }
}

pub struct BlockDevice {
    dev: *mut u32,
    queue: QueueState,
    ack_used_idx: u16,
    read_only: bool,
}

impl BlockDevice {
    const fn empty() -> Self {
        Self {
            dev: null_mut(),
            queue: QueueState::empty(),
            ack_used_idx: 0,
            read_only: false,
        }
    }

    #[inline]
    fn irq(&self) -> Option<u32> {
        if self.dev.is_null() {
            None
        } else {
            let slot = ((self.dev as usize) - super::spec::MMIO_VIRTIO_START) >> 12;
            Some(VIRTIO_MMIO_IRQ_BASE + slot as u32)
        }
    }
}

static mut BLOCK_DEVICES: [BlockDevice; MAX_VIRTIO_DEVICES] =
    [const { BlockDevice::empty() }; MAX_VIRTIO_DEVICES];
static mut VIRTIO_DEVICES: [Option<VirtioDevice>; MAX_VIRTIO_DEVICES] =
    [const { None }; MAX_VIRTIO_DEVICES];

#[inline]
fn dev_slot_from_ptr(ptr: *mut u32) -> usize {
    ((ptr as usize) - super::spec::MMIO_VIRTIO_START) >> 12
}

#[inline]
fn alloc_request() -> Option<*mut Request> {
    let page = allocate(1)?;
    Some(page.as_ptr() as *mut Request)
}

unsafe fn free_request(req: *mut Request) {
    if req.is_null() {
        return;
    }
    let ptr = core::ptr::NonNull::new(req as *mut u8).expect("request pointer was null");
    unsafe { deallocate(ptr) };
}

fn alloc_desc_chain(queue: &mut QueueState) -> Option<[u16; 3]> {
    let mut out = [0u16; 3];
    let mut found = 0usize;

    for i in 0..VIRTIO_RING_SIZE {
        if queue.free[i] {
            queue.free[i] = false;
            out[found] = i as u16;
            found += 1;
            if found == 3 {
                return Some(out);
            }
        }
    }

    for idx in out.iter().take(found) {
        queue.free[*idx as usize] = true;
    }
    None
}

fn free_desc_chain(queue: &mut QueueState, head: u16) {
    let mut idx = head as usize;
    loop {
        queue.free[idx] = true;
        let desc = unsafe { read_volatile(queue.desc.add(idx)) };
        if desc.flags & VIRTIO_DESC_F_NEXT == 0 {
            break;
        }
        idx = usize::from(desc.next);
    }
}

#[inline]
fn mmio_read32(ptr: *mut u32, off: MmioOffsets) -> Option<u32> {
    trap::set_mmio_guard(true);
    trap::clear_access_fault();
    let v = unsafe { ptr.add(off.scale32()).read_volatile() };
    let faulted = trap::take_access_fault();
    trap::set_mmio_guard(false);
    if faulted { None } else { Some(v) }
}

#[inline]
fn mmio_write32(ptr: *mut u32, off: MmioOffsets, val: u32) -> bool {
    trap::set_mmio_guard(true);
    trap::clear_access_fault();
    unsafe { ptr.add(off.scale32()).write_volatile(val) };
    let ok = !trap::take_access_fault();
    trap::set_mmio_guard(false);
    ok
}

pub fn setup_block_device(ptr: *mut u32) -> bool {
    unsafe {
        let slot = dev_slot_from_ptr(ptr);

        if !mmio_write32(ptr, MmioOffsets::Status, 0) {
            return false;
        }

        let mut status_bits = VIRTIO_ACKNOWLEDGE;
        if !mmio_write32(ptr, MmioOffsets::Status, status_bits) {
            return false;
        }

        status_bits |= VIRTIO_DRIVER;
        if !mmio_write32(ptr, MmioOffsets::Status, status_bits) {
            return false;
        }

        let Some(host_features) = mmio_read32(ptr, MmioOffsets::HostFeatures) else {
            return false;
        };
        let guest_features = host_features & !(1u32 << VIRTIO_BLK_F_RO);
        let read_only = (host_features & (1u32 << VIRTIO_BLK_F_RO)) != 0;

        if !mmio_write32(ptr, MmioOffsets::GuestFeatures, guest_features) {
            return false;
        }

        status_bits |= VIRTIO_FEATURES_OK;
        if !mmio_write32(ptr, MmioOffsets::Status, status_bits) {
            return false;
        }

        let Some(status_ok) = mmio_read32(ptr, MmioOffsets::Status) else {
            return false;
        };
        if (status_ok & VIRTIO_FEATURES_OK) == 0 {
            print!("features fail...");
            let _ = mmio_write32(ptr, MmioOffsets::Status, VIRTIO_FAILED);
            return false;
        }

        if !mmio_write32(ptr, MmioOffsets::QueueSel, 0) {
            return false;
        }
        let Some(qnmax) = mmio_read32(ptr, MmioOffsets::QueueNumMax) else {
            return false;
        };
        if qnmax < VIRTIO_RING_SIZE as u32 {
            print!("queue size fail...");
            let _ = mmio_write32(ptr, MmioOffsets::Status, VIRTIO_FAILED);
            return false;
        }

        if !mmio_write32(ptr, MmioOffsets::QueueNum, VIRTIO_RING_SIZE as u32) {
            return false;
        }
        if !mmio_write32(ptr, MmioOffsets::QueueAlign, PAGE_SIZE as u32) {
            return false;
        }
        if !mmio_write32(ptr, MmioOffsets::GuestPageSize, PAGE_SIZE as u32) {
            return false;
        }

        let desc_bytes = virtqueue_desc_bytes();
        let avail_bytes = virtqueue_avail_bytes();
        let used_bytes = virtqueue_used_bytes();
        let used_offset = align_up(desc_bytes + avail_bytes, PAGE_SIZE);
        let queue_bytes = used_offset + used_bytes;
        let queue_pages = align_up(queue_bytes, PAGE_SIZE) / PAGE_SIZE;

        let Some(queue_page) = zallocate(queue_pages) else {
            print!("queue alloc fail...");
            let _ = mmio_write32(ptr, MmioOffsets::Status, VIRTIO_FAILED);
            return false;
        };

        let queue_mem = queue_page.as_ptr();
        let desc = queue_mem as *mut Descriptor;
        let avail = queue_mem.add(desc_bytes) as *mut AvailRing;
        let used = queue_mem.add(used_offset) as *mut UsedRing;

        if !mmio_write32(
            ptr,
            MmioOffsets::QueuePfn,
            (queue_mem as u32) / PAGE_SIZE as u32,
        ) {
            return false;
        }

        let mut qstate = QueueState::empty();
        qstate.desc = desc;
        qstate.avail = avail;
        qstate.used = used;

        BLOCK_DEVICES[slot] = BlockDevice {
            dev: ptr,
            queue: qstate,
            ack_used_idx: 0,
            read_only,
        };

        VIRTIO_DEVICES[slot] = Some(VirtioDevice {
            kind: DeviceType::Block,
        });

        status_bits |= VIRTIO_DRIVER_OK;
        if !mmio_write32(ptr, MmioOffsets::Status, status_bits) {
            return false;
        }

        true
    }
}

pub fn register_entropy_device(ptr: *mut u32) {
    unsafe {
        VIRTIO_DEVICES[dev_slot_from_ptr(ptr)] = Some(VirtioDevice {
            kind: DeviceType::Entropy,
        });
    }
}

pub fn register_unknown_device(ptr: *mut u32) {
    unsafe {
        VIRTIO_DEVICES[dev_slot_from_ptr(ptr)] = Some(VirtioDevice {
            kind: DeviceType::Unknown,
        });
    }
}

pub fn block_read(dev: usize, buffer: *mut u8, size: u32, offset: u64) -> bool {
    block_op(dev, buffer, size, offset, false)
}

pub fn block_write(dev: usize, buffer: *mut u8, size: u32, offset: u64) -> bool {
    block_op(dev, buffer, size, offset, true)
}

fn block_op(dev: usize, buffer: *mut u8, size: u32, offset: u64, write: bool) -> bool {
    unsafe {
        if dev >= MAX_VIRTIO_DEVICES {
            return false;
        }
        let base = (&raw mut BLOCK_DEVICES).cast::<BlockDevice>();
        let bdev = &mut *base.add(dev);
        if bdev.dev.is_null() {
            return false;
        }
        if bdev.read_only && write {
            return false;
        }

        let Some(req) = alloc_request() else {
            return false;
        };
        (*req).header = Header {
            blktype: if write {
                VIRTIO_BLK_T_OUT
            } else {
                VIRTIO_BLK_T_IN
            },
            reserved: 0,
            sector: offset / 512,
        };
        (*req).data = buffer;
        (*req).status = STATUS_PENDING;

        let Some([head, data, stat]) = alloc_desc_chain(&mut bdev.queue) else {
            free_request(req);
            return false;
        };

        let head_desc = Descriptor {
            addr: (&(*req).header as *const Header) as u64,
            len: size_of::<Header>() as u32,
            flags: VIRTIO_DESC_F_NEXT,
            next: data,
        };

        let data_desc = Descriptor {
            addr: buffer as u64,
            len: size,
            flags: VIRTIO_DESC_F_NEXT | if write { 0 } else { VIRTIO_DESC_F_WRITE },
            next: stat,
        };

        let status_desc = Descriptor {
            addr: (&raw mut (*req).status) as u64,
            len: 1,
            flags: VIRTIO_DESC_F_WRITE,
            next: 0,
        };

        write_volatile(bdev.queue.desc.add(head as usize), head_desc);
        write_volatile(bdev.queue.desc.add(data as usize), data_desc);
        write_volatile(bdev.queue.desc.add(stat as usize), status_desc);
        bdev.queue.req_head[head as usize] = req;

        let avail_idx = read_volatile(&(*bdev.queue.avail).idx);
        write_volatile(
            &mut (*bdev.queue.avail).ring[(avail_idx as usize) % VIRTIO_RING_SIZE],
            head,
        );
        write_volatile(&mut (*bdev.queue.avail).idx, avail_idx.wrapping_add(1));

        fence(Ordering::SeqCst);
        bdev.dev
            .add(MmioOffsets::QueueNotify.scale32())
            .write_volatile(0);

        // Keep chapter-9 synchronous call semantics while still supporting
        // interrupt-driven completion via `pending`.
        loop {
            pending(bdev);
            if (*req).status != STATUS_PENDING {
                break;
            }
        }

        let ok = (*req).status == 0;
        free_request(req);
        ok
    }
}

pub fn pending_by_irq(irq: u32) -> bool {
    let slot = irq.checked_sub(VIRTIO_MMIO_IRQ_BASE).map(|v| v as usize);
    let Some(slot) = slot else {
        return false;
    };
    if slot >= MAX_VIRTIO_DEVICES {
        return false;
    }

    unsafe {
        let bdev = &mut BLOCK_DEVICES[slot];
        if bdev.dev.is_null() {
            return false;
        }

        let isr = bdev
            .dev
            .add(MmioOffsets::InterruptStatus.scale32())
            .read_volatile();
        if isr == 0 {
            return false;
        }

        pending(bdev);
        bdev.dev
            .add(MmioOffsets::InterruptAck.scale32())
            .write_volatile(isr & 0x3);

        true
    }
}

fn pending(bd: &mut BlockDevice) {
    unsafe {
        while bd.ack_used_idx != read_volatile(&(*bd.queue.used).idx) {
            let used_idx = (bd.ack_used_idx as usize) % VIRTIO_RING_SIZE;
            let elem = read_volatile(&(*bd.queue.used).ring[used_idx]);
            bd.ack_used_idx = bd.ack_used_idx.wrapping_add(1);

            let head = elem.id as usize;
            if head >= VIRTIO_RING_SIZE {
                continue;
            }

            let req = bd.queue.req_head[head];
            bd.queue.req_head[head] = null_mut();
            free_desc_chain(&mut bd.queue, head as u16);
            if req.is_null() {
                continue;
            }
        }
    }
}

pub fn first_block_device() -> Option<usize> {
    unsafe {
        let base = (&raw const BLOCK_DEVICES).cast::<BlockDevice>();
        for idx in 0..MAX_VIRTIO_DEVICES {
            let dev = &*base.add(idx);
            if !dev.dev.is_null() {
                return Some(idx);
            }
        }
    }
    None
}

pub fn configured_irqs() -> [Option<u32>; MAX_VIRTIO_DEVICES] {
    let mut out = [None; MAX_VIRTIO_DEVICES];
    unsafe {
        let base = (&raw const BLOCK_DEVICES).cast::<BlockDevice>();
        for (i, out_slot) in out.iter_mut().enumerate() {
            let dev = &*base.add(i);
            *out_slot = dev.irq();
        }
    }
    out
}
