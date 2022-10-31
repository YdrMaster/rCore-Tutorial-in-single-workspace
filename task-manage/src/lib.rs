//! 任务管理 lib

#![no_std]
#![deny(warnings, missing_docs)]

extern crate alloc;

mod id;
mod manager;
mod processor;
mod scheduler;
mod relation;

pub use manager::Manage;
pub use scheduler::Schedule;
pub use id::*;
use relation::ProcRelation;
#[cfg(feature = "proc")]
pub use processor::Processor;
