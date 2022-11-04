//! 任务管理 lib

#![no_std]
#![feature(doc_cfg)]
#![deny(warnings, missing_docs)]

extern crate alloc;

mod id;
mod manager;
mod scheduler;

pub use id::*;
pub use manager::Manage;
pub use scheduler::Schedule;

#[cfg(feature = "proc")]
mod proc_manage;
#[cfg(feature = "proc")]
mod proc_rel;
#[cfg(feature = "proc")]
pub use proc_manage::PManager;
#[cfg(feature = "proc")]
pub use proc_rel::ProcRel;

#[cfg(feature = "thread")]
mod proc_thread_rel;
#[cfg(feature = "thread")]
mod thread_manager;
#[cfg(feature = "thread")]
pub use proc_thread_rel::ProcThreadRel;
#[cfg(feature = "thread")]
pub use thread_manager::PThreadManager;
