#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]

pub use output::{print, println};

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
    exit(main());
    unreachable!()
}

#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}

#[inline]
pub fn write(fd: usize, buf: &[u8]) -> isize {
    syscall::write(fd, buf)
}

#[inline]
pub fn exit(exit_code: i32) -> isize {
    syscall::exit(exit_code)
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
