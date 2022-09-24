//! 任务管理 lib

#![no_std]
#![deny(warnings, missing_docs)]

mod manager;
mod processor;

pub use manager::Manage;
pub use processor::Processor;
