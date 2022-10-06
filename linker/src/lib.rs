//! 在 kernel 的 build.rs 和 src 之间共享常量和类型定义。

#![no_std]
#![deny(warnings, missing_docs)]

mod app;

pub use app::{AppIterator, AppMeta};

/// 链接脚本。
pub const SCRIPT: &[u8] = b"\
OUTPUT_ARCH(riscv)
ENTRY(_start)
SECTIONS {
    . = 0x80200000;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : ALIGN(4K) {
        __rodata = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : ALIGN(4K) {
        __data = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        *(.bss.uninit)
        . = ALIGN(8);
        __bss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
    __end = .;
}";

/// 内核地址信息。
#[derive(Debug)]
pub struct KernelLayout {
    /// 代码段开头。
    pub text: usize,
    /// 只读数据段开头。
    pub rodata: usize,
    /// 数据段开头。
    pub data: usize,
    /// .bss 段开头。
    bss: usize,
    /// 内核结束位置。
    pub end: usize,
}

impl KernelLayout {
    /// 非零初始化，避免 bss。
    pub const INIT: Self = Self {
        text: usize::MAX,
        rodata: usize::MAX,
        data: usize::MAX,
        bss: usize::MAX,
        end: usize::MAX,
    };

    /// 定位内核布局。
    #[inline]
    pub fn locate() -> Self {
        extern "C" {
            fn _start();
            fn __rodata();
            fn __data();
            fn __bss();
            fn __end();
        }

        Self {
            text: _start as _,
            rodata: __rodata as _,
            data: __data as _,
            bss: __bss as _,
            end: __end as _,
        }
    }

    /// 清零 .bss 段。
    #[inline]
    pub unsafe fn zero_bss(&self) {
        r0::zero_bss::<u64>(self.bss as _, self.end as _);
    }
}
