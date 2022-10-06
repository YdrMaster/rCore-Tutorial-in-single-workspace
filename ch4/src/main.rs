#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod mm;
mod process;

#[macro_use]
extern crate console;

#[macro_use]
extern crate alloc;

use crate::{
    impls::{Sv39Manager, SyscallContext},
    process::Process,
};
use alloc::vec::Vec;
use console::log;
use impls::Console;
use kernel_context::foreign::ForeignPortal;
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN},
    AddressSpace,
};
use riscv::register::*;
use sbi_rt::*;
use syscall::Caller;
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

static mut PROCESSES: Vec<Process> = Vec::new();

extern "C" fn rust_main() -> ! {
    let layout = linker::KernelLayout::locate();
    // bss 段清零
    unsafe { layout.zero_bss() };
    // 初始化 `console`
    console::init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();
    // 初始化 syscall
    syscall::init_io(&SyscallContext);
    syscall::init_process(&SyscallContext);
    syscall::init_scheduling(&SyscallContext);
    syscall::init_clock(&SyscallContext);
    // 初始化内核堆
    mm::init();
    mm::test();
    // 建立内核地址空间
    let mut ks = kernel_space(layout);
    // 加载应用程序
    for (i, elf) in linker::AppMeta::locate().iter().enumerate() {
        let base = elf.as_ptr() as usize;
        log::info!("detect app[{i}]: {base:#x}..{:#x}", base + elf.len());
        if let Some(process) = Process::new(ElfFile::new(elf).unwrap()) {
            unsafe { PROCESSES.push(process) };
        }
    }
    // 异界传送门
    // 可以直接放在栈上
    let mut portal = ForeignPortal::new();
    // 传送门映射到所有地址空间
    map_portal(&mut ks, &portal);
    unsafe {
        PROCESSES
            .iter_mut()
            .for_each(|proc| map_portal(&mut proc.address_space, &portal))
    };
    const PROTAL_TRANSIT: usize = VPN::<Sv39>::MAX.base().val();
    while !unsafe { PROCESSES.is_empty() } {
        let ctx = unsafe { &mut PROCESSES[0].context };
        unsafe { ctx.execute(&mut portal, PROTAL_TRANSIT) };
        match scause::read().cause() {
            scause::Trap::Exception(scause::Exception::UserEnvCall) => {
                use syscall::{SyscallId as Id, SyscallResult as Ret};

                let ctx = &mut ctx.context;
                let id: Id = ctx.a(7).into();
                let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                match syscall::handle(Caller { entity: 0, flow: 0 }, id, args) {
                    Ret::Done(ret) => match id {
                        Id::EXIT => unsafe {
                            PROCESSES.remove(0);
                        },
                        _ => {
                            *ctx.a_mut(0) = ret as _;
                            ctx.move_next();
                        }
                    },
                    Ret::Unsupported(_) => {
                        log::info!("id = {id:?}");
                        unsafe { PROCESSES.remove(0) };
                    }
                }
            }
            e => {
                log::error!("unsupported trap: {e:?}");
                unsafe { PROCESSES.remove(0) };
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

fn kernel_space(layout: linker::KernelLayout) -> AddressSpace<Sv39, Sv39Manager> {
    // 打印段位置
    let text = VAddr::<Sv39>::new(layout.text);
    let rodata = VAddr::<Sv39>::new(layout.rodata);
    let data = VAddr::<Sv39>::new(layout.data);
    let end = VAddr::<Sv39>::new(layout.end);
    log::info!("__text ----> {:#10x}", text.val());
    log::info!("__rodata --> {:#10x}", rodata.val());
    log::info!("__data ----> {:#10x}", data.val());
    log::info!("__end -----> {:#10x}", end.val());
    println!();

    // 内核地址空间
    let mut space = AddressSpace::<Sv39, Sv39Manager>::new();
    space.map_extern(
        text.floor()..rodata.ceil(),
        PPN::new(text.floor().val()),
        VmFlags::build_from_str("X_RV"),
    );
    space.map_extern(
        rodata.floor()..data.ceil(),
        PPN::new(rodata.floor().val()),
        VmFlags::build_from_str("__RV"),
    );
    space.map_extern(
        data.floor()..end.ceil(),
        PPN::new(data.floor().val()),
        VmFlags::build_from_str("_WRV"),
    );
    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().val()) };
    space
}

#[inline]
fn map_portal(space: &mut AddressSpace<Sv39, Sv39Manager>, portal: &ForeignPortal) {
    const PORTAL: VPN<Sv39> = VPN::MAX; // 虚地址最后一页给传送门
    space.map_extern(
        PORTAL..PORTAL + 1,
        PPN::new(portal as *const _ as usize >> Sv39::PAGE_BITS),
        VmFlags::build_from_str("XWRV"),
    );
}

/// 各种接口库的实现。
mod impls {
    use crate::{mm::PAGE, PROCESSES};
    use alloc::alloc::handle_alloc_error;
    use console::log;
    use core::{alloc::Layout, num::NonZeroUsize, ptr::NonNull};
    use kernel_vm::{
        page_table::{MmuMeta, Pte, Sv39, VAddr, VmFlags, PPN, VPN},
        PageManager,
    };
    use syscall::*;

    #[repr(transparent)]
    pub struct Sv39Manager(NonNull<Pte<Sv39>>);

    impl Sv39Manager {
        const OWNED: VmFlags<Sv39> = unsafe { VmFlags::from_raw(1 << 8) };
    }

    impl PageManager<Sv39> for Sv39Manager {
        #[inline]
        fn new_root() -> Self {
            const SIZE: usize = 1 << Sv39::PAGE_BITS;
            unsafe {
                match PAGE.allocate(Sv39::PAGE_BITS, NonZeroUsize::new_unchecked(SIZE)) {
                    Ok((ptr, _)) => Self(ptr),
                    Err(_) => handle_alloc_error(Layout::from_size_align_unchecked(SIZE, SIZE)),
                }
            }
        }

        #[inline]
        fn root_ppn(&self) -> PPN<Sv39> {
            PPN::new(self.0.as_ptr() as usize >> Sv39::PAGE_BITS)
        }

        #[inline]
        fn root_ptr(&self) -> NonNull<Pte<Sv39>> {
            self.0
        }

        #[inline]
        fn p_to_v<T>(&self, ppn: PPN<Sv39>) -> NonNull<T> {
            unsafe { NonNull::new_unchecked(VPN::<Sv39>::new(ppn.val()).base().as_mut_ptr()) }
        }

        #[inline]
        fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Sv39> {
            PPN::new(VAddr::<Sv39>::new(ptr.as_ptr() as _).floor().val())
        }

        #[inline]
        fn check_owned(&self, pte: Pte<Sv39>) -> bool {
            pte.flags().contains(Self::OWNED)
        }

        fn allocate(&mut self, len: usize, flags: &mut VmFlags<Sv39>) -> NonNull<u8> {
            unsafe {
                match PAGE.allocate(
                    Sv39::PAGE_BITS,
                    NonZeroUsize::new_unchecked(len << Sv39::PAGE_BITS),
                ) {
                    Ok((ptr, size)) => {
                        assert_eq!(size, len << Sv39::PAGE_BITS);
                        *flags |= Self::OWNED;
                        ptr
                    }
                    Err(_) => handle_alloc_error(Layout::from_size_align_unchecked(
                        len << Sv39::PAGE_BITS,
                        1 << Sv39::PAGE_BITS,
                    )),
                }
            }
        }

        fn deallocate(&mut self, _pte: Pte<Sv39>, _len: usize) -> usize {
            todo!()
        }

        fn drop_root(&mut self) {
            todo!()
        }
    }

    pub struct Console;

    impl console::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            #[allow(deprecated)]
            sbi_rt::legacy::console_putchar(c as _);
        }
    }

    pub struct SyscallContext;

    impl IO for SyscallContext {
        fn write(&self, caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            match fd {
                STDOUT | STDDEBUG => {
                    const READABLE: VmFlags<Sv39> = VmFlags::build_from_str("RV");
                    if let Some(ptr) = unsafe { PROCESSES.get_mut(caller.entity) }
                        .unwrap()
                        .address_space
                        .translate(VAddr::new(buf), READABLE)
                    {
                        print!("{}", unsafe {
                            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                                ptr.as_ptr(),
                                count,
                            ))
                        });
                        count as _
                    } else {
                        log::error!("ptr not readable");
                        -1
                    }
                }
                _ => {
                    console::log::error!("unsupported fd: {fd}");
                    -1
                }
            }
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: Caller, _status: usize) -> isize {
            0
        }
    }

    impl Scheduling for SyscallContext {
        #[inline]
        fn sched_yield(&self, _caller: Caller) -> isize {
            0
        }
    }

    impl Clock for SyscallContext {
        #[inline]
        fn clock_gettime(&self, caller: Caller, clock_id: ClockId, tp: usize) -> isize {
            const WRITABLE: VmFlags<Sv39> = VmFlags::build_from_str("W_V");
            match clock_id {
                ClockId::CLOCK_MONOTONIC => {
                    if let Some(mut ptr) = unsafe { PROCESSES.get(caller.entity) }
                        .unwrap()
                        .address_space
                        .translate(VAddr::new(tp), WRITABLE)
                    {
                        let time = riscv::register::time::read() * 10000 / 125;
                        *unsafe { ptr.as_mut() } = TimeSpec {
                            tv_sec: time / 1_000_000_000,
                            tv_nsec: time % 1_000_000_000,
                        };
                        0
                    } else {
                        log::error!("ptr not readable");
                        -1
                    }
                }
                _ => -1,
            }
        }
    }
}
