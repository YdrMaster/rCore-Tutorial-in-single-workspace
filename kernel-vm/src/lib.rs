//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

mod counter;

pub use counter::PageCounter;

use core::alloc::Layout;

/// 4 KiB 页类型。
#[repr(C, align(4096))]
pub struct Page4K([u8; 4096]);

impl Page4K {
    /// 空白页。
    pub const ZERO: Self = Self([0; 4096]);
    /// 页布局。
    pub const LAYOUT: Layout = Layout::new::<Self>();
    /// 页虚存地址。
    #[inline]
    pub fn addr(&self) -> usize {
        self as *const _ as _
    }
}
