#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

#[macro_use]
extern crate output;

#[macro_use]
extern crate alloc;

use impls::Console;
use output::log;
use sbi_rt::*;

// 应用程序内联进来。
// core::arch::global_asm!(include_str!(env!("APP_ASM")));

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
    // 打印段位置
    #[link_section = ".trampoline"]
    static _PLACE_HOLDER: u8 = 0;
    extern "C" {
        fn __text();
        fn __trampoline();
        fn __rodata();
        fn __data();
        fn __end();
    }
    log::info!("__text -------> {:#10x}", __text as usize);
    log::info!("__trampoline -> {:#10x}", __trampoline as usize);
    log::info!("__rodata -----> {:#10x}", __rodata as usize);
    log::info!("__data -------> {:#10x}", __data as usize);
    log::info!("__end --------> {:#10x}", __end as usize);
    println!();
    mm::init();
    {
        let mut vec = vec![0; 256];
        for (i, val) in vec.iter_mut().enumerate() {
            *val = i;
        }
        println!("{vec:?}");
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
        cell::RefCell,
        ptr::NonNull,
    };

    pub fn init() {
        let ptr = NonNull::new(unsafe { MEMORY.as_mut_ptr() }).unwrap();
        let len = core::mem::size_of_val(unsafe { &MEMORY });
        unsafe { GLOBAL.init(12, ptr) };
        unsafe { GLOBAL.transfer(ptr, len) };
        ALLOC.0.borrow_mut().init(3, ptr);
    }

    #[repr(C, align(4096))]
    struct Page([u8; 4096]);

    impl Page {
        const ZERO: Self = Self([0; 4096]);
    }

    /// 托管空间 8 MiB
    static mut MEMORY: [Page; 2048] = [Page::ZERO; 2048];
    static mut GLOBAL: MutAllocator<5> = MutAllocator::<5>::new();

    type MutAllocator<const N: usize> = BuddyAllocator<N, UsizeBuddy, LinkedListBuddy>;

    #[global_allocator]
    static ALLOC: SharedAllocator<22> = SharedAllocator(RefCell::new(MutAllocator::new()));

    unsafe impl<const N: usize> Sync for SharedAllocator<N> {}

    struct SharedAllocator<const N: usize>(RefCell<MutAllocator<N>>);

    unsafe impl<const N: usize> GlobalAlloc for SharedAllocator<N> {
        #[inline]
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let mut inner = self.0.borrow_mut();
            loop {
                if let Ok((ptr, _)) = inner.allocate::<u8>(layout) {
                    return ptr.as_ptr();
                } else if let Ok((ptr, size)) = unsafe { GLOBAL.allocate::<u8>(layout) } {
                    inner.transfer(ptr, size);
                } else {
                    handle_alloc_error(layout)
                }
            }
        }

        #[inline]
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            self.0
                .borrow_mut()
                .deallocate(NonNull::new(ptr).unwrap(), layout.size())
        }
    }
}
