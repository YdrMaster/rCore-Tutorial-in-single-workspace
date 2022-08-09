#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

use core::ops::Range;
use output::*;
use riscv::register::*;
use sbi_rt::*;

// 用户程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));

// 用户程序的地址也要传进来。
const APP_BASE: &str = env!("APP_BASE");

/// Supervisor 汇编入口。
///
/// 设置栈并跳转到 Rust。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 4096;

    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::asm!(
        "   la  sp, {stack}
            li  t0, {stack_size}
            add sp, sp, t0
            j   {main}
        ",
        stack_size = const STACK_SIZE,
        stack      =   sym STACK,
        main       =   sym rust_main,
        options(noreturn),
    )
}

struct Console;

impl output::Console for Console {
    fn put_char(&self, c: u8) {
        #[allow(deprecated)]
        legacy::console_putchar(c as _);
    }
}

extern "C" fn rust_main() -> ! {
    init_console(&Console);
    log::set_max_level(output::log::LevelFilter::Trace);

    extern "C" {
        static mut _num_app: u64;
    }

    let ranges = unsafe {
        core::slice::from_raw_parts(
            (&_num_app as *const u64).add(1) as *const usize,
            (_num_app + 1) as _,
        )
    };

    for range in ranges.windows(2) {
        println!("{:#10x}..{:#10x}", range[0], range[1]);
    }

    let app_base = if let Some(num) = APP_BASE.strip_prefix("0x") {
        usize::from_str_radix(num, 16).unwrap()
    } else {
        usize::from_str_radix(APP_BASE, 10).unwrap()
    };

    println!("app_base: {app_base:#10x}");

    let mut uctx = trap_frame::UserContext::new(app_base);
    uctx.set_scratch();

    unsafe { stvec::write(trap_frame::u_to_s as _, stvec::TrapMode::Direct) };

    for range in ranges.windows(2) {
        load(range[0]..range[1], app_base);

        loop {
            unsafe { trap_frame::s_to_u() };

            use scause::{Exception, Trap};
            match scause::read().cause() {
                Trap::Exception(Exception::StoreFault) => {
                    println!("sepc = {:#x}", uctx.sepc);
                    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
                }
                trap => {
                    println!("{trap:?}");
                    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
                }
            }
        }
    }

    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    unreachable!()
}

/// Rust 异常处理函数，以异常方式关机。
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
    unreachable!()
}

/// 将一个应用程序加载到目标位置。
#[inline]
fn load(range: Range<usize>, base: usize) {
    unsafe { core::ptr::copy_nonoverlapping::<u8>(range.start as _, base as _, range.len()) };
}
