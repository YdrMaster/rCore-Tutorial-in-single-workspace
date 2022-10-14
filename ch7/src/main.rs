#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod exit_process;
mod fs;
mod process;
mod processor;
mod virtio_block;

#[macro_use]
extern crate console;

#[macro_use]
extern crate alloc;

use crate::{
    fs::{read_all, FS},
    impls::{Sv39Manager, SyscallContext},
    process::Process,
};
use console::log;
use easy_fs::{FSManager, OpenFlags};
use exit_process::exit_process;
use impls::Console;
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN},
    AddressSpace,
};
use processor::init_processor;
pub use processor::PROCESSOR;
use riscv::register::*;
use sbi_rt::*;
use signal::SignalResult;
use spin::Once;
use syscall::Caller;
use xmas_elf::ElfFile;

// 定义内核入口。
linker::boot0!(rust_main; stack = 16 * 4096);
// 物理内存容量 = 16 MiB。
const MEMORY: usize = 16 << 20;
// 内核地址空间。
static mut KERNEL_SPACE: Once<AddressSpace<Sv39, Sv39Manager>> = Once::new();

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
    syscall::init_signal(&SyscallContext);
    // 初始化内核堆
    kernel_alloc::init(layout.start() as _);
    unsafe {
        kernel_alloc::transfer(core::slice::from_raw_parts_mut(
            layout.end() as _,
            MEMORY - layout.len(),
        ))
    };
    // 建立内核地址空间
    unsafe { KERNEL_SPACE.call_once(|| kernel_space(layout, MEMORY)) };
    // 异界传送门
    // 可以直接放在栈上
    init_processor();
    let tramp = (
        PPN::<Sv39>::new(unsafe { &PROCESSOR.portal } as *const _ as usize >> Sv39::PAGE_BITS),
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
            unsafe { PROCESSOR.add(process.pid, process) };
        }
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
                    let syscall_ret = syscall::handle(Caller { entity: 0, flow: 0 }, id, args);
                    // 目前信号处理位置放在 syscall 执行之后，这只是临时的实现。
                    // 正确处理信号的位置应该是在 “trap 中处理异常和中断和异常之后，返回用户态之前”。
                    // 例如发现有访存异常时，应该触发 SIGSEGV 信号然后进行处理。
                    // 但目前 syscall 之后直接切换用户程序，没有 “返回用户态” 这一步，甚至 trap 本身也没了。
                    //
                    // 最简单粗暴的方法是，在 `scause::Trap` 分类的每一条分支之后都加上信号处理，
                    // 当然这样可能代码上不够优雅。处理信号的具体时机还需要后续再讨论。
                    match task.signal.handle_signals(ctx) {
                        // 进程应该结束执行
                        SignalResult::ProcessKilled(_exit_code) => {
                            exit_process();
                            unsafe { PROCESSOR.make_current_exited() }
                        }
                        _ => match syscall_ret {
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
                        },
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

pub const MMIO: &[(usize, usize)] = &[
    (0x1000_1000, 0x00_1000), // Virtio Block in virt machine
];

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

    unsafe { satp::set(satp::Mode::Sv39, 0, space.root_ppn().val()) };
    space
}

/// 各种接口库的实现。
mod impls {
    use crate::{
        exit_process,
        fs::{read_all, FS},
        process::TaskId,
        PROCESSOR,
    };
    use alloc::{alloc::alloc_zeroed, string::String, vec::Vec};
    use console::log;
    use core::{alloc::Layout, ptr::NonNull};
    use easy_fs::UserBuffer;
    use easy_fs::{FSManager, OpenFlags};
    use kernel_vm::{
        page_table::{MmuMeta, Pte, Sv39, VAddr, VmFlags, PPN, VPN},
        PageManager,
    };
    use signal::SignalNo;
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
        fn exit(&self, _caller: Caller, _status: usize) -> isize {
            exit_process()
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

        fn getpid(&self, _caller: Caller) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            current.pid.get_val() as _
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

    impl Signal for SyscallContext {
        fn kill(&self, _caller: Caller, pid: isize, signum: u8) -> isize {
            if let Some(target_task) = unsafe { PROCESSOR.get_task(TaskId::from(pid as usize)) } {
                if let Ok(signal_no) = SignalNo::try_from(signum) {
                    if signal_no != SignalNo::ERR {
                        target_task.signal.add_signal(signal_no);
                        return 0;
                    }
                }
            }
            -1
        }

        fn sigaction(
            &self,
            _caller: Caller,
            signum: u8,
            action: usize,
            old_action: usize,
        ) -> isize {
            if signum as usize > signal::MAX_SIG {
                return -1;
            }
            let current = unsafe { PROCESSOR.current().unwrap() };
            if let Ok(signal_no) = SignalNo::try_from(signum) {
                if signal_no == SignalNo::ERR {
                    return -1;
                }
                // 如果需要返回原来的处理函数，则从信号模块中获取
                if old_action as usize != 0 {
                    if let Some(mut ptr) = current
                        .address_space
                        .translate(VAddr::new(old_action), WRITEABLE)
                    {
                        if let Some(signal_action) = current.signal.get_action_ref(signal_no) {
                            *unsafe { ptr.as_mut() } = signal_action;
                        } else {
                            return -1;
                        }
                    } else {
                        // 如果返回了 None，说明 signal_no 无效
                        return -1;
                    }
                }
                // 如果需要设置新的处理函数，则设置到信号模块中
                if action as usize != 0 {
                    if let Some(ptr) = current
                        .address_space
                        .translate(VAddr::new(action), READABLE)
                    {
                        // 如果返回了 false，说明 signal_no 无效
                        if !current
                            .signal
                            .set_action(signal_no, &unsafe { *ptr.as_ptr() })
                        {
                            return -1;
                        }
                    } else {
                        return -1;
                    }
                }
                return 0;
            }
            -1
        }

        fn sigprocmask(&self, _caller: Caller, mask: usize) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            current.signal.update_mask(mask) as isize
        }

        fn sigreturn(&self, _caller: Caller) -> isize {
            let current = unsafe { PROCESSOR.current().unwrap() };
            // 如成功，则需要修改当前用户程序的 LocalContext
            if current.signal.sig_return(&mut current.context.context) {
                0
            } else {
                -1
            }
        }
    }
}
