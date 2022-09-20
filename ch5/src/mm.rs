use alloc::alloc::handle_alloc_error;
use console::log;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use customizable_buddy::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use kernel_vm::page_table::{MmuMeta, Sv39};

/// 初始化全局分配器和内核堆分配器。
pub fn init() {
    /// 4 KiB 页类型。
    #[repr(C, align(4096))]
    pub struct Memory<const N: usize>([u8; N]);

    const MEMORY_SIZE: usize = 512 << Sv39::PAGE_BITS;

    /// 托管空间 1 MiB
    static mut MEMORY: Memory<MEMORY_SIZE> = Memory([0u8; MEMORY_SIZE]);
    unsafe {
        let ptr = NonNull::new(MEMORY.0.as_mut_ptr()).unwrap();
        log::info!(
            "MEMORY = {:#x}..{:#x}",
            ptr.as_ptr() as usize,
            ptr.as_ptr() as usize + MEMORY_SIZE
        );
        PAGE.init(Sv39::PAGE_BITS, ptr);
        HEAP.init(core::mem::size_of::<usize>().trailing_zeros() as _, ptr);
        PAGE.transfer(ptr, MEMORY_SIZE);
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
            log::trace!("global transfers {} pages to heap", size >> Sv39::PAGE_BITS);
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
