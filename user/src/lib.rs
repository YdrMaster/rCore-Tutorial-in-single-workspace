#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]

pub use output::{print, println};
pub use syscall::*;

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    output::init_console(&Console);
    exit(main());
    unreachable!()
}

#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    let err = panic_info.message().unwrap();
    if let Some(location) = panic_info.location() {
        println!("Panicked at {}:{}, {err}", location.file(), location.line());
    } else {
        println!("Panicked: {err}");
    }
    exit(1);
    unreachable!()
}

struct Console;

impl output::Console for Console {
    #[inline]
    fn put_char(&self, c: u8) {
        syscall::write(0, &[c]);
    }

    #[inline]
    fn put_str(&self, s: &str) {
        syscall::write(0, s.as_bytes());
    }
}
