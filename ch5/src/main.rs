#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

#[macro_use]
extern crate output;

// #[macro_use]
extern crate alloc;

mod loader;
mod mm;
mod page_table;

use crate::page_table::KernelSpaceBuilder;
use crate::mm::global;
use ::page_table::{PageTable, PageTableShuttle, Sv39, VAddr, VmMeta, VPN};
use impls::Console;
use output::log;
use riscv::register::satp;
use sbi_rt::*;



// 应用程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));

/// Supervisor 汇编入口。
///
/// 设置栈并跳转到 Rust。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 4 * 4096;

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
    extern "C" {
        fn __text();
        fn __transit();
        fn __rodata();
        fn __data();
        fn __end();
    }
    log::info!("__text ----> {:#10x}", __text as usize);
    log::info!("__transit -> {:#10x}", __transit as usize);
    log::info!("__rodata --> {:#10x}", __rodata as usize);
    log::info!("__data ----> {:#10x}", __data as usize);
    log::info!("__end -----> {:#10x}", __end as usize);
    println!();
    mm::init();

    // 内核地址空间
    {
        let kernel_root = mm::Page::ZERO;
        let kernel_root = VAddr::<Sv39>::new(kernel_root.addr());
        let table = unsafe {
            PageTable::<Sv39>::from_raw_parts(
                kernel_root.val() as *mut _,
                VPN::ZERO,
                Sv39::MAX_LEVEL,
            )
        };
        let mut shuttle = PageTableShuttle {
            table,
            f: |ppn| VPN::new(ppn.val()),
        };
        shuttle.walk_mut(KernelSpaceBuilder(unsafe { global() }));
        // println!("{shuttle:?}");
        unsafe { satp::set(satp::Mode::Sv39, 0, kernel_root.floor().val()) };
    }
    loader::list_apps();

    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    unreachable!()
}

/// Rust 异常处理函数，以异常方式关机。
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
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


