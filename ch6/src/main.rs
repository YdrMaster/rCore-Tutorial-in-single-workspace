#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod fs;
mod mm;
mod process;
mod virtio_block;

#[macro_use]
extern crate console;

#[macro_use]
extern crate alloc;

use crate::{
    fs::{read_all, FS},
    impls::{Sv39Manager, SyscallContext},
    process::{Process, TaskId},
};
use console::log;
use easy_fs::{FSManager, OpenFlags};
use impls::Console;
use kernel_context::foreign::ForeignPortal;
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN},
    AddressSpace,
};
use riscv::register::*;
use sbi_rt::*;
use spin::Once;
use syscall::Caller;
use task_manage::TaskManager;
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
    const STACK_SIZE: usize = 128 * 4096;

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

static mut KERNEL_SPACE: Once<AddressSpace<Sv39, Sv39Manager>> = Once::new();
static mut TASK_MANAGER: TaskManager<Process, TaskId> = TaskManager::new();

extern "C" fn rust_main() -> ! {
    // bss 段清零
    utils::zero_bss();
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
    unsafe { KERNEL_SPACE.call_once(kernel_space) };
    // 异界传送门
    // 可以直接放在栈上
    let mut portal = ForeignPortal::new();
    let tramp = (
        PPN::<Sv39>::new(&portal as *const _ as usize >> Sv39::PAGE_BITS),
        VmFlags::build_from_str("XWRV"),
    );
    // 传送门映射到所有地址空间
    unsafe { KERNEL_SPACE.get_mut().unwrap().map_portal(tramp) };
    // 加载应用程序
    // TODO!
    println!("/**** APPS ****");
    for app in FS.readdir("").unwrap() {
        println!("{}", app);
    }
    println!("**************/");
    {
        let initproc = read_all(FS.open("initproc", OpenFlags::RDONLY).unwrap());
        if let Some(mut process) = Process::from_elf(ElfFile::new(initproc.as_slice()).unwrap()) {
            process.address_space.map_portal(tramp);
            unsafe {
                TASK_MANAGER.insert(process.pid, process);
            }
        }
    }

    const PROTAL_TRANSIT: usize = VPN::<Sv39>::MAX.base().val();
    loop {
        if let Some(task) = unsafe { TASK_MANAGER.fetch() } {
            task.execute(&mut portal, PROTAL_TRANSIT);
            match scause::read().cause() {
                scause::Trap::Exception(scause::Exception::UserEnvCall) => {
                    use syscall::{SyscallId as Id, SyscallResult as Ret};
                    let ctx = &mut task.context.context;
                    ctx.move_next();
                    let id: Id = ctx.a(7).into();
                    let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                    match syscall::handle(Caller { entity: 0, flow: 0 }, id, args) {
                        Ret::Done(ret) => match id {
                            Id::EXIT => unsafe {
                                TASK_MANAGER.del(task.pid);
                            },
                            _ => {
                                let ctx =
                                    unsafe { &mut TASK_MANAGER.current().unwrap().context.context };
                                *ctx.a_mut(0) = ret as _;
                                unsafe {
                                    TASK_MANAGER.add(task.pid);
                                }
                            }
                        },
                        Ret::Unsupported(_) => {
                            log::info!("id = {id:?}");
                            unsafe {
                                TASK_MANAGER.del(task.pid);
                            }
                        }
                    }
                }
                e => {
                    log::error!("unsupported trap: {e:?}");
                    unsafe {
                        TASK_MANAGER.del(task.pid);
                    }
                }
            }
        } else {
            println!("no task");
            break;
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

pub const MMIO: &[(usize, usize)] = &[
    (0x1000_1000, 0x00_1000), // Virtio Block in virt machine
];

fn kernel_space() -> AddressSpace<Sv39, Sv39Manager> {
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
    let mut space = AddressSpace::<Sv39, Sv39Manager>::new();
    space.map_extern(
        _text.floor().._rodata.ceil(),
        PPN::new(_text.floor().val()),
        VmFlags::build_from_str("X_RV"),
    );
    space.map_extern(
        _rodata.floor().._data.ceil(),
        PPN::new(_rodata.floor().val()),
        VmFlags::build_from_str("__RV"),
    );
    space.map_extern(
        _data.floor().._end.ceil(),
        PPN::new(_data.floor().val()),
        VmFlags::build_from_str("_WRV"),
    );

    // MMIO
    for pair in MMIO {
        let _mmio_begin = VAddr::<Sv39>::new(pair.0);
        let _mmio_end = VAddr::<Sv39>::new(pair.0 + pair.1);
        log::info!(
            "MMIO range ---> {:#10x}, {:#10x} \n",
            _mmio_begin.val(),
            _mmio_end.val()
        );
        space.map_extern(
            _mmio_begin.floor().._mmio_end.ceil(),
            PPN::new(_mmio_begin.floor().val()),
            VmFlags::build_from_str("_WRV"),
        );
    }

    // log::debug!("{space:?}");
    println!();
    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().val()) };
    space
}

/// 各种接口库的实现。
mod impls {
    use crate::{
        fs::{read_all, FS},
        mm::PAGE,
        process::TaskId,
        TASK_MANAGER,
    };
    use alloc::vec::Vec;
    use alloc::{alloc::handle_alloc_error, string::String};
    use console::log;
    use core::{alloc::Layout, num::NonZeroUsize, ptr::NonNull};
    use easy_fs::UserBuffer;
    use easy_fs::{FSManager, OpenFlags};
    use kernel_vm::{
        page_table::{MmuMeta, Pte, Sv39, VAddr, VmFlags, PPN, VPN},
        PageManager,
    };
    use spin::Mutex;
    use syscall::*;
    use xmas_elf::ElfFile;

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
    const READABLE: VmFlags<Sv39> = VmFlags::build_from_str("RV");
    const WRITEABLE: VmFlags<Sv39> = VmFlags::build_from_str("W_V");

    impl IO for SyscallContext {
        #[inline]
        fn write(&self, _caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            if let Some(ptr) = current.address_space.translate(VAddr::new(buf), READABLE) {
                if fd == 0 {
                    print!("{}", unsafe {
                        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                            ptr.as_ptr(),
                            count,
                        ))
                    });
                    count as _
                } else if fd > 1 && fd < current.fd_table.len() {
                    if let Some(file) = &current.fd_table[fd] {
                        let mut _file = file.lock();
                        if !_file.writable() {
                            return -1;
                        }
                        let mut v: Vec<&'static mut [u8]> = Vec::new();
                        unsafe {
                            let raw_buf: &'static mut [u8] =
                                core::slice::from_raw_parts_mut(ptr.as_ptr(), count);
                            v.push(raw_buf);
                        }
                        _file.write(UserBuffer::new(v)) as _
                    } else {
                        log::error!("unsupported fd: {fd}");
                        -1
                    }
                } else {
                    log::error!("unsupported fd: {fd}");
                    -1
                }
            } else {
                log::error!("ptr not readable");
                -1
            }
        }

        #[inline]
        #[allow(deprecated)]
        fn read(&self, _caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            if fd == 0 || fd >= current.fd_table.len() {
                return -1;
            }
            if let Some(ptr) = current.address_space.translate(VAddr::new(buf), WRITEABLE) {
                if fd == 1 {
                    let mut ptr = ptr.as_ptr();
                    for _ in 0..count {
                        let c = sbi_rt::legacy::console_getchar() as u8;
                        unsafe {
                            *ptr = c;
                            ptr = ptr.add(1);
                        }
                    }
                    count as _
                } else if fd != 0 && fd < current.fd_table.len() {
                    if let Some(file) = &current.fd_table[fd] {
                        let mut _file = file.lock();
                        if !_file.readable() {
                            return -1;
                        }
                        let mut v: Vec<&'static mut [u8]> = Vec::new();
                        unsafe {
                            let raw_buf: &'static mut [u8] =
                                core::slice::from_raw_parts_mut(ptr.as_ptr(), count);
                            v.push(raw_buf);
                        }
                        _file.read(UserBuffer::new(v)) as _
                    } else {
                        log::error!("unsupported fd: {fd}");
                        -1
                    }
                } else {
                    log::error!("unsupported fd: {fd}");
                    -1
                }
            } else {
                log::error!("ptr not writeable");
                -1
            }
        }

        #[inline]
        fn open(&self, _caller: Caller, path: usize, flags: usize) -> isize {
            // FS.open(, flags)
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            if let Some(ptr) = current.address_space.translate(VAddr::new(path), READABLE) {
                let mut string = String::new();
                let mut raw_ptr: *mut u8 = ptr.as_ptr();
                loop {
                    unsafe {
                        let ch = *raw_ptr;
                        if ch == 0 {
                            break;
                        }
                        string.push(ch as char);
                        raw_ptr = (raw_ptr as usize + 1) as *mut u8;
                    }
                }

                if let Some(fd) =
                    FS.open(string.as_str(), OpenFlags::from_bits(flags as u32).unwrap())
                {
                    let new_fd = current.fd_table.len();
                    current.fd_table.push(Some(Mutex::new(fd.as_ref().clone())));
                    new_fd as isize
                } else {
                    -1
                }
            } else {
                log::error!("ptr not writeable");
                -1
            }
        }

        #[inline]
        fn close(&self, _caller: Caller, fd: usize) -> isize {
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            if fd >= current.fd_table.len() || current.fd_table[fd].is_none() {
                return -1;
            }
            current.fd_table[fd].take();
            0
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: Caller, _status: usize) -> isize {
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            if let Some(parent) = unsafe { TASK_MANAGER.get_task(current.parent) } {
                let pair = parent
                    .children
                    .iter()
                    .enumerate()
                    .find(|(_, &id)| id == current.pid);
                if let Some((idx, _)) = pair {
                    parent.children.remove(idx);
                    // log::debug!("parent remove child {}", parent.children.remove(idx));
                }
                for (_, &id) in current.children.iter().enumerate() {
                    // log::warn!("parent insert child {}", id);
                    parent.children.push(id);
                }
            }
            0
        }

        fn fork(&self, _caller: Caller) -> isize {
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            let mut child_proc = current.fork().unwrap();
            let pid = child_proc.pid;
            let context = &mut child_proc.context.context;
            *context.a_mut(0) = 0 as _;
            unsafe {
                TASK_MANAGER.insert(pid, child_proc);
            }
            pid.get_val() as isize
        }

        fn exec(&self, _caller: Caller, path: usize, count: usize) -> isize {
            const READABLE: VmFlags<Sv39> = VmFlags::build_from_str("RV");
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            if let Some(ptr) = current.address_space.translate(VAddr::new(path), READABLE) {
                let name = unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr.as_ptr(), count))
                };
                current.exec(
                    ElfFile::new(read_all(FS.open(name, OpenFlags::RDONLY).unwrap()).as_slice())
                        .unwrap(),
                );
                0
            } else {
                -1
            }
        }

        // 简化的 wait 系统调用，pid == -1，则需要等待所有子进程结束，若当前进程有子进程，则返回 -1，否则返回 0
        // pid 为具体的某个值，表示需要等待某个子进程结束，因此只需要在 TASK_MANAGER 中查找是否有任务
        // 简化了进程的状态模型
        fn wait(&self, _caller: Caller, pid: isize, exit_code_ptr: usize) -> isize {
            let current = unsafe { TASK_MANAGER.current().unwrap() };
            const WRITABLE: VmFlags<Sv39> = VmFlags::build_from_str("W_V");
            if let Some(mut ptr) = current
                .address_space
                .translate(VAddr::new(exit_code_ptr), WRITABLE)
            {
                unsafe { *ptr.as_mut() = 333 as i32 };
            }
            if pid == -1 {
                if current.children.is_empty() {
                    return 0;
                } else {
                    return -1;
                }
            } else {
                if unsafe { TASK_MANAGER.get_task(TaskId::from(pid as usize)).is_none() } {
                    return pid;
                } else {
                    return -1;
                }
            }
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
        fn clock_gettime(&self, _caller: Caller, clock_id: ClockId, tp: usize) -> isize {
            const WRITABLE: VmFlags<Sv39> = VmFlags::build_from_str("W_V");
            match clock_id {
                ClockId::CLOCK_MONOTONIC => {
                    if let Some(mut ptr) = unsafe { TASK_MANAGER.current().unwrap() }
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
