#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

#[macro_use]
extern crate output;

use impls::{Console, SyscallContext};
use kernel_context::LocalContext;
use output::log;
use riscv::register::*;
use sbi_rt::*;
use syscall::SyscallId;

// 用户程序内联进来。
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

    // 初始化 syscall
    syscall::init_io(&SyscallContext);
    syscall::init_process(&SyscallContext);
    // 确定应用程序位置
    extern "C" {
        static apps: utils::AppMeta;
    }
    // 批处理
    for (i, app_base) in unsafe { apps.iter_static() }.enumerate() {
        log::info!("load app{i} to {app_base:#x}");
        // 初始化上下文
        let mut ctx = LocalContext::user(app_base);
        // 设置用户栈
        let mut user_stack = [0u8; 4096];
        *ctx.sp_mut() = user_stack.as_mut_ptr() as usize + user_stack.len();
        // 执行应用程序
        loop {
            unsafe { ctx.execute() };

            use scause::{Exception, Trap};
            match scause::read().cause() {
                Trap::Exception(Exception::UserEnvCall) => {
                    use SyscallResult::*;
                    match handle_syscall(&mut ctx) {
                        Done => continue,
                        Exit(code) => log::info!("app{i} exit with code {code}"),
                        Error(id) => log::error!("app{i} call an unsupported syscall {}", id.0),
                    }
                }
                trap => log::error!("app{i} was killed because of {trap:?}"),
            }
            // 清除指令缓存
            unsafe { core::arch::asm!("fence.i") };
            break;
        }
        println!();
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

enum SyscallResult {
    Done,
    Exit(usize),
    Error(SyscallId),
}

/// 处理系统调用，返回是否应该终止程序。
fn handle_syscall(ctx: &mut LocalContext) -> SyscallResult {
    use syscall::{SyscallId as Id, SyscallResult as Ret};

    let id = ctx.a(7).into();
    let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
    match syscall::handle(id, args) {
        Ret::Done(ret) => match id {
            Id::EXIT => SyscallResult::Exit(ctx.a(0)),
            _ => {
                *ctx.a_mut(0) = ret as _;
                ctx.move_next();
                SyscallResult::Done
            }
        },
        Ret::Unsupported(id) => SyscallResult::Error(id),
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

    pub struct SyscallContext;

    impl syscall::IO for SyscallContext {
        fn write(&self, fd: usize, buf: usize, count: usize) -> isize {
            if fd == 0 {
                print!("{}", unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        buf as *const u8,
                        count,
                    ))
                });
                count as _
            } else {
                output::log::error!("unsupported fd: {fd}");
                -1
            }
        }
    }

    impl syscall::Process for SyscallContext {
        #[inline]
        fn exit(&self, _status: usize) -> isize {
            0
        }
    }
}
