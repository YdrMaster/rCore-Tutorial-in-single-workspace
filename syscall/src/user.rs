use crate::{ClockId, SignalAction, SignalNo, SyscallId, TimeSpec};
use bitflags::*;
use native::*;

/// see <https://man7.org/linux/man-pages/man2/write.2.html>.
#[inline]
pub fn write(fd: usize, buffer: &[u8]) -> isize {
    unsafe { syscall3(SyscallId::WRITE, fd, buffer.as_ptr() as _, buffer.len()) }
}

#[inline]
pub fn read(fd: usize, buffer: &[u8]) -> isize {
    unsafe { syscall3(SyscallId::READ, fd, buffer.as_ptr() as _, buffer.len()) }
}

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

#[inline]
pub fn open(path: &str, flags: OpenFlags) -> isize {
    unsafe {
        syscall2(
            SyscallId::OPENAT,
            path.as_ptr() as usize,
            flags.bits as usize,
        )
    }
}

#[inline]
pub fn close(fd: usize) -> isize {
    unsafe { syscall1(SyscallId::CLOSE, fd) }
}

/// see <https://man7.org/linux/man-pages/man2/exit.2.html>.
#[inline]
pub fn exit(exit_code: i32) -> isize {
    unsafe { syscall1(SyscallId::EXIT, exit_code as _) }
}

/// see <https://man7.org/linux/man-pages/man2/sched_yield.2.html>.
#[inline]
pub fn sched_yield() -> isize {
    unsafe { syscall0(SyscallId::SCHED_YIELD) }
}

/// see <https://man7.org/linux/man-pages/man2/clock_gettime.2.html>.
#[inline]
pub fn clock_gettime(clockid: ClockId, tp: *mut TimeSpec) -> isize {
    unsafe { syscall2(SyscallId::CLOCK_GETTIME, clockid.0, tp as _) }
}

pub fn fork() -> isize {
    unsafe { syscall0(SyscallId::CLONE) }
}

pub fn exec(path: &str) -> isize {
    unsafe { syscall2(SyscallId::EXECVE, path.as_ptr() as usize, path.len()) }
}

pub fn wait(exit_code_ptr: *mut i32) -> isize {
    loop {
        match unsafe { syscall2(SyscallId::WAIT4, usize::MAX, exit_code_ptr as usize) } {
            -2 => {
                sched_yield();
            }
            exit_pid => return exit_pid,
        }
    }
}

pub fn waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    loop {
        match unsafe { syscall2(SyscallId::WAIT4, pid as usize, exit_code_ptr as usize) } {
            -2 => {
                sched_yield();
            }
            exit_pid => return exit_pid,
        }
    }
}

pub fn getpid() -> isize {
    unsafe { syscall0(SyscallId::GETPID) }
}

#[inline]
pub fn kill(pid: isize, signum: SignalNo) -> isize {
    unsafe { syscall2(SyscallId::KILL, pid as _, signum as _) }
}

#[inline]
pub fn sigaction(
    signum: SignalNo,
    action: *const SignalAction,
    old_action: *const SignalAction,
) -> isize {
    unsafe {
        syscall3(
            SyscallId::RT_SIGACTION,
            signum as _,
            action as _,
            old_action as _,
        )
    }
}

#[inline]
pub fn sigprocmask(mask: usize) -> isize {
    unsafe { syscall1(SyscallId::RT_SIGPROCMASK, mask) }
}

#[inline]
pub fn sigreturn() -> isize {
    unsafe { syscall0(SyscallId::RT_SIGRETURN) }
}

#[inline]
pub fn thread_create(entry: usize, arg: usize) -> isize {
    unsafe { syscall2(SyscallId::THREAD_CREATE, entry, arg) }
}

#[inline]
pub fn gettid() -> isize {
    unsafe { syscall0(SyscallId::GETTID) }
}

