//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

mod frame_queue;

use core::ops::Range;
use frame_queue::FrameQueue;
use page_table::{VmMeta, PPN, VPN};

pub extern crate page_table;

/// 页帧分配器。
pub trait FrameAllocator<Meta: VmMeta>: Sized {
    /// 分配一个 4 KiB 小页。
    ///
    /// 这个功能比较常用，独立出来可能会简化使用。
    fn allocate_one(&self) -> Result<PPN<Meta>, AllocError>;

    /// 根据 `vpn_range` 分配一组适宜映射的物理页帧。
    ///
    /// `vpn_range` 指示的虚存范围可能不在当前虚存空间。
    ///
    /// `p_to_v` 是物理页映射到当前虚地址空间的方式，因为实现会将页帧链表节点写在这些物理页帧的开头。
    /// 可以在调用时分配一个临时的虚页。
    ///
    /// 这个方法需要写入其分配内存，即**侵入式**的分配。
    fn allocate(
        &self,
        vpn_range: Range<VPN<Meta>>,
        p_to_v: impl Fn(PPN<Meta>) -> VPN<Meta>,
    ) -> Result<FrameQueue<'_, Meta, Self>, AllocError>;

    /// 回收物理页。
    ///
    /// # Notice
    ///
    /// 为了简化设计，大页需要分解成小页号范围回收。
    ///
    /// # Safety
    ///
    /// 实现不要抛出异常，即使收到一个不合理的范围，例如重复回收或试图回收不在管辖范围的页号。
    unsafe fn deallocate(&self, ppn_range: Range<PPN<Meta>>);
}

/// 分配失败。
///
/// 通常是因为物理页帧不足，也可能是不支持这样的分配要求，由实现决定。
#[derive(Debug)]
pub struct AllocError;
