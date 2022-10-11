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
        unsafe { $crate::_init(&mut SPACE, true) };
    }};
}

/// 初始化全局分配器和内核堆分配器。
#[doc(hidden)]
pub unsafe fn _init<T>(region: &'static mut [T], test: bool) {
    PAGE_BITS = core::mem::size_of::<T>().trailing_zeros() as usize;

    let range = region.as_mut_ptr_range();
    log::info!("MEMORY = {range:?}");
    let ptr = NonNull::new(range.start).unwrap();
    PAGE.init(PAGE_BITS, ptr);
    HEAP.init(PTR_BITS, ptr);
    PAGE.transfer(ptr, region.len() << PAGE_BITS);

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

const PTR_BITS: usize = core::mem::size_of::<usize>().trailing_zeros() as _;

/// 页地址位数。
static mut PAGE_BITS: usize = 0;

/// 页分配器。
static mut PAGE: MutAllocator<12> = MutAllocator::new();

/// 堆分配器。
static mut HEAP: MutAllocator<21> = MutAllocator::new();

/// 整页分配。
pub fn alloc_pages(count: usize) -> &'static mut [usize] {
    unsafe {
        let layout = Layout::from_size_align_unchecked(count << PAGE_BITS, 1 << PAGE_BITS);
        match PAGE.allocate_layout(layout) {
            Ok((ptr, _)) => {
                core::slice::from_raw_parts_mut(ptr.as_ptr(), count << (PAGE_BITS - PTR_BITS))
            }
            Err(_) => handle_alloc_error(layout),
        }
    }
}

/// 整页回收。
pub fn dealloc_pages<T>(ptr: NonNull<T>, count: usize) {
    unsafe { PAGE.deallocate(ptr, count << PAGE_BITS) }
}

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
            log::trace!("global transfers {size} bytes to heap");
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
