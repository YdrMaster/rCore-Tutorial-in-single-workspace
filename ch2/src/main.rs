#![no_std]
#![no_main]
#![feature(naked_functions, asm_const)]
#![deny(warnings)]

#[macro_use]
extern crate rcore_console;

use impls::{Console, SyscallContext};
use kernel_context::LocalContext;
use rcore_console::log;
use riscv::register::*;
use sbi_rt::*;
use syscall::{Caller, SyscallId};

// 用户程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));
// 定义内核入口。
linker::boot0!(rust_main; stack = 4 * 4096);

extern "C" fn rust_main() -> ! {
    // bss 段清零
    unsafe { linker::KernelLayout::locate().zero_bss() };
    // 初始化 `console`
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG"));
    rcore_console::test_log();
    // 初始化 syscall
    syscall::init_io(&SyscallContext);
    syscall::init_process(&SyscallContext);
    // 批处理
    for (i, app) in linker::AppMeta::locate().iter().enumerate() {
        let app_base = app.as_ptr() as usize;
        log::info!("load app{i} to {app_base:#x}");
        // 初始化上下文
        let mut ctx = LocalContext::user(app_base);
        // 设置用户栈
        let mut user_stack = [0usize; 256];
        *ctx.sp_mut() = user_stack.as_mut_ptr() as usize + core::mem::size_of_val(&user_stack);
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

    system_reset(Shutdown, NoReason);
    unreachable!()
}

/// Rust 异常处理函数，以异常方式关机。
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    system_reset(Shutdown, SystemFailure);
    loop {}
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
    match syscall::handle(Caller { entity: 0, flow: 0 }, id, args) {
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
    use syscall::{STDDEBUG, STDOUT};

    pub struct Console;

    impl rcore_console::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            #[allow(deprecated)]
            sbi_rt::legacy::console_putchar(c as _);
        }
    }

    pub struct SyscallContext;

    impl syscall::IO for SyscallContext {
        fn write(&self, _caller: syscall::Caller, fd: usize, buf: usize, count: usize) -> isize {
            match fd {
                STDOUT | STDDEBUG => {
                    print!("{}", unsafe {
                        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                            buf as *const u8,
                            count,
                        ))
                    });
                    count as _
                }
                _ => {
                    rcore_console::log::error!("unsupported fd: {fd}");
                    -1
                }
            }
        }
    }

    impl syscall::Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: syscall::Caller, _status: usize) -> isize {
            0
        }
    }
}
