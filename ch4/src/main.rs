#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod mm;
mod process;

#[macro_use]
extern crate output;

#[macro_use]
extern crate alloc;

use crate::{impls::SyscallContext, process::Process};
use ::page_table::{Sv39, VAddr};
use alloc::vec::Vec;
use impls::Console;
use kernel_context::foreign::ForeignPortal;
use kernel_vm::AddressSpace;
use output::log;
use page_table::{MmuMeta, VmFlags, PPN, VPN};
use riscv::register::*;
use sbi_rt::*;
use xmas_elf::ElfFile;

// 应用程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));

/// Supervisor 汇编入口。
///
/// 设置栈并跳转到 Rust。
#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 6 * 4096;

    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::asm!(
        "la sp, {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack      =   sym STACK,
        main       =   sym rust_main,
        options(noreturn),
    )
}

extern "C" fn rust_main() -> ! {
    // bss 段清零
    utils::zero_bss();
    // 初始化 `output`
    output::init_console(&Console);
    output::set_log_level(option_env!("LOG"));
    output::test_log();
    // 初始化 syscall
    syscall::init_io(&SyscallContext);
    syscall::init_process(&SyscallContext);
    syscall::init_scheduling(&SyscallContext);
    syscall::init_clock(&SyscallContext);
    // 初始化内核堆
    mm::init();
    mm::test();
    // 建立内核地址空间
    let mut ks = kernel_space();
    let mut processes = Vec::<Process>::new();
    // 加载应用程序
    extern "C" {
        static apps: utils::AppMeta;
    }
    for (i, elf) in unsafe { apps.iter_elf() }.enumerate() {
        let base = elf.as_ptr() as usize;
        println!("detect app[{i}]: {base:#x}..{:#x}", base + elf.len());
        if let Some(process) = Process::new(ElfFile::new(elf).unwrap()) {
            processes.push(process);
        }
    }
    // 异界传送门
    // 可以直接放在栈上
    let mut portal = ForeignPortal::new();
    // 传送门映射到所有地址空间
    map_portal(&mut ks, &portal);
    processes
        .iter_mut()
        .for_each(|proc| map_portal(&mut proc.address_space, &portal));
    let ctx = &mut processes[0].context;
    loop {
        unsafe { ctx.execute(&mut portal, !0 << Sv39::PAGE_BITS) };
        match scause::read().cause() {
            scause::Trap::Exception(scause::Exception::UserEnvCall) => {
                use syscall::SyscallId as Id;

                let ctx = &mut ctx.context;
                let id: Id = ctx.a(7).into();
                log::info!("id = {id:?}");
                break;
                // let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                // match syscall::handle(id, args) {
                //     Ret::Done(ret) => match id {
                //         Id::EXIT => break,
                //         _ => {
                //             *ctx.a_mut(0) = ret as _;
                //             ctx.move_next();
                //         }
                //     },
                //     Ret::Unsupported(id) => {
                //         log::error!("unsupported syscall: {id:?}");
                //         break;
                //     }
                // }
            }
            e => {
                log::error!("unsupported trap: {e:?}");
                break;
            }
        }
    }
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

fn kernel_space() -> AddressSpace<Sv39> {
    // 打印段位置
    extern "C" {
        fn __text();
        fn __rodata();
        fn __data();
        fn __end();
    }
    let _text = VAddr::<Sv39>::new(__text as _);
    let _rodata = VAddr::<Sv39>::new(__rodata as _);
    let _data = VAddr::<Sv39>::new(__data as _);
    let _end = VAddr::<Sv39>::new(__end as _);
    log::info!("__text ----> {:#10x}", _text.val());
    log::info!("__rodata --> {:#10x}", _rodata.val());
    log::info!("__data ----> {:#10x}", _data.val());
    log::info!("__end -----> {:#10x}", _end.val());
    println!();

    // 内核地址空间
    let mut space = AddressSpace::<Sv39>::new(0);
    space.push(
        _text.floor().._rodata.ceil(),
        PPN::new(_text.floor().val()),
        unsafe { VmFlags::from_raw(0b1011) },
    );
    space.push(
        _rodata.floor().._data.ceil(),
        PPN::new(_rodata.floor().val()),
        unsafe { VmFlags::from_raw(0b0011) },
    );
    space.push(
        _data.floor().._end.ceil(),
        PPN::new(_data.floor().val()),
        unsafe { VmFlags::from_raw(0b0111) },
    );
    // log::debug!("\n{:?}", space.shuttle().unwrap());
    log::info!("kernel page count = {:?}", space.page_count());
    for seg in space.segments() {
        log::info!("{seg}");
    }
    println!();
    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().unwrap().val()) };
    space
}

#[inline]
fn map_portal(space: &mut AddressSpace<Sv39>, portal: &ForeignPortal) {
    const PORTAL: VPN<Sv39> = VPN::MAX; // 虚地址最后一页给传送门
    const FLAGS: VmFlags<Sv39> = unsafe { VmFlags::from_raw(0b1111) };
    space.push(
        PORTAL..PORTAL + 1,
        PPN::new(portal as *const _ as usize >> Sv39::PAGE_BITS),
        FLAGS,
    );
}

/// 各种接口库的实现。
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
