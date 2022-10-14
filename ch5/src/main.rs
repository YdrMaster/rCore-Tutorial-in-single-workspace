#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod process;
mod processor;

#[macro_use]
extern crate console;

extern crate alloc;

use alloc::collections::BTreeMap;
use console::log;
use core::ffi::CStr;
use impls::{Console, Sv39Manager, SyscallContext};
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN},
    AddressSpace,
};
use process::Process;
use processor::{init_processor, PROCESSOR};
use riscv::register::*;
use sbi_rt::*;
use spin::Lazy;
use syscall::Caller;
use xmas_elf::ElfFile;

// 应用程序内联进来。
core::arch::global_asm!(include_str!(env!("APP_ASM")));
// 定义内核入口。
linker::boot0!(rust_main; stack = 16 * 4096);
// 物理内存容量 = 16 MiB。
const MEMORY: usize = 16 << 20;
/// 加载用户进程。
static APPS: Lazy<BTreeMap<&'static str, &'static [u8]>> = Lazy::new(|| {
    extern "C" {
        static app_names: u8;
    }
    unsafe {
        linker::AppMeta::locate()
            .iter()
            .scan(&app_names as *const _ as usize, |addr, data| {
                let name = CStr::from_ptr(*addr as _).to_str().unwrap();
                *addr += name.as_bytes().len() + 1;
                Some((name, data))
            })
    }
    .collect()
});

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
    kernel_alloc::init(layout.start() as _);
    unsafe {
        kernel_alloc::transfer(core::slice::from_raw_parts_mut(
            layout.end() as _,
            MEMORY - layout.len(),
        ))
    };
    // 建立内核地址空间
    let mut ks = kernel_space(layout, MEMORY);
    let tramp = (
        PPN::<Sv39>::new(unsafe { &PROCESSOR.portal } as *const _ as usize >> Sv39::PAGE_BITS),
        VmFlags::build_from_str("XWRV"),
    );
    // 传送门映射到所有地址空间
    init_processor();
    ks.map_portal(tramp);

    println!("/**** APPS ****");
    APPS.keys().for_each(|app| println!("{app}"));
    println!("**************/");
    // 加载应用程序
    let initproc_data = APPS.get("initproc").unwrap();
    if let Some(mut process) = Process::from_elf(ElfFile::new(initproc_data).unwrap()) {
        process.address_space.map_portal(tramp);
        unsafe { PROCESSOR.add(process.pid, process) };
    }
    const PROTAL_TRANSIT: usize = VPN::<Sv39>::MAX.base().val();
    loop {
        if let Some(task) = unsafe { PROCESSOR.find_next() } {
            task.execute(unsafe { &mut PROCESSOR.portal }, PROTAL_TRANSIT);
            match scause::read().cause() {
                scause::Trap::Exception(scause::Exception::UserEnvCall) => {
                    use syscall::{SyscallId as Id, SyscallResult as Ret};
                    let ctx = &mut task.context.context;
                    ctx.move_next();
                    let id: Id = ctx.a(7).into();
                    let args = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                    match syscall::handle(Caller { entity: 0, flow: 0 }, id, args) {
                        Ret::Done(ret) => match id {
                            Id::EXIT => unsafe { PROCESSOR.make_current_exited() },
                            _ => {
                                let ctx = &mut task.context.context;
                                *ctx.a_mut(0) = ret as _;
                                unsafe { PROCESSOR.make_current_suspend() };
                            }
                        },
                        Ret::Unsupported(_) => {
                            log::info!("id = {id:?}");
                            unsafe { PROCESSOR.make_current_exited() };
                        }
                    }
                }
                e => {
                    log::error!("unsupported trap: {e:?}");
                    unsafe { PROCESSOR.make_current_exited() };
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

fn kernel_space(layout: linker::KernelLayout, memory: usize) -> AddressSpace<Sv39, Sv39Manager> {
    let mut space = AddressSpace::<Sv39, Sv39Manager>::new();
    for region in layout.iter() {
        log::info!("{region}");
        use linker::KernelRegionTitle::*;
        let flags = match region.title {
            Text => "X_RV",
            Rodata => "__RV",
            Data => "_WRV",
            Boot => "_WRV",
        };
        let s = VAddr::<Sv39>::new(region.range.start);
        let e = VAddr::<Sv39>::new(region.range.end);
        space.map_extern(
            s.floor()..e.ceil(),
            PPN::new(s.floor().val()),
            VmFlags::build_from_str(flags),
        )
    }
    log::info!(
        "(heap) ---> {:#10x}..{:#10x}",
        layout.end(),
        layout.start() + memory
    );
    let s = VAddr::<Sv39>::new(layout.end());
    let e = VAddr::<Sv39>::new(layout.start() + memory);
    space.map_extern(
        s.floor()..e.ceil(),
        PPN::new(s.floor().val()),
        VmFlags::build_from_str("_WRV"),
    );
    println!();
    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().val()) };
    space
}

/// 各种接口库的实现。
mod impls {
    use crate::{process::TaskId, APPS, PROCESSOR};
    use alloc::alloc::alloc_zeroed;
    use console::log;
    use core::{alloc::Layout, ptr::NonNull};
    use kernel_vm::{
        page_table::{MmuMeta, Pte, Sv39, VAddr, VmFlags, PPN, VPN},
        PageManager,
    };
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

    impl console::Console for Console {
        #[inline]
        fn put_char(&self, c: u8) {
            #[allow(deprecated)]
            sbi_rt::legacy::console_putchar(c as _);
        }
    }

    pub struct SyscallContext;

    impl IO for SyscallContext {
        fn write(&self, _caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            match fd {
                STDOUT | STDDEBUG => {
                    const READABLE: VmFlags<Sv39> = VmFlags::build_from_str("RV");
                    if let Some(ptr) = unsafe { PROCESSOR.current() }
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

        #[inline]
        fn read(&self, _caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
            if fd == STDIN {
                const WRITEABLE: VmFlags<Sv39> = VmFlags::build_from_str("W_V");
                if let Some(mut ptr) = unsafe { PROCESSOR.current().unwrap() }
                    .address_space
                    .translate(VAddr::new(buf), WRITEABLE)
                {
                    let mut ptr = unsafe { ptr.as_mut() } as *mut u8;
                    for _ in 0..count {
                        #[allow(deprecated)]
                        let c = sbi_rt::legacy::console_getchar() as u8;
                        unsafe {
                            *ptr = c;
                            ptr = ptr.add(1);
                        }
                    }
                    count as _
                } else {
                    log::error!("ptr not writeable");
                    -1
                }
            } else {
                log::error!("unsupported fd: {fd}");
                -1
            }
        }
    }

    impl Process for SyscallContext {
        #[inline]
        fn exit(&self, _caller: Caller, _status: usize) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            if let Some(parent) = unsafe { PROCESSOR.get_task(current.parent) } {
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
            let current = unsafe { PROCESSOR.current().unwrap() };
            let mut child_proc = current.fork().unwrap();
            let pid = child_proc.pid;
            let context = &mut child_proc.context.context;
            *context.a_mut(0) = 0 as _;
            unsafe {
                PROCESSOR.add(pid, child_proc);
            }
            pid.get_val() as isize
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
                .and_then(|name| APPS.get(name))
                .and_then(|input| ElfFile::new(input).ok())
                .map_or(-1, |data| {
                    current.exec(data);
                    0
                })
        }

        // 简化的 wait 系统调用，pid == -1，则需要等待所有子进程结束，若当前进程有子进程，则返回 -1，否则返回 0
        // pid 为具体的某个值，表示需要等待某个子进程结束，因此只需要在 TASKMANAGER 中查找是否有任务
        // 简化了进程的状态模型
        fn wait(&self, _caller: Caller, pid: isize, exit_code_ptr: usize) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
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
                if unsafe { PROCESSOR.get_task(TaskId::from(pid as usize)).is_none() } {
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
