use core::arch::asm;
use core::hint::black_box;

use crate::arch;
use crate::drivers::plic;
use crate::drivers::uart::{UART_BASE, Uart};
use crate::drivers::virtio;
use crate::exec::elf;
use crate::fs::minix3;
use crate::linker;
use crate::mm::alloc::page_frame::{self, PAGE_SIZE};
use crate::mm::paging::sv39::map::id_map_range;
use crate::mm::paging::sv39::types::{PteFlags, Table};
use crate::process;
use crate::{print, println};

#[inline]
fn align_down(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    addr & !(align - 1)
}

#[inline]
fn align_up(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    addr.next_multiple_of(align)
}

/// Identity-map `[start, end)` rounded out to page boundaries. Rounding
/// matters because linker sections are byte-aligned but the MMU only
/// understands whole pages — over-mapping a few bytes is harmless.
fn map_section(root: &mut Table, start: usize, end: usize, flags: PteFlags) {
    id_map_range(
        root,
        align_down(start, PAGE_SIZE),
        align_up(end, PAGE_SIZE),
        flags,
    );
}

/// M-mode entry point reached from `_start` (boot.S) once the BSS is
/// zeroed and the stack is set. Sets up the UART, builds an identity
/// page table, installs the trap handler, configures the PLIC, then
/// `mret`s into [`skmain`] in S-mode.
#[unsafe(no_mangle)]
pub extern "C" fn kinit() -> ! {
    let uart = Uart::new(UART_BASE);
    uart.init();

    println!();
    println!("vesper booted successfully!");

    page_frame::init();

    let root_page = page_frame::zallocate(1).expect("OOM allocating root page table");
    let root_table_addr = root_page.as_ptr() as usize;
    // The freshly-zeroed page is the level-2 table the MMU will walk.
    let root = unsafe { &mut *(root_table_addr as *mut Table) };

    // ACCESSED/DIRTY are pre-set so the hardware never has to fault to
    // update them — simpler than implementing the A/D update trap.
    let rx = PteFlags::READ | PteFlags::EXECUTE | PteFlags::ACCESSED;
    let ro = PteFlags::READ | PteFlags::ACCESSED;
    let rw = PteFlags::READ | PteFlags::WRITE | PteFlags::ACCESSED | PteFlags::DIRTY;

    map_section(root, linker::text_start(), linker::text_end(), rx);
    map_section(root, linker::rodata_start(), linker::rodata_end(), ro);
    map_section(root, linker::data_start(), linker::data_end(), rw);
    map_section(root, linker::bss_start(), linker::bss_end(), rw);
    map_section(root, linker::stack_start(), linker::stack_end(), rw);
    // The heap lives between `_heap_start` and `_memory_end`; mapping
    // the rest of RAM keeps the page-frame allocator's metadata reachable.
    map_section(root, linker::heap_start(), linker::memory_end(), rw);

    // UART MMIO is outside the linker-defined RAM window, so it needs an
    // explicit mapping or the first `print!` after enabling paging faults.
    let uart_base = align_down(UART_BASE, PAGE_SIZE);
    map_section(root, uart_base, uart_base + PAGE_SIZE, rw);
    map_section(
        root,
        virtio::spec::MMIO_VIRTIO_START,
        virtio::spec::MMIO_VIRTIO_END + virtio::spec::MMIO_VIRTIO_STRIDE,
        rw,
    );

    let satp = arch::make_satp_sv39(root_table_addr);

    // Install the M-mode trap handler before configuring the PLIC, so
    // any spurious interrupt that does fire lands in real Rust code
    // rather than the boot.S stub.
    arch::install_trap_handler(satp);

    // Route the UART through the PLIC at priority 1 with the global
    // threshold lowered so anything at priority >=1 gets through.
    plic::set_threshold(0);
    plic::enable(plic::UART0_IRQ);
    plic::set_priority(plic::UART0_IRQ, 1);
    virtio::bus::probe();
    for irq in virtio::device::configured_irqs().into_iter().flatten() {
        plic::enable(irq);
        plic::set_priority(irq, 1);
    }

    arch::enable_interrupts();

    arch::enter_sv39(satp, skmain)
}

