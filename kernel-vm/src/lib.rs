//! 内核虚存管理。

#![no_std]
#![deny(warnings)] //, missing_docs)]

mod space;

pub use space::{AddressSpace, Segment};

extern crate alloc;

use core::ptr::NonNull;

pub trait PageAllocator: Sync {
    fn allocate(&self, bits: usize) -> NonNull<u8>;

    fn deallocate(&self, ptr: NonNull<u8>, bits: usize);
}

static ALLOC: spin::Once<&'static dyn PageAllocator> = spin::Once::new();

pub fn init_allocator(a: &'static dyn PageAllocator) {
    ALLOC.call_once(|| a);
}
