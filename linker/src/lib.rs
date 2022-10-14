//! 这个板块为内核提供链接脚本的文本，以及依赖于定制链接脚本的功能。
//!
//! build.rs 文件可依赖此板块，并将 [`SCRIPT`] 文本常量写入链接脚本文件：
//!
//! ```rust
//! use std::{env, fs, path::PathBuf};
//!
//! let ld = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
//! fs::write(ld, linker::SCRIPT).unwrap();
//!
//! println!("cargo:rerun-if-changed=build.rs");
//! println!("cargo:rustc-link-arg=-T{}", ld.display());
//! ```
//!
//! 内核使用 [`boot0`] 宏定义内核启动栈和高级语言入口：
//!
//! ```rust
//! linker::boot0!(rust_main; stack = 4 * 4096);
//! ```
//!
//! 内核所在内核区域定义成 4 个部分（[`KernelRegionTitle`]）:
//!
//! 1. 代码段
//! 2. 只读数据段
//! 3. 数据段
//! 4. 启动数据段
//!
//! 启动数据段放在最后，以便启动完成后换栈。届时可放弃启动数据段，将其加入动态内存区。
//!
//! 用 [`KernelLayout`] 结构体定位、保存和访问内核内存布局。

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
    .bss : ALIGN(8) {
        __sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        __ebss = .;
    }
    .boot : ALIGN(4K) {
        __boot = .;
        KEEP(*(.boot.stack))
    }
    __end = .;
}";

/// 定义内核入口。
///
/// 将设置一个启动栈，并在启动栈上调用高级语言入口。
#[macro_export]
macro_rules! boot0 {
    ($entry:ident; stack = $stack:expr) => {
        #[naked]
        #[no_mangle]
        #[link_section = ".text.entry"]
        unsafe extern "C" fn _start() -> ! {
            #[link_section = ".boot.stack"]
            static mut STACK: [u8; $stack] = [0u8; $stack];

            core::arch::asm!(
                "la sp, __end",
                "j  {main}",
                main = sym rust_main,
                options(noreturn),
            )
        }
    };
}

/// 内核地址信息。
#[derive(Debug)]
pub struct KernelLayout {
    text: usize,
    rodata: usize,
    data: usize,
    sbss: usize,
    ebss: usize,
    boot: usize,
    end: usize,
}

impl KernelLayout {
    /// 非零初始化，避免 bss。
    pub const INIT: Self = Self {
        text: usize::MAX,
        rodata: usize::MAX,
        data: usize::MAX,
        sbss: usize::MAX,
        ebss: usize::MAX,
        boot: usize::MAX,
        end: usize::MAX,
    };

    /// 定位内核布局。
    #[inline]
    pub fn locate() -> Self {
        extern "C" {
            fn _start();
            fn __rodata();
            fn __data();
            fn __sbss();
            fn __ebss();
            fn __boot();
            fn __end();
        }

        Self {
            text: _start as _,
            rodata: __rodata as _,
            data: __data as _,
            sbss: __sbss as _,
            ebss: __ebss as _,
            boot: __boot as _,
            end: __end as _,
        }
    }

    /// 内核起始地址。
    #[inline]
    pub const fn start(&self) -> usize {
        self.text
    }

    /// 内核结尾地址。
    #[inline]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// 内核静态二进制长度。
    #[inline]
    pub const fn len(&self) -> usize {
        self.end - self.text
    }

    /// 清零 .bss 段。
    #[inline]
    pub unsafe fn zero_bss(&self) {
        r0::zero_bss::<u64>(self.sbss as _, self.ebss as _);
    }

    /// 内核区段迭代器。
    #[inline]
    pub fn iter(&self) -> KernelRegionIterator {
        KernelRegionIterator {
            layout: self,
            next: Some(KernelRegionTitle::Text),
        }
    }
}

use core::{fmt, ops::Range};

/// 内核内存分区迭代器。
pub struct KernelRegionIterator<'a> {
    layout: &'a KernelLayout,
    next: Option<KernelRegionTitle>,
}

/// 内核内存分区名称。
#[derive(Clone, Copy)]
pub enum KernelRegionTitle {
    /// 代码段。
    Text,
    /// 只读数据段。
    Rodata,
    /// 数据段。
    Data,
    /// 启动数据段。
    Boot,
}

/// 内核内存分区。
pub struct KernelRegion {
    /// 分区名称。
    pub title: KernelRegionTitle,
    /// 分区地址范围。
    pub range: Range<usize>,
}

impl fmt::Display for KernelRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.title {
            KernelRegionTitle::Text => write!(f, ".text ----> ")?,
            KernelRegionTitle::Rodata => write!(f, ".rodata --> ")?,
            KernelRegionTitle::Data => write!(f, ".data ----> ")?,
            KernelRegionTitle::Boot => write!(f, ".boot ----> ")?,
        }
        write!(f, "{:#10x}..{:#10x}", self.range.start, self.range.end)
    }
}

impl Iterator for KernelRegionIterator<'_> {
    type Item = KernelRegion;

    fn next(&mut self) -> Option<Self::Item> {
        use KernelRegionTitle::*;
        match self.next? {
            Text => {
                self.next = Some(Rodata);
                Some(KernelRegion {
                    title: Text,
                    range: self.layout.text..self.layout.rodata,
                })
            }
            Rodata => {
                self.next = Some(Data);
                Some(KernelRegion {
                    title: Rodata,
                    range: self.layout.rodata..self.layout.data,
                })
            }
            Data => {
                self.next = Some(Boot);
                Some(KernelRegion {
                    title: Data,
                    range: self.layout.data..self.layout.ebss,
                })
            }
            Boot => {
                self.next = None;
                Some(KernelRegion {
                    title: Boot,
                    range: self.layout.boot..self.layout.end,
                })
            }
        }
    }
}
