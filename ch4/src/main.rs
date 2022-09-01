#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod mm;

#[macro_use]
extern crate output;

#[macro_use]
extern crate alloc;

use ::page_table::{Sv39, VAddr};
use alloc::vec::Vec;
use core::alloc::Layout;
use impls::Console;
use kernel_context::LocalContext;
use kernel_vm::AddressSpace;
use mm::PAGE;
use output::log;
use page_table::{MmuMeta, VmFlags, PPN, VPN};
use riscv::register::satp;
use sbi_rt::*;
use xmas_elf::{
    header::{self, HeaderPt2, Machine},
    ElfFile,
};

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
    // 初始化内核堆
    mm::init();
    mm::test();
    // 建立内核地址空间
    let _ks = kernel_space();
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
    // let _portal = ForeignPortal::EMPTY;
    // portal.runtime_init((!0) << 12);

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

fn kernel_space() -> AddressSpace<Sv39> {
    // 打印段位置
    extern "C" {
        fn __text();
        fn __transit();
        fn __rodata();
        fn __data();
        fn __end();
    }
    let _text = VAddr::<Sv39>::new(__text as _);
    let _transit = VAddr::<Sv39>::new(__transit as _);
    let _rodata = VAddr::<Sv39>::new(__rodata as _);
    let _data = VAddr::<Sv39>::new(__data as _);
    let _end = VAddr::<Sv39>::new(__end as _);
    log::info!("__text ----> {:#10x}", _text.val());
    log::info!("__transit -> {:#10x}", _transit.val());
    log::info!("__rodata --> {:#10x}", _rodata.val());
    log::info!("__data ----> {:#10x}", _data.val());
    log::info!("__end -----> {:#10x}", _end.val());
    println!();

    // 内核地址空间
    let mut space = AddressSpace::<Sv39>::new(0);
    space.push(
        _text.floor().._transit.ceil(),
        PPN::new(_text.floor().val()),
        unsafe { VmFlags::from_raw(0b1011) },
    );
    space.push(
        _transit.floor().._rodata.ceil(),
        PPN::new(_transit.floor().val()),
        unsafe { VmFlags::from_raw(0b1111) },
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

/// 进程。
struct Process {
    _context: LocalContext,
    _address_space: AddressSpace<Sv39>,
}

impl Process {
    fn new(elf: ElfFile) -> Option<Self> {
        let entry = match elf.header.pt2 {
            HeaderPt2::Header64(pt2)
                if pt2.type_.as_type() == header::Type::Executable
                    && pt2.machine.as_machine() == Machine::RISC_V =>
            {
                pt2.entry_point as usize
            }
            _ => None?,
        };

        let mut address_space = AddressSpace::<Sv39>::new(0);
        for program in elf.program_iter() {
            if !matches!(program.get_type(), Ok(xmas_elf::program::Type::Load)) {
                continue;
            }

            const PAGE_MASK: usize = (1 << 12) - 1;

            let off_file = program.offset() as usize;
            let len_file = program.file_size() as usize;
            let off_mem = program.virtual_addr() as usize;
            let end_mem = off_mem + program.mem_size() as usize;
            assert_eq!(off_file & PAGE_MASK, off_mem & PAGE_MASK);

            let svpn = VAddr::<Sv39>::new(off_mem).floor();
            let evpn = VAddr::<Sv39>::new(end_mem).ceil();
            let (pages, size) = unsafe {
                PAGE.allocate_layout::<u8>(Layout::from_size_align_unchecked(
                    (evpn.val() - svpn.val()) << 12,
                    1 << 12,
                ))
                .unwrap()
            };
            assert_eq!(size, (evpn.val() - svpn.val()) << 12);

            let ptr = pages.as_ptr();
            let off_inside = off_mem & PAGE_MASK;
            unsafe {
                use core::slice::from_raw_parts_mut;
                from_raw_parts_mut(ptr, off_inside).fill(0);
                pages
                    .as_ptr()
                    .add(off_inside)
                    .copy_from_nonoverlapping(elf.input[off_file..].as_ptr(), len_file);
                let off_inside = off_inside + len_file;
                from_raw_parts_mut(ptr.add(off_inside), size - off_inside).fill(0);
                let mut flags = 1usize;
                if program.flags().is_read() {
                    flags |= 0b0010;
                }
                if program.flags().is_write() {
                    flags |= 0b0100;
                }
                if program.flags().is_execute() {
                    flags |= 0b1000;
                }
                address_space.push(
                    svpn..evpn,
                    PPN::new(ptr as usize >> 12),
                    VmFlags::from_raw(flags),
                );
            }
        }
        unsafe {
            const STACK_SIZE: usize = 2 << Sv39::PAGE_BITS;
            let (pages, size) = PAGE
                .allocate_layout::<u8>(Layout::from_size_align_unchecked(STACK_SIZE, 1 << 12))
                .unwrap();
            assert_eq!(size, STACK_SIZE);
            core::slice::from_raw_parts_mut(pages.as_ptr(), STACK_SIZE).fill(0);
            address_space.push(
                VPN::new((1 << 26) - 2)..VPN::new(1 << 26),
                PPN::new(pages.as_ptr() as usize >> 12),
                VmFlags::from_raw(0b0111),
            );
        }

        log::info!("kernel page count = {:?}", address_space.page_count());
        for seg in address_space.segments() {
            log::info!("{seg}");
        }

        Some(Self {
            _context: LocalContext::user(entry),
            _address_space: address_space,
        })
    }
}
