//! 页帧分配器。

#![no_std]
#![deny(warnings, missing_docs)]

/// 序号分配器。
pub trait IdxAllocator {
    /// 在指定位置分配 `size`。
    fn allocate_fixed(&self, base: usize, size: usize) -> Result<(), AllocError>;

    /// 分配一个对象，它对齐到它的 `size`。
    #[inline]
    fn allocate_sigle(&self, size: usize) -> Result<usize, AllocError> {
        self.allocate(size, size)
    }

    /// 通常的分配，从一个对齐到 `align` 的位置分配 `size` 并返回位置。
    fn allocate(&self, align: usize, size: usize) -> Result<usize, AllocError>;

    /// 回收一个对象。
    fn deallocate(&self, base: usize, size: usize);
}

/// 分配错误
pub struct AllocError;
