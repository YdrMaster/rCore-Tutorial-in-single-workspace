//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

mod space;

pub extern crate page_table;
pub use space::AddressSpace;

use core::ptr::NonNull;
use page_table::{Pos, Pte, VmFlags, VmMeta, PPN};

/// 物理页管理。
pub trait PageManager<Meta: VmMeta> {
    /// 新建根页表页。
    fn new_root() -> Self;

    /// 获取根页表。
    fn root_ptr(&self) -> NonNull<Pte<Meta>>;

    /// 获取根页表的物理页号。
    #[inline]
    fn root_ppn(&self) -> PPN<Meta> {
        self.v_to_p(self.root_ptr())
    }

    /// 计算当前地址空间上指向物理页的指针。
    fn p_to_v<T>(&self, ppn: PPN<Meta>) -> NonNull<T>;

    /// 计算当前地址空间上的指针指向的物理页。
    fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Meta>;

    /// 检查是否拥有一个页的所有权。
    fn check_ownership(&self, pos: Pos<Meta>, pte: Pte<Meta>) -> Ownership;

    /// 为地址空间分配 `len` 个物理页。
    fn allocate(&mut self, len: usize, flags: &mut VmFlags<Meta>) -> NonNull<u8>;

    /// 从地址空间释放 `pte` 指示的 `len` 个物理页。
    fn deallocate(&mut self, pte: Pte<Meta>, len: usize) -> usize;

    /// 释放根页表。
    fn drop_root(&mut self);
}

/// 页所有权。
///
/// RISC-V 可以利用页表项里的 2 个预留位保存所有权特性。
/// 否则需要在段管理中保存。
pub enum Ownership {
    /// 独占。
    ///
    /// 独占的页取消映射时会被回收。
    Owned,
    /// 静态引用。
    ///
    /// 静态引用的页取消映射时不会回收。如果是页表页，不可修改。
    ///
    /// 用于已知拥有页所有权的对象生命周期必然比当前地址空间长的情况，如内核向进程共享。
    Ref,
    /// 引用计数。
    ///
    /// 引用计数的页映射时计数 +1，取消映射时计数 -1。
    /// 如果取消映射时发现是最后一个引用，页被回收。
    ///
    /// 用于不确定生命周期的情况，如几个进程之间共享。
    Rc,
    /// 写时复制引用。
    ///
    /// 写时复制引用映射时计数 +1，且一定不可写。
    /// 取消映射时计数 -1，如果发现是最后一个引用，页被回收。
    /// 写入时计数 -1，如果发现是最后一个引用，转为独占；否则复制一个独占的。
    ///
    /// 用于不确定生命周期的可写页共享。
    Cow,
}