/// S-mode entry point. Paging is live, interrupts are unmasked, and
/// every device input is delivered through the M-mode trap handler.
/// Spawn the init kernel thread, then idle — the next timer tick will
/// preempt this idle path and hand the CPU to init.
#[unsafe(no_mangle)]
pub extern "C" fn skmain() -> ! {
    println!();
    println!("paging enabled (Sv39), now in S-mode");
    println!("interrupts enabled — type to echo:");
    println!("Testing block driver.");
    if let Some(dev) = virtio::device::first_block_device() {
        if let Some(buffer) = page_frame::allocate(1) {
            let ptr = buffer.as_ptr();
            let ok = virtio::device::block_read(dev, ptr, 512, 0);
            if ok {
                for i in 0..48usize {
                    let b = unsafe { ptr.add(i).read() };
                    print!(" {:02x}", b);
                    if (i + 1) % 24 == 0 {
                        println!();
                    }
                }
            } else {
                println!("block read failed");
            }
            unsafe { page_frame::deallocate(buffer) };
        } else {
            println!("failed to allocate test buffer");
        }
    } else {
        println!("no block device discovered");
    }
    println!("Block driver done");

    test_minix3();

    process::spawn_kernel(init_process).expect("failed to spawn init");
    println!("spawned init kernel thread, awaiting first timer tick");

    loop {
        arch::wfi();
    }
}

/// PID 1: a placeholder kernel thread that periodically issues a test
/// syscall. Real workloads will replace this once user-mode and ELF
/// loading land.
fn init_process() -> ! {
    let mut i: usize = 0;
    loop {
        // black_box prevents the optimiser from collapsing the loop.
        i = black_box(i).wrapping_add(1);
        if i > 1_000_000 {
            unsafe { syscall(1) };
            i = 0;
        }
    }
}

/// Issue an `ecall` with `num` in `a0`. Returns the kernel's value of
/// `a0` after the call (currently unused — placeholder for the real
/// syscall ABI).
#[inline]
unsafe fn syscall(num: usize) -> usize {
    let ret;
    unsafe {
        asm!("ecall", inlateout("a0") num => ret, options(nostack));
    }
    ret
}

/// Inode/size of a userland ELF the build system might have copied
/// onto the disk. If the disk hasn't been formatted with mkfs.minix
/// (the default `hdd.dsk` is just a 32 MiB hole) we skip silently.
const ELF_TEST_INODE: u32 = 0;
const ELF_TEST_MAX_SIZE: usize = 64 * 1024;

/// Probe `dev 0` for a Minix 3 filesystem. If we find one, dump the
/// superblock summary and walk the root directory; this exercises the
/// chapter-10 reader against a real disk image.
fn test_minix3() {
    let Some(dev) = virtio::device::first_block_device() else {
        return;
    };

    println!();
    println!("Probing device {} for Minix 3 filesystem...", dev);
    let fs = match minix3::Fs::mount(dev) {
        Ok(fs) => fs,
        Err(minix3::FsError::BadMagic) => {
            println!(
                "  no Minix 3 filesystem (magic mismatch). Format with `mkfs.minix -3` to test."
            );
            return;
        }
        Err(e) => {
            println!("  mount failed: {:?}", e);
            return;
        }
    };

    println!(
        "  superblock: ninodes={} zones={} block_size={} first_data_zone={}",
        fs.sb.ninodes, fs.sb.zones, fs.sb.block_size, fs.sb.first_data_zone
    );

    let root = match fs.read_inode(minix3::ROOT_INODE) {
        Ok(i) => i,
        Err(e) => {
            println!("  read root inode failed: {:?}", e);
            return;
        }
    };
    println!(
        "  root inode: mode=0o{:o} size={} zones[0]={}",
        root.mode, root.size, root.zones[0]
    );

    if !root.is_dir() {
        println!("  root inode is not a directory; aborting");
        return;
    }

    list_dir(&fs, &root);

    if ELF_TEST_INODE != 0 {
        test_elf(&fs);
    }
}

