#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

#[macro_use]
extern crate output;

use core::ops::Range;
use impls::{Console, IOSyscall, ProcessSyscall};
use output::log;
use riscv::register::*;
use sbi_rt::*;
use syscall::SyscallId;
use trap_frame::{s_to_u, u_to_s, Context};

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

extern "C" fn rust_main() -> ! {
    // 初始化 `output`
    output::init_console(&Console);
    log::set_max_level(log::LevelFilter::Trace);

    println!("[PRINT] Hello, world!");
    log::trace!("Hello, world!");
    log::debug!("Hello, world!");
    log::info!("Hello, world!");
    log::warn!("Hello, world!");
    log::error!("Hello, world!");

    // 初始化 syscall
    syscall::init_io(&IOSyscall);
    syscall::init_process(&ProcessSyscall);
    // 应用程序位置
    let ranges = unsafe {
        extern "C" {
            static mut _num_app: u64;
        }

        core::slice::from_raw_parts(
            (&_num_app as *const u64).add(1) as *const usize,
            (_num_app + 1) as _,
        )
    };
    // 应用程序加载位置
    let app_base = if let Some(num) = APP_BASE.strip_prefix("0x") {
        usize::from_str_radix(num, 16).unwrap()
    } else {
        usize::from_str_radix(APP_BASE, 10).unwrap()
    };
    // 设置陷入响应地址
    unsafe { stvec::write(u_to_s as _, stvec::TrapMode::Direct) };
    // 批处理
    log::error!("log will break inside the loop!!!");
    for (i, range) in ranges.windows(2).enumerate() {
        println!();
        println!(
            "* load app{i} from {:#10x}..{:#10x} to {app_base:#10x}",
            range[0], range[1]
        );
        // 加载应用程序
        load(range[0]..range[1], app_base);
        // 初始化上下文
        let mut ctx = Context::new(app_base);
        ctx.set_scratch();
        // 设置用户栈
        let mut user_stack = [0u8; 4096];
        *ctx.sp_mut() = user_stack.as_mut_ptr() as usize + user_stack.len();
        // 执行应用程序
        loop {
            unsafe { s_to_u() };

            use scause::{Exception, Trap};
            match scause::read().cause() {
                Trap::Exception(Exception::UserEnvCall) => {
                    if let Some(code) = handle_syscall(&mut ctx) {
                        println!("> app{i} exit with code {code}",);
                        break;
                    }
                }
                trap => {
                    println!("> app{i} was killed because of {trap:?}");
                    break;
                }
            }
        }
        // 清除指令缓存
        unsafe { core::arch::asm!("fence.i") };
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

/// 处理系统调用，返回是否应该终止程序。
fn handle_syscall(ctx: &mut Context) -> Option<usize> {
    let id = ctx.a(7).into();
    let ret = syscall::handle(
        id,
        [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)],
    );
    match id {
        SyscallId::EXIT => Some(ctx.a(0)),
        _ => {
            *ctx.a_mut(0) = ret as _;
            ctx.sepc += 4;
            None
        }
    }
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

    pub struct IOSyscall;

    impl syscall::IO for IOSyscall {
        fn write(&self, fd: usize, buf: usize, count: usize) -> isize {
            if fd == 0 {
                output::print!("{}", unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        buf as *const u8,
                        count,
                    ))
                });
                count as _
            } else {
                output::println!("unsupported fd: {fd}");
                -1
            }
        }
    }

    pub struct ProcessSyscall;

    impl syscall::Process for ProcessSyscall {
        #[inline]
        fn exit(&self, _status: usize) -> isize {
            0
        }
    }
}
