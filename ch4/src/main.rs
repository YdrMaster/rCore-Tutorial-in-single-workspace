#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
// #![deny(warnings)]

mod frame_allocator;

// #[macro_use]
// extern crate output;

use impls::Console;
use output::log;
use sbi_rt::*;

// 应用程序内联进来。
// core::arch::global_asm!(include_str!(env!("APP_ASM")));

/// Supervisor 汇编入口。
///
/// 设置栈并跳转到 Rust。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 2 * 4096;

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

extern "C" fn rust_main() -> ! {
    // bss 段清零
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
    // 初始化 `output`
    output::init_console(&Console);
    output::set_log_level(option_env!("LOG"));
    utils::test_log();
    // 打印段位置
    #[link_section = ".trampoline"]
    static _PLACE_HOLDER: u8 = 0;
    extern "C" {
        fn __text();
        fn __trampoline();
        fn __rodata();
        fn __data();
        fn __end();
    }
    log::info!("__text -------> {:#10x}", __text as usize);
    log::info!("__trampoline -> {:#10x}", __trampoline as usize);
    log::info!("__rodata -----> {:#10x}", __rodata as usize);
    log::info!("__data -------> {:#10x}", __data as usize);
    log::info!("__end --------> {:#10x}", __end as usize);

    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    unreachable!()
}

/// Rust 异常处理函数，以异常方式关机。
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
    unreachable!()
}

/// 各种接口库的实现
mod impls {
    pub struct Console;

    impl output::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            #[allow(deprecated)]
            sbi_rt::legacy::console_putchar(c as _);
        }
    }
}
