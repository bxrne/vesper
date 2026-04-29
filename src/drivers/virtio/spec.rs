use core::mem::size_of;

pub const MMIO_VIRTIO_START: usize = 0x1000_1000;
pub const MMIO_VIRTIO_END: usize = 0x1000_8000;
pub const MMIO_VIRTIO_STRIDE: usize = 0x1000;
pub const MMIO_VIRTIO_MAGIC: u32 = 0x7472_6976;
pub const VIRTIO_MMIO_IRQ_BASE: u32 = 1;

pub const VIRTIO_RING_SIZE: usize = 8;

pub const VIRTIO_DESC_F_NEXT: u16 = 1;
pub const VIRTIO_DESC_F_WRITE: u16 = 2;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;
pub const VIRTIO_BLK_F_RO: u32 = 5;

pub const VIRTIO_ACKNOWLEDGE: u32 = 1;
pub const VIRTIO_DRIVER: u32 = 2;
pub const VIRTIO_DRIVER_OK: u32 = 4;
pub const VIRTIO_FEATURES_OK: u32 = 8;
pub const VIRTIO_FAILED: u32 = 128;

pub const VIRTIO_DEVICE_BLOCK: u32 = 2;
pub const VIRTIO_DEVICE_ENTROPY: u32 = 4;

#[repr(usize)]
#[derive(Copy, Clone)]
pub enum MmioOffsets {
    MagicValue = 0x000,
    Version = 0x004,
    DeviceId = 0x008,
    VendorId = 0x00c,
    HostFeatures = 0x010,
    HostFeaturesSel = 0x014,
    GuestFeatures = 0x020,
    GuestFeaturesSel = 0x024,
    GuestPageSize = 0x028,
    QueueSel = 0x030,
    QueueNumMax = 0x034,
    QueueNum = 0x038,
    QueueAlign = 0x03c,
    QueuePfn = 0x040,
    QueueNotify = 0x050,
    InterruptStatus = 0x060,
    InterruptAck = 0x064,
    Status = 0x070,
    Config = 0x100,
}

impl MmioOffsets {
    #[inline]
    pub const fn val(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn scaled(self, scale: usize) -> usize {
        self.val() / scale
    }

    #[inline]
    pub const fn scale32(self) -> usize {
        self.scaled(4)
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Descriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
pub struct AvailRing {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; VIRTIO_RING_SIZE],
    pub used_event: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct UsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
pub struct UsedRing {
    pub flags: u16,
    pub idx: u16,
    pub ring: [UsedElem; VIRTIO_RING_SIZE],
    pub avail_event: u16,
}

#[repr(C)]
pub struct Header {
    pub blktype: u32,
    pub reserved: u32,
    pub sector: u64,
}

#[repr(C)]
pub struct Request {
    pub header: Header,
    pub data: *mut u8,
    pub status: u8,
}

#[inline]
pub const fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

#[inline]
pub const fn virtqueue_desc_bytes() -> usize {
    size_of::<Descriptor>() * VIRTIO_RING_SIZE
}

#[inline]
pub const fn virtqueue_avail_bytes() -> usize {
    size_of::<AvailRing>()
}

#[inline]
pub const fn virtqueue_used_bytes() -> usize {
    size_of::<UsedRing>()
}
