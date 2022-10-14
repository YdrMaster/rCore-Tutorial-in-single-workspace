//! 内存分配。

#![no_std]
#![deny(warnings, missing_docs)]

extern crate alloc;

use alloc::alloc::handle_alloc_error;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};

/// 初始化内存分配。
pub unsafe fn init(region: &'static mut [u8]) {
    let range = region.as_mut_ptr_range();
    log::info!("MEMORY = {range:?}");
    let ptr = NonNull::new(range.start).unwrap();
    HEAP.init(core::mem::size_of::<usize>().trailing_zeros() as _, ptr);
    HEAP.transfer(ptr, region.len());
}

/// 堆分配器。
///
/// 6 + 21 + 3 = 30 -> 1 GiB
static mut HEAP: BuddyAllocator<21, UsizeBuddy, LinkedListBuddy> = BuddyAllocator::new();

struct Global;

#[global_allocator]
static GLOBAL: Global = Global;

unsafe impl GlobalAlloc for Global {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Ok((ptr, _)) = HEAP.allocate_layout::<u8>(layout) {
            ptr.as_ptr()
        } else {
            handle_alloc_error(layout)
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HEAP.deallocate_layout(NonNull::new(ptr).unwrap(), layout)
    }
}
