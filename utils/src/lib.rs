//! 一些零散的函数和定义。

#![no_std]
#![deny(warnings, missing_docs)]

/// 打印一些测试信息。
pub fn test_log() {
    use output::*;
    println!(
        r"
  ______        __                _         __
 /_  __/__  __ / /_ ____   _____ (_)____ _ / /
  / /  / / / // __// __ \ / ___// // __ `// /
 / /  / /_/ // /_ / /_/ // /   / // /_/ // /
/_/   \__,_/ \__/ \____//_/   /_/ \__,_//_/
==========================================="
    );
    log::trace!("LOG TEST >> Hello, world!");
    log::debug!("LOG TEST >> Hello, world!");
    log::info!("LOG TEST >> Hello, world!");
    log::warn!("LOG TEST >> Hello, world!");
    log::error!("LOG TEST >> Hello, world!");
    println!();
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
    /// 加载一个应用程序，并返回目标位置。
    ///
    /// 将应用程序可能用到的其他地址区域清零。
    pub unsafe fn load(&self, i: usize) -> usize {
        let slice = core::slice::from_raw_parts(
            &self.first as *const _ as *const usize,
            (self.count + 1) as _,
        );
        let pos = slice[i];
        let size = slice[i + 1] - pos;
        let base = self.base as usize + i * self.step as usize;
        core::ptr::copy_nonoverlapping::<u8>(pos as _, base as _, size);
        core::slice::from_raw_parts_mut(base as *mut u8, 0x20_0000)[size..].fill(0);
        base
    }

    /// 返回元数据描述的应用程序数量。
    #[inline]
    pub fn len(&self) -> usize {
        self.count as _
    }
}
