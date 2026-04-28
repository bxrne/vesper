# boot.S
# Entry point for the bootstrap hart (hart 0). Other harts park on `wfi`.

.option norvc
.section .data

.section .text.init
.global _start
_start:
	# Any hardware threads (hart) that are not bootstrapping
	# need to wait for an IPI
	csrr	t0, mhartid
	bnez	t0, 3f

	# SATP should be zero, but let's make sure
	csrw	satp, zero

	# Allow S-mode full access across the physical address space.
	# On newer QEMU/RISC-V setups, dropping to S-mode without a PMP
	# region can fault immediately on instruction fetch.
	li	t0, -1
	csrw	pmpaddr0, t0
	li	t0, 0x0f		# TOR | R | W | X
	csrw	pmpcfg0, t0
.option push
.option norelax
	la	gp, _global_pointer
.option pop

	# The BSS section is expected to be zero
	la	a0, _bss_start
	la	a1, _bss_end
	bgeu	a0, a1, 2f
1:
	sd	zero, (a0)
	addi	a0, a0, 8
	bltu	a0, a1, 1b
2:
	# Set the stack, then mret into kmain in M-mode.
	la	sp, _stack_end
	# mstatus: MPP=11 (return to M-mode), MPIE=1. Leave MIE=0 — we don't
	# have a real trap handler yet; interrupts get enabled in a later chapter.
	li	t0, (0b11 << 11) | (1 << 7)
	csrw	mstatus, t0
	la	t1, kmain
	csrw	mepc, t1
	la	t2, asm_trap_vector
	csrw	mtvec, t2
	# Park here if kmain ever returns.
	la	ra, 4f
	mret

3:
	# Non-bootstrap harts park here until we wake them.
	wfi
	j	3b

4:
	# kmain returned. Park forever.
	wfi
	j	4b

# Minimal trap vector stub. Replace with a real handler in a later chapter.
# `mtvec` requires the base to be 4-byte aligned in direct mode.
.section .text
.balign 4
.global asm_trap_vector
asm_trap_vector:
	mret

