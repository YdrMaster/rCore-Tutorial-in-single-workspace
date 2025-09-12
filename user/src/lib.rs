#![no_std]

mod heap;

extern crate alloc;

use core::alloc::Layout;
use rcore_console::log;

pub use rcore_console::{print, println};
pub use syscall::*;

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG"));
    heap::init();

    extern "C" {
        fn main() -> i32;
    }

    exit(unsafe { main() });
    unreachable!()
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    let err = panic_info.message();
    if let Some(location) = panic_info.location() {
        log::error!("Panicked at {}:{}, {err}", location.file(), location.line());
    } else {
        log::error!("Panicked: {err}");
    }
    exit(1);
    unreachable!()
}

pub fn getchar() -> u8 {
    let mut c = [0u8; 1];
    read(STDIN, &mut c);
    c[0]
}

struct Console;

impl rcore_console::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        syscall::write(STDOUT, &[c]);
    }

    #[inline]
    fn put_str(&self, s: &str) {
        syscall::write(STDOUT, s.as_bytes());
    }
}

pub fn sleep(period_ms: usize) {
    let mut time: TimeSpec = TimeSpec::ZERO;
    clock_gettime(ClockId::CLOCK_MONOTONIC, &mut time as *mut _ as _);
    let time = time + TimeSpec::from_millsecond(period_ms);
    loop {
        let mut now: TimeSpec = TimeSpec::ZERO;
        clock_gettime(ClockId::CLOCK_MONOTONIC, &mut now as *mut _ as _);
        if now > time {
            break;
        }
        sched_yield();
    }
}
