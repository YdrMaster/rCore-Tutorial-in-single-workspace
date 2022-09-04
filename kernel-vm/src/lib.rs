//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

mod space;

pub extern crate page_table;
pub use space::AddressSpace;

use core::ptr::NonNull;

/// 分区的页分配器。
///
/// 每个地址空间会可以使用一个分区的分配器以减少分配中的竞争。
pub trait ScopedAllocator: Sync {
    /// 创建一个页号位数为 `bits` 的新的地址空间页分配器分区。
    unsafe fn create(&self, bits: usize) -> NonNull<u8>;

    /// 销毁地址空间页分配器分区。
    unsafe fn destory(&self, root: NonNull<u8>);

    /// 向指定分区分配 `len` 个页。
    unsafe fn allocate(&self, root: NonNull<u8>, len: usize) -> NonNull<u8>;

    /// 从指定分区回收 `len` 个页。
    unsafe fn deallocate(&self, root: NonNull<u8>, ptr: NonNull<u8>, len: usize);
}

static ALLOC: spin::Once<&'static dyn ScopedAllocator> = spin::Once::new();

/// 初始化分区页分配器。
pub fn init_allocator(a: &'static dyn ScopedAllocator) {
    ALLOC.call_once(|| a);
}
