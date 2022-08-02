#![no_std]
#![deny(warnings)]

use core::fmt::{Arguments, Write};
use log::{Level, Log};
use spin::Once;

pub extern crate log;

pub trait Console: Sync {
    fn put_char(&self, c: u8);

    fn write_str(&self, s: &str) {
        for c in s.bytes() {
            self.put_char(c);
        }
    }
}

static CONSOLE: Once<&'static dyn Console> = Once::new();

struct Logger;

impl Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        let _ = CONSOLE.get().unwrap().write_str(s);
        Ok(())
    }
}

pub fn init_console(console: &'static dyn Console) {
    CONSOLE.call_once(|| console);
    log::set_logger(&Logger).unwrap();
}

#[inline]
pub fn print(args: Arguments) {
    Logger.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::print(core::format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {{
        $crate::print(core::format_args!($($arg)*));
        $crate::print!("\n");
    }}
}

impl Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let color_code = match record.level() {
                Level::Error => "31",
                Level::Warn => "93",
                Level::Info => "34",
                Level::Debug => "32",
                Level::Trace => "90",
            };
            println!(
                "\x1b[{}m[{:>5}] {}\x1b[0m",
                color_code,
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