#[inline]
pub fn waittid(tid: usize) -> isize {
    loop {
        match unsafe { syscall1(SyscallId::WAITID, tid) } {
            -2 => {
                sched_yield();
            }
            exit_code => return exit_code,
        }
    }
}

#[inline]
pub fn semaphore_create(res_count: usize) -> isize {
    unsafe { syscall1(SyscallId::SEMAPHORE_CREATE, res_count) }
}

#[inline]
pub fn semaphore_up(sem_id: usize) -> isize {
    unsafe { syscall1(SyscallId::SEMAPHORE_UP, sem_id) }
}

#[inline]
pub fn semaphore_down(sem_id: usize) -> isize {
    unsafe { syscall1(SyscallId::SEMAPHORE_DOWN, sem_id) }
}

#[inline]
pub fn mutex_create(blocking: bool) -> isize {
    unsafe { syscall1(SyscallId::MUTEX_CREATE, blocking as _) }
}

#[inline]
pub fn mutex_lock(mutex_id: usize) -> isize {
    unsafe { syscall1(SyscallId::MUTEX_LOCK, mutex_id) }
}

#[inline]
pub fn mutex_unlock(mutex_id: usize) -> isize {
    unsafe { syscall1(SyscallId::MUTEX_UNLOCK, mutex_id) }
}

#[inline]
pub fn condvar_create() -> isize {
    unsafe { syscall1(SyscallId::CONDVAR_CREATE, 0) }
}

#[inline]
pub fn condvar_signal(condvar_id: usize) -> isize {
    unsafe { syscall1(SyscallId::CONDVAR_SIGNAL, condvar_id) }
}

#[inline]
pub fn condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    unsafe { syscall2(SyscallId::CONDVAR_WAIT, condvar_id, mutex_id) }
}

/// 这个模块包含调用系统调用的最小封装，用户可以直接使用这些函数调用自定义的系统调用。
pub mod native {
    use crate::SyscallId;
    use core::arch::asm;

    #[inline(always)]
    pub unsafe fn syscall0(id: SyscallId) -> isize {
        let ret: isize;
        asm!("ecall",
            in("a7") id.0,
            out("a0") ret,
        );
        ret
    }

    #[inline(always)]
    pub unsafe fn syscall1(id: SyscallId, a0: usize) -> isize {
        let ret: isize;
        asm!("ecall",
            inlateout("a0") a0 => ret,
            in("a7") id.0,
        );
        ret
    }

    #[inline(always)]
    pub unsafe fn syscall2(id: SyscallId, a0: usize, a1: usize) -> isize {
        let ret: isize;
        asm!("ecall",
            in("a7") id.0,
            inlateout("a0") a0 => ret,
            in("a1") a1,
        );
        ret
    }

    #[inline(always)]
    pub unsafe fn syscall3(id: SyscallId, a0: usize, a1: usize, a2: usize) -> isize {
        let ret: isize;
        asm!("ecall",
            in("a7") id.0,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
        );
        ret
    }

    #[inline(always)]
    pub unsafe fn syscall4(id: SyscallId, a0: usize, a1: usize, a2: usize, a3: usize) -> isize {
        let ret: isize;
        asm!("ecall",
            in("a7") id.0,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
        );
        ret
    }

    #[inline(always)]
    pub unsafe fn syscall5(
        id: SyscallId,
        a0: usize,
        a1: usize,
        a2: usize,
        a3: usize,
        a4: usize,
    ) -> isize {
        let ret: isize;
        asm!("ecall",
            in("a7") id.0,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
        );
        ret
    }

    #[inline(always)]
    pub unsafe fn syscall6(
        id: SyscallId,
        a0: usize,
        a1: usize,
        a2: usize,
        a3: usize,
        a4: usize,
        a5: usize,
    ) -> isize {
        let ret: isize;
        asm!("ecall",
            in("a7") id.0,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
            in("a5") a5,
        );
        ret
    }
}
