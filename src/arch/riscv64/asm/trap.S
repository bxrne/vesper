# M-mode trap vector. mscratch holds a pointer to the kernel's TrapFrame;
# t6 is x31 (the highest-numbered GPR) which lets the save/restore loop
# stop on a clean boundary without special-casing the base register.

.altmacro
.set REG_SIZE, 8

.macro save_gp i, basereg=t6
    sd x\i, ((\i)*REG_SIZE)(\basereg)
.endm
.macro load_gp i, basereg=t6
    ld x\i, ((\i)*REG_SIZE)(\basereg)
.endm

# norvc keeps every instruction 4 bytes wide; combined with .balign 4
# this satisfies mtvec's direct-mode alignment requirement.
.option norvc
.section .text
.balign 4
.global m_trap_vector
m_trap_vector:
    # Atomic swap so the user's t6 ends up in mscratch and the frame
    # pointer ends up in t6 — without trampling either value.
    csrrw   t6, mscratch, t6

    # Save x1..x30 into frame.regs[1..=30]. x0 is hard-wired zero, no need.
    .set    i, 1
    .rept   30
        save_gp %i
        .set i, i+1
    .endr

    # x31 (== t6) is special: t6 currently holds the frame pointer, not
    # the caller's value. Stash the frame ptr in t5, swap mscratch back
    # into t6 to recover the caller's x31, then store it.
    mv      t5, t6
    csrr    t6, mscratch
    save_gp 31, t5
    csrw    mscratch, t5

    # Marshal arguments for the Rust handler.
    csrr    a0, mepc
    csrr    a1, mtval
    csrr    a2, mcause
    csrr    a3, mhartid
    csrr    a4, mstatus
    mv      a5, t5
    # Switch to the dedicated trap stack so we don't smash whatever sp
    # the trapped context was using.
    ld      sp, 520(a5)
    call    m_trap

    # m_trap returns the resume PC in a0; install it into mepc.
    csrw    mepc, a0

    csrr    t6, mscratch

    # Restore x1..x31. The 31st iteration reloads t6 itself, leaving
    # every GPR exactly as the trap entered with.
    .set    i, 1
    .rept   31
        load_gp %i
        .set i, i+1
    .endr

    mret
