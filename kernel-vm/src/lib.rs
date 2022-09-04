//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

mod space;

pub extern crate page_table;
pub use space::AddressSpace;

use core::ptr::NonNull;
use page_table::{PageTable, Pte, VmFlags, VmMeta, PPN};

/// 物理页管理。
pub trait PageManager<Meta: VmMeta> {
    /// 新建根页表页。
    fn new_root() -> Self;

    /// 获取根页表。
    fn root(&self) -> PageTable<Meta> {
        unsafe { PageTable::from_root(self.p_to_v(self.root_ppn())) }
    }

    /// 获取根页表的物理页号。
    fn root_ppn(&self) -> PPN<Meta>;

    /// 计算当前地址空间上指向物理页的指针。
    fn p_to_v<T>(&self, ppn: PPN<Meta>) -> NonNull<T>;

    /// 计算当前地址空间上的指针指向的物理页。
    fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Meta>;

    /// 为地址空间分配 `len` 个物理页。
    fn allocate(&mut self, len: usize, flags: &mut VmFlags<Meta>) -> NonNull<u8>;

    /// 从地址空间释放 `pte` 指示的 `len` 个物理页。
    fn deallocate(&mut self, pte: Pte<Meta>, len: usize) -> usize;

    /// 释放根页表。
    fn drop_root(&mut self);
}
