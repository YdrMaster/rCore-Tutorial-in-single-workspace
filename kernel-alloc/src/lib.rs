//! 内存分配。

#![no_std]
#![deny(warnings, missing_docs)]

#[macro_use]
extern crate alloc;

use alloc::alloc::handle_alloc_error;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};

/// 4 KiB 内存页类型。
#[repr(C, align(4096))]
pub struct Page4K([u8; 4096]);

impl Page4K {
    /// 空白页。
    pub const ZERO: Self = Self([0u8; 4096]);
}

const BITS_PAGE: usize = core::mem::size_of::<Page4K>().trailing_zeros() as _;
const BITS_PTR: usize = core::mem::size_of::<usize>().trailing_zeros() as _;

/// 初始化 `n` 个页的托管区，这些页将放置在 bss 段上。
#[macro_export]
macro_rules! init {
    (pages = $n:expr) => {{
        static mut SPACE: [$crate::Page4K; $n] = [$crate::Page4K::ZERO; $n];
        unsafe { $crate::_init(&mut SPACE, true) };
    }};
}

/// 初始化全局分配器和内核堆分配器。
#[doc(hidden)]
pub unsafe fn _init(region: &'static mut [Page4K], test: bool) {
    let range = region.as_mut_ptr_range();
    log::info!("MEMORY = {range:?}");
    let ptr = NonNull::new(range.start).unwrap();
    PAGE.init(BITS_PAGE, ptr);
    HEAP.init(BITS_PTR, ptr);
    PAGE.transfer(ptr, region.len() << BITS_PAGE);

    // 测试堆分配回收
    if test {
        let mut vec = vec![0; 1234];
        for (i, val) in vec.iter_mut().enumerate() {
            *val = i;
        }
        for (i, val) in vec.into_iter().enumerate() {
            assert_eq!(i, val);
        }
        log::debug!("memory management test pass");
    }
}

type MutAllocator<const N: usize> = BuddyAllocator<N, UsizeBuddy, LinkedListBuddy>;

/// 页分配器。
pub static mut PAGE: MutAllocator<12> = MutAllocator::new();

/// 堆分配器。
static mut HEAP: MutAllocator<21> = MutAllocator::new();

struct Global;

#[global_allocator]
static GLOBAL: Global = Global;

unsafe impl GlobalAlloc for Global {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Ok((ptr, _)) = HEAP.allocate_layout::<u8>(layout) {
            ptr.as_ptr()
        } else if let Ok((ptr, size)) = PAGE.allocate_layout::<u8>(
            Layout::from_size_align_unchecked(layout.size().next_power_of_two(), layout.align()),
        ) {
            log::trace!("global transfers {} pages to heap", size >> BITS_PAGE);
            HEAP.transfer(ptr, size);
            HEAP.allocate_layout::<u8>(layout).unwrap().0.as_ptr()
        } else {
            handle_alloc_error(layout)
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HEAP.deallocate_layout(NonNull::new(ptr).unwrap(), layout)
    }
}