/// Read up to one block of directory entries and print each name.
/// Bigger directories are truncated; the chapter-10 reader can handle
/// them, this just keeps boot output short.
fn list_dir(fs: &minix3::Fs, dir: &minix3::Inode) {
    let buf_pages = page_frame::zallocate(1).expect("OOM listing dir");
    // SAFETY: one full page is at least BLOCK_SIZE bytes, plenty for
    // BLOCK_SIZE / DIRENT_SIZE = 16 entries.
    let slice =
        unsafe { core::slice::from_raw_parts_mut(buf_pages.as_ptr(), minix3::BLOCK_SIZE as usize) };
    let read = match fs.read_file(dir, 0, slice) {
        Ok(n) => n,
        Err(e) => {
            println!("  list_dir failed: {:?}", e);
            unsafe { page_frame::deallocate(buf_pages) };
            return;
        }
    };
    let entries = read / minix3::DIRENT_SIZE;
    println!("  /  ({} entries):", entries);
    for i in 0..entries {
        let off = i * minix3::DIRENT_SIZE;
        let de = unsafe { &*(slice.as_ptr().add(off) as *const minix3::DirEntry) };
        if de.inode == 0 {
            continue;
        }
        println!("    inode {:>4}  {}", de.inode, de.name_str());
    }
    unsafe { page_frame::deallocate(buf_pages) };
}

/// Read the ELF at `ELF_TEST_INODE`, validate it, and dump the
/// program-header summary. This is the chapter-11 entry point — once a
/// real userland binary lives on the disk we can swap the print loop
/// for a `process::spawn_user` call.
fn test_elf(fs: &minix3::Fs) {
    println!("  loading ELF at inode {}...", ELF_TEST_INODE);
    let inode = match fs.read_inode(ELF_TEST_INODE) {
        Ok(i) => i,
        Err(e) => {
            println!("    read inode failed: {:?}", e);
            return;
        }
    };
    let want = core::cmp::min(inode.size as usize, ELF_TEST_MAX_SIZE);
    let pages = want.div_ceil(PAGE_SIZE);
    let Some(buf) = page_frame::zallocate(pages) else {
        println!("    OOM allocating ELF buffer");
        return;
    };
    let slice = unsafe { core::slice::from_raw_parts_mut(buf.as_ptr(), want) };
    let got = match fs.read_file(&inode, 0, slice) {
        Ok(n) => n,
        Err(e) => {
            println!("    read_file failed: {:?}", e);
            unsafe { page_frame::deallocate(buf) };
            return;
        }
    };
    if got != want {
        println!("    short read: wanted {} got {}", want, got);
        unsafe { page_frame::deallocate(buf) };
        return;
    }
    match elf::parse(&slice[..got]) {
        Ok(hdr) => {
            println!(
                "    ELF ok: entry=0x{:x} phnum={} machine=0x{:x}",
                hdr.entry_addr, hdr.phnum, hdr.machine
            );
            for (i, ph) in elf::program_headers(&slice[..got], hdr).enumerate() {
                if ph.seg_type != elf::PT_LOAD {
                    continue;
                }
                println!(
                    "    PH#{}: vaddr=0x{:x} filesz={} memsz={} flags=0x{:x}",
                    i, ph.vaddr, ph.filesz, ph.memsz, ph.flags
                );
            }
        }
        Err(e) => println!("    elf parse: {:?}", e),
    }
    unsafe { page_frame::deallocate(buf) };
}
