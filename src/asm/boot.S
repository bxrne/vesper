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
	# Control registers, set the stack, mstatus, mepc,
	# and mtvec to return to the main function.
	la	sp, _stack_end
	# mstatus: MPP=11 (M-mode), MPIE=1, MIE=1
	li	t0, (0b11 << 11) | (1 << 7) | (1 << 3)
	csrw	mstatus, t0
	# Disable paging while we set up.
	la	t1, kmain
	csrw	mepc, t1
	la	t2, asm_trap_vector
	csrw	mtvec, t2
	# Enable software, timer and external interrupts.
	li	t3, (1 << 3) | (1 << 7) | (1 << 11)
	csrw	mie, t3
	# Set the return address so kmain has a place to come back to.
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

# Minimal trap vector stub. Replace with a real handler later.
.section .text
.global asm_trap_vector
asm_trap_vector:
	mret
