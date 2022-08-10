#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
// #![deny(warnings)]

mod task;

#[macro_use]
extern crate output;

use core::ops::Range;
use impls::{Console, IOSyscall, ProcessSyscall};
use output::log;
use riscv::register::*;
use sbi_rt::*;
use task::TaskControlBlock;

// 应用程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));

// 应用程序数量。
// const APP_COUNT: &str = env!("APP_COUNT");
const APP_COUNT: usize = 8;

// 应用程序地址基值。
const APP_BASE: &str = env!("APP_BASE");

// 每个应用程序地址偏移。
const APP_STEP: &str = env!("APP_STEP");

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
    // bss 段清零
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
    // 初始化 `output`
    output::init_console(&Console);
    log::set_max_level(log::LevelFilter::Trace);

    println!(
        r"
  ______        __                _         __
 /_  __/__  __ / /_ ____   _____ (_)____ _ / /
  / /  / / / // __// __ \ / ___// // __ `// /
 / /  / /_/ // /_ / /_/ // /   / // /_/ // /
/_/   \__,_/ \__/ \____//_/   /_/ \__,_//_/
==========================================="
    );
    log::trace!("LOG TEST >> Hello, world!");
    log::debug!("LOG TEST >> Hello, world!");
    log::info!("LOG TEST >> Hello, world!");
    log::warn!("LOG TEST >> Hello, world!");
    log::error!("LOG TEST >> Hello, world!");
    println!();

    // 初始化 syscall
    syscall::init_io(&IOSyscall);
    syscall::init_process(&ProcessSyscall);
    // 确定应用程序位置
    let ranges = unsafe {
        extern "C" {
            static mut _num_app: u64;
        }

        core::slice::from_raw_parts(
            (&_num_app as *const u64).add(1) as *const usize,
            (_num_app + 1) as _,
        )
    };
    let app_base = parse_num(APP_BASE);
    let app_step = parse_num(APP_STEP);
    // 任务控制块
    static mut TCBS: [TaskControlBlock; APP_COUNT] = [TaskControlBlock::ZERO; APP_COUNT];
    // 初始化
    for (i, range) in ranges.windows(2).enumerate() {
        let app_base = app_base + i * app_step;
        log::info!(
            "load app{i} from {:#10x}..{:#10x} to {app_base:#10x}",
            range[0],
            range[1],
        );
        load_app(range[0]..range[1], app_base);
        unsafe { TCBS[i].init(app_base) };
    }
    // 设置陷入地址
    unsafe { stvec::write(kernel_context::trap as _, stvec::TrapMode::Direct) };
    // 多道执行
    let index_mod = ranges.len() - 1;
    let mut remain = index_mod;
    let mut i = 0usize;
    while remain > 0 {
        let tcb = unsafe { &mut TCBS[i] };
        if !tcb.finish {
            loop {
                unsafe { tcb.execute() };

                use scause::{Exception, Trap};
                match scause::read().cause() {
                    Trap::Exception(Exception::UserEnvCall) => {
                        use task::SchedulingEvent as Event;
                        match tcb.handle_syscall() {
                            Event::None => {}
                            Event::Exit(code) => {
                                log::info!("app{i} exit with code {code}");
                                tcb.finish = true;
                                remain -= 1;
                                break;
                            }
                            Event::Yield => {
                                log::debug!("app{i} yield");
                                break;
                            }
                        }
                    }
                    Trap::Exception(e) => {
                        log::error!("app{i} was killed by {e:?}");
                        tcb.finish = true;
                        remain -= 1;
                        break;
                    }
                    Trap::Interrupt(ir) => {
                        log::error!("app{i} was killed by an unexpected interrupt {ir:?}");
                        tcb.finish = true;
                        remain -= 1;
                        break;
                    }
                }
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

/// 将一个应用程序加载到目标位置。
#[inline]
fn load_app(range: Range<usize>, base: usize) {
    unsafe { core::ptr::copy_nonoverlapping::<u8>(range.start as _, base as _, range.len()) };
}

#[inline]
fn parse_num(str: &str) -> usize {
    if let Some(num) = str.strip_prefix("0x") {
        usize::from_str_radix(num, 16).unwrap()
    } else {
        usize::from_str_radix(str, 10).unwrap()
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

    pub struct ProcessSyscall;

    impl syscall::Process for ProcessSyscall {
        #[inline]
        fn exit(&self, _status: usize) -> isize {
            0
        }
    }
}
