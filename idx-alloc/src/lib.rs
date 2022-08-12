//! 页帧分配器。

#![no_std]
#![deny(warnings, missing_docs)]

/// 序号分配器。
pub trait IdxAllocator {
    /// 在指定位置分配 `size`。
    fn allocate_fixed(&self, base: usize, size: usize) -> Result<(), AllocError>;

    /// 找到一个对齐到 `align` 的位置分配 `size` 并返回位置。
    fn allocate(&self, align: usize, size: usize) -> Result<usize, AllocError>;

    /// 回收一个对象。
    fn deallocate(&self, base: usize, size: usize);
}

/// 分配错误。
pub enum AllocError {
    /// 分配失败。找不到满足所有条件的空间。
    Failed,
    /// 不支持的分配要求。
    ///
    /// TODO 或许更好的方式是总结出一个特性空间，然后要求实现提供一个支持特性集常量，而不是返回不支持。
    Unsupported,
}
