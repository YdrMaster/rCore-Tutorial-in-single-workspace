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

/// 初始化 `n` 个页的托管区，这些页将放置在 bss 段上。
#[macro_export]
macro_rules! init {
    (pages = $n:expr) => {
        $crate::init!(pages = $n; 4096)
    };

    (pages = $n:expr; $size:expr) => {{
        #[repr(C, align($size))]
        struct Page([u8; $size]);

        impl Page {
            const ZERO: Self = Self([0u8; $size]);
        }

        static mut SPACE: [Page; $n] = [Page::ZERO; $n];
        unsafe { $crate::_init(&mut SPACE) };
    }};
}

/// 初始化全局分配器和内核堆分配器。
#[doc(hidden)]
pub unsafe fn _init<T>(region: &'static mut [T]) {
    let range = region.as_mut_ptr_range();
    log::info!("MEMORY = {range:?}");
    let ptr = NonNull::new(range.start).unwrap();
    HEAP.init(PTR_BITS, ptr);
    HEAP.transfer(ptr, region.len() * core::mem::size_of::<T>());
}

const PTR_BITS: usize = core::mem::size_of::<usize>().trailing_zeros() as _;

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
