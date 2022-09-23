//! 任务管理 lib

#![no_std]
#![deny(warnings, missing_docs)]
#![feature(const_btree_new, const_mut_refs)]

mod manager;
mod processor;
mod scheduler;
mod task;

pub use manager::TaskManager;
pub use processor::Processor;
pub use task::Execute;

extern crate alloc;
