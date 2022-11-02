//! 任务管理 lib

#![no_std]
#![deny(warnings, missing_docs)]

extern crate alloc;

#[cfg(all(feature = "proc", feature = "thread"))]
compile_error!("You can only use one of `supervisor` or `user` features at a time");

mod id;
mod manager;
#[cfg(feature = "proc")]
mod proc_manage;
mod scheduler;
#[cfg(all(feature = "proc", feature = "thread"))]
compile_error!("You can only use one of `proc` or `thread` features at a time");
mod relation;
#[cfg(feature = "thread")]
mod thread_manager;

pub use manager::Manage;
pub use scheduler::Schedule;
pub use id::*;
#[cfg(feature = "proc")]
use relation::ProcRelation; 
#[cfg(feature = "thread")]
use relation::ProcThreadRel;

#[cfg(feature = "proc")]
pub use proc_manage::PManager;

#[cfg(feature = "thread")]
pub use thread_manager::PThreadManager;
