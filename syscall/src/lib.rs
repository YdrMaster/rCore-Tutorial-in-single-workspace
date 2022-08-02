#![no_std]

#[cfg(all(feature = "supervisor", feature = "user"))]
compile_error!("You can only use one of `supervisor` or `user` features at a time");

#[cfg(feature = "user")]
mod user;

#[cfg(feature = "user")]
pub use user::*;

const ID_WRITE: usize = 64;
const ID_EXIT: usize = 93;
