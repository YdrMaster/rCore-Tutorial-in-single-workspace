#![no_std]
#![deny(warnings)]

use core::fmt::{Arguments, Write};
use spin::Once;

/// 向用户提供 `log`。
pub extern crate log;

/// 这个接口定义了向控制台“输出”这件事。
pub trait Console: Sync {
    /// 向控制台放置一个字符。
    fn put_char(&self, c: u8);

    /// 向控制台放置一个字符串。
    ///
    /// 如果使用了锁，覆盖这个实现以免反复获取和释放锁。
    #[inline]
    fn put_str(&self, s: &str) {
        for c in s.bytes() {
            self.put_char(c);
        }
    }
}

/// 库找到输出的方法：保存一个对象引用，这是一种单例。
static CONSOLE: Once<&'static dyn Console> = Once::new();

/// 用户调用这个函数设置输出的方法。
pub fn init_console(console: &'static dyn Console) {
    CONSOLE.call_once(|| console);
    log::set_logger(&Logger).unwrap();
}

/// 给宏用的，用户不会直接调它。
#[doc(hidden)]
#[inline]
pub fn _print(args: Arguments) {
    Logger.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(core::format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {{
        $crate::_print(core::format_args!($($arg)*));
        $crate::println!();
    }}
}

/// 这个 Unit struct 是 `core::fmt` 要求的。
struct Logger;

/// 实现 `core::fmt::Write` trait，格式化的基础。
impl Write for Logger {
    #[inline]
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        let _ = CONSOLE.get().unwrap().put_str(s);
        Ok(())
    }
}

/// 实现 `log::Log` trait，提供分级日志。
///
/// > **NOTICE** 强行塞一个如此简单的实现只是为了使用方便。但强行塞一个复杂的实现也是一样。将这个实现留给用户自己实现也是合适的。
impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        use log::Level::*;
        let color_code: u8 = match record.level() {
            Error => 31,
            Warn => 93,
            Info => 34,
            Debug => 32,
            Trace => 90,
        };
        println!(
            "\x1b[{color_code}m[{:>5}] {}\x1b[0m",
            record.level(),
            record.args(),
        );
    }

    fn flush(&self) {}
}
