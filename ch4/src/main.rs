#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

#[macro_use]
extern crate output;

#[macro_use]
extern crate alloc;

use ::page_table::{Sv39, VAddr};
use impls::Console;
use kernel_vm::AddressSpace;
use output::log;
use page_table::{VmFlags, PPN};
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
    mm::init();

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
    // println!("{:?}", space.shuttle().unwrap());
    println!("page count = {:?}", space.page_count());
    for seg in space.segments() {
        println!("{seg}");
    }
    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().unwrap().val()) };
    // 测试内核堆分配
    {
        let mut vec = vec![0; 256];
        for (i, val) in vec.iter_mut().enumerate() {
            *val = i;
        }
        println!("{vec:?}");
        println!();
    }

    {
        use xmas_elf::{
            header::{self, HeaderPt2, Machine},
            ElfFile,
        };
        // 加载应用程序
        extern "C" {
            static apps: utils::AppMeta;
        }
        for (i, elf) in unsafe { apps.iter_elf() }.enumerate() {
            println!(
                "detect app[{i}] at {:?} (size: {} bytes)",
                elf.as_ptr(),
                elf.len()
            );
            let elf = ElfFile::new(elf).unwrap();
            if let HeaderPt2::Header64(pt2) = elf.header.pt2 {
                if pt2.type_.as_type() != header::Type::Executable
                    || pt2.machine.as_machine() != Machine::RISC_V
                {
                    continue;
                }
                for _segment in elf.program_iter() {}
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

mod mm {
    use alloc::alloc::handle_alloc_error;
    use buddy_allocator::{BuddyAllocator, LinkedListBuddy, UsizeBuddy};
    use core::{
        alloc::{GlobalAlloc, Layout},
        ptr::NonNull,
    };

    /// 初始化全局分配器和内核堆分配器。
    pub fn init() {
        /// 4 KiB 页类型。
        #[repr(C, align(4096))]
        pub struct Pages<const N: usize>([u8; N]);

        const MEMORY_SIZE: usize = 4 << 20;

        /// 托管空间 4 MiB
        static mut MEMORY: Pages<MEMORY_SIZE> = Pages([0u8; MEMORY_SIZE]);
        unsafe {
            let ptr = NonNull::new(MEMORY.0.as_mut_ptr()).unwrap();
            println!(
                "MEMORY = {:#x}..{:#x}",
                ptr.as_ptr() as usize,
                ptr.as_ptr() as usize + MEMORY_SIZE
            );
            PAGE.init(12, ptr);
            HEAP.init(3, ptr);
            PAGE.transfer(ptr, MEMORY_SIZE);
            kernel_vm::init_allocator(&PageAllocator);
        }
    }

    type MutAllocator<const N: usize> = BuddyAllocator<N, UsizeBuddy, LinkedListBuddy>;
    static mut PAGE: MutAllocator<5> = MutAllocator::new();
    static mut HEAP: MutAllocator<22> = MutAllocator::new();

    struct GlobAllocator;
    struct PageAllocator;

    impl kernel_vm::PageAllocator for PageAllocator {
        #[inline]
        fn allocate(&self, bits: usize) -> NonNull<u8> {
            let size = 1 << bits;
            unsafe { PAGE.allocate_layout(Layout::from_size_align_unchecked(size, size)) }
                .unwrap()
                .0
        }

        #[inline]
        fn deallocate(&self, ptr: NonNull<u8>, bits: usize) {
            unsafe { PAGE.deallocate(ptr, 1 << bits) };
        }
    }

    #[global_allocator]
    static GLOBAL: GlobAllocator = GlobAllocator;

    unsafe impl GlobalAlloc for GlobAllocator {
        #[inline]
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            loop {
                if let Ok((ptr, _)) = HEAP.allocate_layout::<u8>(layout) {
                    return ptr.as_ptr();
                } else if let Ok((ptr, size)) = PAGE.allocate_layout::<u8>(layout) {
                    HEAP.transfer(ptr, size);
                } else {
                    handle_alloc_error(layout)
                }
            }
        }

        #[inline]
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            HEAP.deallocate(NonNull::new(ptr).unwrap(), layout.size())
        }
    }
}
