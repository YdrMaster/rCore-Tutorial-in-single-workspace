//! 内核虚存管理。

#![no_std]
#![deny(warnings)] //, missing_docs)]

mod space;

pub extern crate page_table;
pub use space::AddressSpace;

use core::ptr::NonNull;
use page_table::{VAddr, VmFlags, VmMeta};

pub trait PageAllocator: Sync {
    fn allocate(&self, bits: usize) -> NonNull<u8>;

    fn deallocate(&self, ptr: NonNull<u8>, bits: usize);
}

static ALLOC: spin::Once<&'static dyn PageAllocator> = spin::Once::new();

pub fn init_allocator(a: &'static dyn PageAllocator) {
    ALLOC.call_once(|| a);
}

pub struct ForeignPtr<Meta: VmMeta> {
    pub raw: VAddr<Meta>,
    pub flags: VmFlags<Meta>,
}
