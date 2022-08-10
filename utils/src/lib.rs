#![no_std]
#![deny(warnings)]

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

/// 将一个应用程序加载到目标位置。
#[inline]
pub fn load_app(range: core::ops::Range<usize>, base: usize) {
    unsafe { core::ptr::copy_nonoverlapping::<u8>(range.start as _, base as _, range.len()) };
}

/// 解析一个数字。
#[inline]
pub fn parse_num(str: impl AsRef<str>) -> usize {
    let str = str.as_ref();
    if let Some(num) = str.strip_prefix("0x") {
        usize::from_str_radix(num, 16).unwrap()
    } else {
        usize::from_str_radix(str, 10).unwrap()
    }
}
