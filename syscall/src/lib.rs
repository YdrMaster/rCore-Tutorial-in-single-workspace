#![no_std]
#![deny(warnings)]

#[cfg(all(feature = "kernel", feature = "user"))]
compile_error!("You can only use one of `supervisor` or `user` features at a time");

mod io;
mod syscalls;
mod time;

pub use io::*;
pub use signal_defs::{SignalAction, SignalNo, MAX_SIG};
pub use time::*;

#[cfg(feature = "user")]
mod user;

#[cfg(feature = "user")]
pub use user::*;

#[cfg(feature = "kernel")]
mod kernel;

#[cfg(feature = "kernel")]
pub use kernel::*;

/// 系统调用号。
///
/// 实现为包装类型，在不损失扩展性的情况下实现类型安全性。
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[repr(transparent)]
pub struct SyscallId(pub usize);

impl From<usize> for SyscallId {
    #[inline]
    fn from(val: usize) -> Self {
        Self(val)
    }
}
