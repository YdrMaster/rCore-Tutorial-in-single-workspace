#![no_std]
#![no_main]
#![feature(naked_functions, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod fs;
mod process;
mod processor;
mod virtio_block;

#[macro_use]
extern crate rcore_console;

#[macro_use]
extern crate alloc;

use crate::{
    fs::{read_all, FS},
    impls::{Sv39Manager, SyscallContext},
    process::Process,
    processor::ProcManager,
};
use alloc::alloc::alloc;
use core::{alloc::Layout, mem::MaybeUninit};
use easy_fs::{FSManager, OpenFlags};
use impls::Console;
use kernel_context::foreign::MultislotPortal;
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, VmMeta, PPN, VPN},
    AddressSpace,
};
use processor::PROCESSOR;
use rcore_console::log;
use rcore_task_manage::ProcId;
use riscv::register::*;
use sbi_rt::*;
use syscall::Caller;
use xmas_elf::ElfFile;

// 定义内核入口。
linker::boot0!(rust_main; stack = 32 * 4096);
// 物理内存容量 = 48 MiB。
const MEMORY: usize = 48 << 20;
// 传送门所在虚页。
const PROTAL_TRANSIT: VPN<Sv39> = VPN::MAX;
// 内核地址空间。
static mut KERNEL_SPACE: MaybeUninit<AddressSpace<Sv39, Sv39Manager>> = MaybeUninit::uninit();

extern "C" fn rust_main() -> ! {
    let layout = linker::KernelLayout::locate();
    // bss 段清零
    unsafe { layout.zero_bss() };
    // 初始化 `console`
    rcore_console::init_console(&Console);
    rcore_console::set_log_level(option_env!("LOG"));
    rcore_console::test_log();
    // 初始化内核堆
    kernel_alloc::init(layout.start() as _);
    unsafe {
        kernel_alloc::transfer(core::slice::from_raw_parts_mut(
            layout.end() as _,
            MEMORY - layout.len(),
        ))
    };
    // 建立异界传送门
    let portal_size = MultislotPortal::calculate_size(1);
    let portal_layout = Layout::from_size_align(portal_size, 1 << Sv39::PAGE_BITS).unwrap();
    let portal_ptr = unsafe { alloc(portal_layout) };
    assert!(portal_layout.size() < 1 << Sv39::PAGE_BITS);
    // 建立内核地址空间
    kernel_space(layout, MEMORY, portal_ptr as _);
    // 初始化异界传送门
    let portal = unsafe { MultislotPortal::init_transit(PROTAL_TRANSIT.base().val(), 1) };
    // 初始化 syscall
    syscall::init_io(&SyscallContext);
    syscall::init_process(&SyscallContext);
    syscall::init_scheduling(&SyscallContext);
    syscall::init_clock(&SyscallContext);
    // 加载初始进程
    let initproc = read_all(FS.open("initproc", OpenFlags::RDONLY).unwrap());
    if let Some(process) = Process::from_elf(ElfFile::new(initproc.as_slice()).unwrap()) {
        unsafe {
            PROCESSOR.set_manager(ProcManager::new());
            PROCESSOR.add(process.pid, process, ProcId::from_usize(usize::MAX));
        }
    }
    loop {
        if let Some(task) = unsafe { PROCESSOR.find_next() } {
            unsafe { task.context.execute(portal, ()) };
            match scause::read().cause() {
                scause::Trap::Exception(scause::Exception::UserEnvCall) => {
                    use syscall::{SyscallId as Id, SyscallResult as Ret};
                    let ctx = &mut task.context.context;
                    ctx.move_next();
                    let id: Id = ctx.a(7).into();
                    let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                    match syscall::handle(Caller { entity: 0, flow: 0 }, id, args) {
                        Ret::Done(ret) => match id {
                            Id::EXIT => unsafe { PROCESSOR.make_current_exited(ret) },
                            _ => {
                                let ctx = &mut task.context.context;
                                *ctx.a_mut(0) = ret as _;
                                unsafe { PROCESSOR.make_current_suspend() };
                            }
                        },
                        Ret::Unsupported(_) => {
                            log::info!("id = {id:?}");
                            unsafe { PROCESSOR.make_current_exited(-2) };
                        }
                    }
                }
                e => {
                    log::error!("unsupported trap: {e:?}");
                    unsafe { PROCESSOR.make_current_exited(-3) };
                }
            }
        } else {
            println!("no task");
            break;
        }
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

pub const MMIO: &[(usize, usize)] = &[
    (0x1000_1000, 0x00_1000), // Virtio Block in virt machine
];

fn kernel_space(layout: linker::KernelLayout, memory: usize, portal: usize) {
    let mut space = AddressSpace::new();
    for region in layout.iter() {
        log::info!("{region}");
        use linker::KernelRegionTitle::*;
        let flags = match region.title {
            Text => "X_RV",
            Rodata => "__RV",
            Data | Boot => "_WRV",
        };
        let s = VAddr::<Sv39>::new(region.range.start);
        let e = VAddr::<Sv39>::new(region.range.end);
        space.map_extern(
            s.floor()..e.ceil(),
            PPN::new(s.floor().val()),
            VmFlags::build_from_str(flags),
        )
    }
    let s = VAddr::<Sv39>::new(layout.end());
    let e = VAddr::<Sv39>::new(layout.start() + memory);
    log::info!("(heap) ---> {:#10x}..{:#10x}", s.val(), e.val());
    space.map_extern(
        s.floor()..e.ceil(),
        PPN::new(s.floor().val()),
        VmFlags::build_from_str("_WRV"),
    );
    space.map_extern(
        PROTAL_TRANSIT..PROTAL_TRANSIT + 1,
        PPN::new(portal >> Sv39::PAGE_BITS),
        VmFlags::build_from_str("__G_XWRV"),
    );
    println!();

    // MMIO
    for (base, len) in MMIO {
        let s = VAddr::<Sv39>::new(*base);
        let e = VAddr::<Sv39>::new(*base + *len);
        log::info!("MMIO range -> {:#10x}..{:#10x}", s.val(), e.val());
        space.map_extern(
            s.floor()..e.ceil(),
            PPN::new(s.floor().val()),
            VmFlags::build_from_str("_WRV"),
        );
    }

    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().val()) };
    unsafe { KERNEL_SPACE = MaybeUninit::new(space) };
}

