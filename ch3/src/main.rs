#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

mod task;

#[macro_use]
extern crate output;

use impls::{Console, SyscallContext};
use output::log;
use riscv::register::*;
use sbi_rt::*;
use task::TaskControlBlock;

// 应用程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));

// 应用程序数量。
const APP_CAPACITY: usize = 32;

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

    // 初始化 syscall
    syscall::init_io(&SyscallContext);
    syscall::init_process(&SyscallContext);
    syscall::init_scheduling(&SyscallContext);
    syscall::init_clock(&SyscallContext);
    // 确定应用程序位置
    extern "C" {
        static apps: utils::AppMeta;
    }
    // 任务控制块
    static mut TCBS: [TaskControlBlock; APP_CAPACITY] = [TaskControlBlock::ZERO; APP_CAPACITY];
    // 初始化
    for i in 0..unsafe { apps.len() } {
        let app_base = unsafe { apps.load(i) };
        log::info!("load app{i} to {app_base:#x}");
        unsafe { TCBS[i].init(app_base) };
    }
    println!();
    // 打开中断
    unsafe { sie::set_stimer() };
    // 设置陷入地址
    unsafe { stvec::write(kernel_context::trap as _, stvec::TrapMode::Direct) };
    // 多道执行
    let index_mod = unsafe { apps.len() } as usize;
    let mut remain = index_mod;
    let mut i = 0usize;
    while remain > 0 {
        let tcb = unsafe { &mut TCBS[i] };
        if !tcb.finish {
            loop {
                #[cfg(not(feature = "coop"))]
                sbi_rt::set_timer(time::read64() + 12500);
                unsafe { tcb.execute() };

                use scause::*;
                let finish = match scause::read().cause() {
                    Trap::Interrupt(Interrupt::SupervisorTimer) => {
                        sbi_rt::set_timer(u64::MAX);
                        log::trace!("app{i} timeout");
                        false
                    }
                    Trap::Exception(Exception::UserEnvCall) => {
                        use task::SchedulingEvent as Event;
                        match tcb.handle_syscall() {
                            Event::None => continue,
                            Event::Exit(code) => {
                                log::info!("app{i} exit with code {code}");
                                true
                            }
                            Event::Yield => {
                                log::debug!("app{i} yield");
                                false
                            }
                            Event::UnsupportedSyscall(id) => {
                                log::error!("app{i} call an unsupported syscall {}", id.0);
                                true
                            }
                        }
                    }
                    Trap::Exception(e) => {
                        log::error!("app{i} was killed by {e:?}");
                        true
                    }
                    Trap::Interrupt(ir) => {
                        log::error!("app{i} was killed by an unexpected interrupt {ir:?}");
                        true
                    }
                };
                if finish {
                    tcb.finish = true;
                    remain -= 1;
                }
                break;
            }
        }
        i = (i + 1) % index_mod;
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

/// 各种接口库的实现
mod impls {
    use syscall::*;

    pub struct Console;

    impl output::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            #[allow(deprecated)]
            sbi_rt::legacy::console_putchar(c as _);
        }
    }

    pub struct SyscallContext;

    impl IO for SyscallContext {
        #[inline]
        fn write(&self, fd: usize, buf: usize, count: usize) -> isize {
            use output::log::*;

            if fd == 0 {
                print!("{}", unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        buf as *const u8,
                        count,
                    ))
                });
                count as _
            } else {
                error!("unsupported fd: {fd}");
                -1
            }
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _status: usize) -> isize {
            0
        }
    }

    impl Scheduling for SyscallContext {
        #[inline]
        fn sched_yield(&self) -> isize {
            0
        }
    }

    impl Clock for SyscallContext {
        #[inline]
        fn clock_gettime(&self, clock_id: ClockId, tp: usize) -> isize {
            match clock_id {
                ClockId::CLOCK_MONOTONIC => {
                    let time = riscv::register::time::read() * 10000 / 125;
                    *unsafe { &mut *(tp as *mut TimeSpec) } = TimeSpec {
                        tv_sec: time / 1_000_000_000,
                        tv_nsec: time % 1_000_000_000,
                    };
                    0
                }
                _ => -1,
            }
        }
    }
}
