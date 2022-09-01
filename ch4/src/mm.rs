﻿use alloc::alloc::handle_alloc_error;
use buddy_allocator::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use kernel_vm::{init_allocator, PageAllocator};
use output::log;

/// 初始化全局分配器和内核堆分配器。
pub fn init() {
    /// 4 KiB 页类型。
    #[repr(C, align(4096))]
    pub struct Memory<const N: usize>([u8; N]);

    const MEMORY_SIZE: usize = 256 << 12;

    /// 托管空间 1 MiB
    static mut MEMORY: Memory<MEMORY_SIZE> = Memory([0u8; MEMORY_SIZE]);
    unsafe {
        let ptr = NonNull::new(MEMORY.0.as_mut_ptr()).unwrap();
        log::info!(
            "MEMORY = {:#x}..{:#x}",
            ptr.as_ptr() as usize,
            ptr.as_ptr() as usize + MEMORY_SIZE
        );
        PAGE.init(12, ptr);
        HEAP.init(3, ptr);
        PAGE.transfer(ptr, MEMORY_SIZE);
        init_allocator(&Pages);
    }
}

/// 测试内核堆分配。
pub fn test() {
    let mut vec = vec![0; 1234];
    for (i, val) in vec.iter_mut().enumerate() {
        *val = i;
    }
    for (i, val) in vec.into_iter().enumerate() {
        assert_eq!(i, val);
    }
    log::debug!("memory management test pass");
    println!();
}

type MutAllocator<const N: usize> = BuddyAllocator<N, UsizeBuddy, LinkedListBuddy>;
pub static mut PAGE: MutAllocator<5> = MutAllocator::new();
static mut HEAP: MutAllocator<32> = MutAllocator::new();

struct Global;
struct Pages;

impl PageAllocator for Pages {
    #[inline]
    fn allocate(&self, bits: usize) -> NonNull<u8> {
        let size = 1 << bits;
        unsafe { PAGE.allocate_layout(Layout::from_size_align_unchecked(size, size)) }
            .unwrap()
            .0
    }

    #[inline]
    fn deallocate(&self, ptr: NonNull<u8>, bits: usize) {
        log::warn!("deallocate {ptr:#x?}");
        unsafe { PAGE.deallocate(ptr, 1 << bits) };
    }
}

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
            log::trace!("global transfers {} pages to heap", size >> 12);
            HEAP.transfer(ptr, size);
            HEAP.allocate_layout::<u8>(layout).unwrap().0.as_ptr()
        } else {
            handle_alloc_error(layout)
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HEAP.deallocate(NonNull::new(ptr).unwrap(), layout.size())
    }
}