/// 映射异界传送门。
fn map_portal(space: &AddressSpace<Sv39, Sv39Manager>) {
    let portal_idx = PROTAL_TRANSIT.index_in(Sv39::MAX_LEVEL);
    space.root()[portal_idx] = unsafe { KERNEL_SPACE.assume_init_ref() }.root()[portal_idx];
}

/// 各种接口库的实现。
mod impls {
    use crate::{
        fs::{read_all, FS},
        PROCESSOR,
    };
    use alloc::vec::Vec;
    use alloc::{alloc::alloc_zeroed, string::String};
    use core::{alloc::Layout, ptr::NonNull};
    use easy_fs::UserBuffer;
    use easy_fs::{FSManager, OpenFlags};
    use kernel_vm::{
        page_table::{MmuMeta, Pte, Sv39, VAddr, VmFlags, PPN, VPN},
        PageManager,
    };
    use rcore_console::log;
    use rcore_task_manage::ProcId;
    use spin::Mutex;
    use syscall::*;
    use xmas_elf::ElfFile;

    #[repr(transparent)]
    pub struct Sv39Manager(NonNull<Pte<Sv39>>);

    impl Sv39Manager {
        const OWNED: VmFlags<Sv39> = unsafe { VmFlags::from_raw(1 << 8) };

        #[inline]
        fn page_alloc<T>(count: usize) -> *mut T {
            unsafe {
                alloc_zeroed(Layout::from_size_align_unchecked(
                    count << Sv39::PAGE_BITS,
                    1 << Sv39::PAGE_BITS,
                ))
            }
            .cast()
        }
    }

    impl PageManager<Sv39> for Sv39Manager {
        #[inline]
        fn new_root() -> Self {
            Self(NonNull::new(Self::page_alloc(1)).unwrap())
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

        #[inline]
        fn allocate(&mut self, len: usize, flags: &mut VmFlags<Sv39>) -> NonNull<u8> {
            *flags |= Self::OWNED;
            NonNull::new(Self::page_alloc(len)).unwrap()
        }

        fn deallocate(&mut self, _pte: Pte<Sv39>, _len: usize) -> usize {
            todo!()
        }

        fn drop_root(&mut self) {
            todo!()
        }
    }

