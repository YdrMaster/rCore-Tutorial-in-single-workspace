#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]

pub use output::print;
pub use output::println;

use syscall::*;

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    zero_bss();
    exit(main());
    panic!("unreachable after sys_exit!");
}

#[linkage = "weak"]
#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}

/// 清零 bss 段
#[inline(always)]
fn zero_bss() {
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
}

#[inline]
pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}

#[inline]
pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    let err = panic_info.message().unwrap();
    if let Some(location) = panic_info.location() {
        println!(
            "Panicked at {}:{}, {}",
            location.file(),
            location.line(),
            err
        );
    } else {
        println!("Panicked: {}", err);
    }
    loop {}
}
