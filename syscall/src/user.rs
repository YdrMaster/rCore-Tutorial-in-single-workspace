use crate::*;
use core::arch::asm;

#[inline]
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    unsafe { syscall3(ID_WRITE, fd, buffer.as_ptr() as _, buffer.len()) }
}

#[inline]
pub fn sys_exit(exit_code: i32) -> isize {
    unsafe { syscall1(ID_EXIT, exit_code as _) }
}

#[inline(always)]
unsafe fn syscall0(id: usize) -> isize {
    let ret: isize;
    asm!("ecall",
        out("a0") ret,
        in("a7") id,
    );
    ret
}

#[inline(always)]
unsafe fn syscall1(id: usize, a0: usize) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a7") id,
    );
    ret
}

#[inline(always)]
unsafe fn syscall2(id: usize, a0: usize, a1: usize) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a1") a1,
        in("a7") id,
    );
    ret
}

#[inline(always)]
unsafe fn syscall3(id: usize, a0: usize, a1: usize, a2: usize) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a1") a1,
        in("a2") a2,
        in("a7") id,
    );
    ret
}

#[inline(always)]
unsafe fn syscall4(id: usize, a0: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a1") a1,
        in("a2") a2,
        in("a3") a3,
        in("a7") id,
    );
    ret
}

#[inline(always)]
unsafe fn syscall5(id: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a1") a1,
        in("a2") a2,
        in("a3") a3,
        in("a4") a4,
        in("a7") id,
    );
    ret
}

#[inline(always)]
unsafe fn syscall6(
    id: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a1") a1,
        in("a2") a2,
        in("a3") a3,
        in("a4") a4,
        in("a5") a5,
        in("a7") id,
    );
    ret
}

unsafe fn syscall7(
    id: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
) -> isize {
    let ret: isize;
    asm!("ecall",
        inlateout("a0") a0 => ret,
        in("a1") a1,
        in("a2") a2,
        in("a3") a3,
        in("a4") a4,
        in("a5") a5,
        in("a6") a6,
        in("a7") id,
    );
    ret
}
