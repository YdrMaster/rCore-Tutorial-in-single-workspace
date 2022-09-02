/// 物理内存管理
use alloc::alloc::handle_alloc_error;
use buddy_allocator::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
    ptr::NonNull,
};

/// 初始化全局分配器和内核堆分配器。
pub fn init() {
    unsafe {
        let ptr = NonNull::new(MEMORY.as_mut_ptr()).unwrap();
        let len = core::mem::size_of_val(&MEMORY);
        println!(
            "MEMORY = {:#x}..{:#x}",
            ptr.as_ptr() as usize,
            ptr.as_ptr() as usize + len
        );
        GLOBAL.init(12, ptr);
        GLOBAL.transfer(ptr, len);
        ALLOC.0.borrow_mut().init(3, ptr);
    }
}

/// 获取全局分配器。
#[inline]
pub unsafe fn global() -> &'static mut MutAllocator<5> {
    &mut GLOBAL
}

#[repr(C, align(4096))]
pub struct Page([u8; 4096]);

impl Page {
    pub const ZERO: Self = Self([0; 4096]);
    pub const LAYOUT: Layout = Layout::new::<Self>();

    #[inline]
    pub fn addr(&self) -> usize {
        self as *const _ as _
    }
}

/// 托管空间 4 MiB
static mut MEMORY: [Page; 1024] = [Page::ZERO; 1024];
static mut GLOBAL: MutAllocator<5> = MutAllocator::<5>::new();
#[global_allocator]
static ALLOC: SharedAllocator<22> = SharedAllocator(RefCell::new(MutAllocator::new()));

pub type MutAllocator<const N: usize> = BuddyAllocator<N, UsizeBuddy, LinkedListBuddy>;

struct SharedAllocator<const N: usize>(RefCell<MutAllocator<N>>);
unsafe impl<const N: usize> Sync for SharedAllocator<N> {}
unsafe impl<const N: usize> GlobalAlloc for SharedAllocator<N> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.0.borrow_mut();
        loop {
            if let Ok((ptr, _)) = inner.allocate::<u8>(layout) {
                return ptr.as_ptr();
            } else if let Ok((ptr, size)) = GLOBAL.allocate::<u8>(layout) {
                inner.transfer(ptr, size);
            } else {
                handle_alloc_error(layout)
            }
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .borrow_mut()
            .deallocate(NonNull::new(ptr).unwrap(), layout.size())
    }
}