    pub struct Console;

    impl rcore_console::Console for Console {
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
        fn write(&self, _caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            if let Some(ptr) = current.address_space.translate(VAddr::new(buf), READABLE) {
                if fd == STDOUT || fd == STDDEBUG {
                    print!("{}", unsafe {
                        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                            ptr.as_ptr(),
                            count,
                        ))
                    });
                    count as _
                } else if let Some(file) = &current.fd_table[fd] {
                    let mut file = file.lock();
                    if file.writable() {
                        let mut v: Vec<&'static mut [u8]> = Vec::new();
                        unsafe { v.push(core::slice::from_raw_parts_mut(ptr.as_ptr(), count)) };
                        file.write(UserBuffer::new(v)) as _
                    } else {
                        log::error!("file not writable");
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

        fn read(&self, _caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            if let Some(ptr) = current.address_space.translate(VAddr::new(buf), WRITEABLE) {
                if fd == STDIN {
                    let mut ptr = ptr.as_ptr();
                    for _ in 0..count {
                        #[allow(deprecated)]
                        unsafe {
                            *ptr = sbi_rt::legacy::console_getchar() as u8;
                            ptr = ptr.add(1);
                        }
                    }
                    count as _
                } else if let Some(file) = &current.fd_table[fd] {
                    let mut file = file.lock();
                    if file.readable() {
                        let mut v: Vec<&'static mut [u8]> = Vec::new();
                        unsafe { v.push(core::slice::from_raw_parts_mut(ptr.as_ptr(), count)) };
                        file.read(UserBuffer::new(v)) as _
                    } else {
                        log::error!("file not readable");
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

        fn open(&self, _caller: Caller, path: usize, flags: usize) -> isize {
            // FS.open(, flags)
            let current = unsafe { PROCESSOR.current().unwrap() };
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
            let current = unsafe { PROCESSOR.current().unwrap() };
            if fd >= current.fd_table.len() || current.fd_table[fd].is_none() {
                return -1;
            }
            current.fd_table[fd].take();
            0
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: Caller, exit_code: usize) -> isize {
            exit_code as isize
        }

        fn fork(&self, _caller: Caller) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            let mut child_proc = current.fork().unwrap();
            let pid = child_proc.pid;
            let context = &mut child_proc.context.context;
            *context.a_mut(0) = 0 as _;
            unsafe {
                PROCESSOR.add(pid, child_proc, current.pid);
            }
            pid.get_usize() as isize
        }

        fn exec(&self, _caller: Caller, path: usize, count: usize) -> isize {
            const READABLE: VmFlags<Sv39> = VmFlags::build_from_str("RV");
            let current = unsafe { PROCESSOR.current().unwrap() };
            current
                .address_space
                .translate(VAddr::new(path), READABLE)
                .map(|ptr| unsafe {
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr.as_ptr(), count))
                })
                .and_then(|name| FS.open(name, OpenFlags::RDONLY))
                .map_or_else(
                    || {
                        log::error!("unknown app, select one in the list: ");
                        FS.readdir("")
                            .unwrap()
                            .into_iter()
                            .for_each(|app| println!("{app}"));
                        println!();
                        -1
                    },
                    |fd| {
                        current.exec(ElfFile::new(&read_all(fd)).unwrap());
                        0
                    },
                )
        }

        fn wait(&self, _caller: Caller, pid: isize, exit_code_ptr: usize) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            const WRITABLE: VmFlags<Sv39> = VmFlags::build_from_str("W_V");
            if let Some((dead_pid, exit_code)) =
                unsafe { PROCESSOR.wait(ProcId::from_usize(pid as usize)) }
            {
                if let Some(mut ptr) = current
                    .address_space
                    .translate(VAddr::new(exit_code_ptr), WRITABLE)
                {
                    unsafe { *ptr.as_mut() = exit_code };
                }
                return dead_pid.get_usize() as _;
            } else {
                // 等待的子进程不存在
                return -1;
            }
        }

        fn getpid(&self, _caller: Caller) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            current.pid.get_usize() as _
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
                    if let Some(mut ptr) = unsafe { PROCESSOR.current().unwrap() }
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
