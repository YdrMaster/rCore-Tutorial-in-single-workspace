//! 一些零散的函数和定义。

#![no_std]
#![deny(warnings, missing_docs)]

/// bss 段清零。
///
/// 需要定义 sbss 和 ebss 全局符号才能定位 bss。
#[inline]
pub fn zero_bss() {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
}

/// 应用程序元数据
#[repr(C)]
pub struct AppMeta {
    base: u64,
    step: u64,
    count: u64,
    first: u64,
}

impl AppMeta {
    /// 以静态链接模式遍历应用程序。
    #[inline]
    pub fn iter_static(&'static self) -> StaticAppIterator {
        StaticAppIterator { meta: self, i: 0 }
    }

    /// 以静态链接模式遍历应用程序。
    #[inline]
    pub fn iter_elf(&'static self) -> ElfIterator {
        ElfIterator { meta: self, i: 0 }
    }

    /// 获取应用程序数量
    #[inline]
    pub fn get_app_num(&'static self) -> u64 {
        self.count
    }
}

/// 静态链接程序迭代器。
pub struct StaticAppIterator {
    meta: &'static AppMeta,
    i: u64,
}

impl Iterator for StaticAppIterator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.meta.count {
            None
        } else {
            let i = self.i as usize;
            self.i += 1;
            unsafe {
                let slice = core::slice::from_raw_parts(
                    &self.meta.first as *const _ as *const usize,
                    (self.meta.count + 1) as _,
                );
                let pos = slice[i];
                let size = slice[i + 1] - pos;
                let base = self.meta.base as usize + i * self.meta.step as usize;
                core::ptr::copy_nonoverlapping::<u8>(pos as _, base as _, size);
                core::slice::from_raw_parts_mut(base as *mut u8, 0x20_0000)[size..].fill(0);
                Some(base)
            }
        }
    }
}

/// Elf 程序迭代器。
pub struct ElfIterator {
    meta: &'static AppMeta,
    i: u64,
}

impl Iterator for ElfIterator {
    type Item = &'static [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.meta.count {
            None
        } else {
            let i = self.i as usize;
            self.i += 1;
            unsafe {
                let slice = core::slice::from_raw_parts(
                    &self.meta.first as *const _ as *const usize,
                    (self.meta.count + 1) as _,
                );
                let pos = slice[i];
                let size = slice[i + 1] - pos;
                Some(core::slice::from_raw_parts(pos as _, size))
            }
        }
    }
}
