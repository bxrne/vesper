# Bootstrap entry. QEMU drops every hart at `_start` simultaneously, so
# only hart 0 continues; the rest park on `wfi` until the kernel decides
# to bring them up via IPIs.

.option norvc
.section .data

.section .text.init
.global _start
_start:
	csrr	t0, mhartid
	bnez	t0, 3f

	# Paging is unconfigured at reset, but be explicit — a stale satp
	# from a prior boot would pin Sv-mode on before kinit is ready.
	csrw	satp, zero

	# Newer QEMU/RISC-V cores enforce PMP even from M-mode in some
	# configurations: without an "allow all" PMP region the first
	# fetch in S-mode faults. One TOR entry covering the whole
	# address space with R|W|X is the simplest unblocker.
	li	t0, -1
	csrw	pmpaddr0, t0
	li	t0, 0x0f
	csrw	pmpcfg0, t0

	# Disable linker relaxation around the gp load: the linker would
	# otherwise rewrite this to a gp-relative form, which is circular
	# (gp isn't valid yet).
.option push
.option norelax
	la	gp, _global_pointer
.option pop

	# BSS isn't loaded from the ELF, so the runtime has to zero it
	# before any Rust code touches a static.
	la	a0, _bss_start
	la	a1, _bss_end
	bgeu	a0, a1, 2f
1:
	sd	zero, (a0)
	addi	a0, a0, 8
	bltu	a0, a1, 1b
2:
	la	sp, _stack_end

	# Set up an `mret` into kinit running in M-mode. MPP=11 keeps us
	# in M-mode after the return; MPIE=1 so the prior MIE state is
	# restored. MIE itself stays 0 — no trap handler is installed.
	li	t0, (0b11 << 11) | (1 << 7)
	csrw	mstatus, t0
	la	t1, kinit
	csrw	mepc, t1
	la	t2, asm_trap_vector
	csrw	mtvec, t2
	# Catch a stray `ret` from kinit so it parks instead of jumping
	# off into garbage.
	la	ra, 4f
	mret

3:
	# Secondary harts wait here. They'll be released once SMP and
	# IPIs are wired up.
	wfi
	j	3b

4:
	# kinit is `-> !`, so reaching here means something is very wrong.
	wfi
	j	4b

# Stub trap vector. mtvec needs a 4-byte-aligned target in direct mode;
# returning straight away is fine until proper trap handling lands.
.section .text
.balign 4
.global asm_trap_vector
asm_trap_vector:
	mret
