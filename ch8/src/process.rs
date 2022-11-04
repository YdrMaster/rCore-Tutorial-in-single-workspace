use crate::{map_portal, Sv39Manager, PROCESSOR};
use alloc::sync::Arc;
use alloc::{alloc::alloc_zeroed, boxed::Box, vec::Vec};
use core::{alloc::Layout, str::FromStr};
use easy_fs::FileHandle;
use kernel_context::{foreign::ForeignContext, LocalContext};
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN},
    AddressSpace,
};
use rcore_task_manage::{ProcId, ThreadId};
use signal::Signal;
use signal_impl::SignalImpl;
use spin::Mutex;
use sync::{Condvar, Mutex as MutexTrait, Semaphore};
use xmas_elf::{
    header::{self, HeaderPt2, Machine},
    program, ElfFile,
};

/// 线程
pub struct Thread {
    /// 不可变
    pub tid: ThreadId,
    /// 可变
    pub context: ForeignContext,
}

impl Thread {
    pub fn new(satp: usize, context: LocalContext) -> Self {
        Self {
            tid: ThreadId::new(),
            context: ForeignContext { context, satp },
        }
    }
}

/// 进程。
pub struct Process {
    /// 不可变
    pub pid: ProcId,
    /// 可变
    pub address_space: AddressSpace<Sv39, Sv39Manager>,
    /// 文件描述符表
    pub fd_table: Vec<Option<Mutex<FileHandle>>>,
    /// 信号模块
    pub signal: Box<dyn Signal>,
    /// 分配的锁以及信号量
    pub semaphore_list: Vec<Option<Arc<Semaphore>>>,
    pub mutex_list: Vec<Option<Arc<dyn MutexTrait>>>,
    pub condvar_list: Vec<Option<Arc<Condvar>>>,
}

impl Process {
    /// 只支持一个线程
    pub fn exec(&mut self, elf: ElfFile) {
        let (proc, thread) = Process::from_elf(elf).unwrap();
        self.address_space = proc.address_space;
        unsafe {
            let pthreads = PROCESSOR.get_thread(self.pid).unwrap();
            PROCESSOR.get_task(pthreads[0]).unwrap().context = thread.context;
        }
    }
    /// 只支持一个线程
    pub fn fork(&mut self) -> Option<(Self, Thread)> {
        // 子进程 pid
        let pid = ProcId::new();
        // 复制父进程地址空间
        let parent_addr_space = &self.address_space;
        let mut address_space: AddressSpace<Sv39, Sv39Manager> = AddressSpace::new();
        parent_addr_space.cloneself(&mut address_space);
        map_portal(&address_space);
        // 线程
        let pthreads = unsafe { PROCESSOR.get_thread(self.pid).unwrap() };
        let context = unsafe {
            PROCESSOR
                .get_task(pthreads[0])
                .unwrap()
                .context
                .context
                .clone()
        };
        let satp = (8 << 60) | address_space.root_ppn().val();
        let thread = Thread::new(satp, context);
        // 复制父进程文件符描述表
        let mut new_fd_table: Vec<Option<Mutex<FileHandle>>> = Vec::new();
        for fd in self.fd_table.iter_mut() {
            if let Some(file) = fd {
                new_fd_table.push(Some(Mutex::new(file.get_mut().clone())));
            } else {
                new_fd_table.push(None);
            }
        }
        Some((
            Self {
                pid,
                address_space,
                fd_table: new_fd_table,
                signal: self.signal.from_fork(),
                semaphore_list: Vec::new(),
                mutex_list: Vec::new(),
                condvar_list: Vec::new(),
            },
            thread,
        ))
    }

    pub fn from_elf(elf: ElfFile) -> Option<(Self, Thread)> {
        let entry = match elf.header.pt2 {
            HeaderPt2::Header64(pt2)
                if pt2.type_.as_type() == header::Type::Executable
                    && pt2.machine.as_machine() == Machine::RISC_V =>
            {
                pt2.entry_point as usize
            }
            _ => None?,
        };

        const PAGE_SIZE: usize = 1 << Sv39::PAGE_BITS;
        const PAGE_MASK: usize = PAGE_SIZE - 1;

        let mut address_space = AddressSpace::new();
        for program in elf.program_iter() {
            if !matches!(program.get_type(), Ok(program::Type::Load)) {
                continue;
            }

            let off_file = program.offset() as usize;
            let len_file = program.file_size() as usize;
            let off_mem = program.virtual_addr() as usize;
            let end_mem = off_mem + program.mem_size() as usize;
            assert_eq!(off_file & PAGE_MASK, off_mem & PAGE_MASK);

            let mut flags: [u8; 5] = *b"U___V";
            if program.flags().is_execute() {
                flags[1] = b'X';
            }
            if program.flags().is_write() {
                flags[2] = b'W';
            }
            if program.flags().is_read() {
                flags[3] = b'R';
            }
            address_space.map(
                VAddr::new(off_mem).floor()..VAddr::new(end_mem).ceil(),
                &elf.input[off_file..][..len_file],
                off_mem & PAGE_MASK,
                VmFlags::from_str(unsafe { core::str::from_utf8_unchecked(&flags) }).unwrap(),
            );
        }
        // 映射用户栈
        let stack = unsafe {
            alloc_zeroed(Layout::from_size_align_unchecked(
                2 << Sv39::PAGE_BITS,
                1 << Sv39::PAGE_BITS,
            ))
        };
        address_space.map_extern(
            VPN::new((1 << 26) - 2)..VPN::new(1 << 26),
            PPN::new(stack as usize >> Sv39::PAGE_BITS),
            VmFlags::build_from_str("U_WRV"),
        );
        // 映射异界传送门
        map_portal(&address_space);
        let satp = (8 << 60) | address_space.root_ppn().val();
        let mut context = LocalContext::user(entry);
        *context.sp_mut() = 1 << 38;
        let thread = Thread::new(satp, context);

        Some((
            Self {
                pid: ProcId::new(),
                address_space,
                fd_table: vec![
                    // Stdin
                    Some(Mutex::new(FileHandle::empty(true, false))),
                    // Stdout
                    Some(Mutex::new(FileHandle::empty(false, true))),
                ],
                signal: Box::new(SignalImpl::new()),
                semaphore_list: Vec::new(),
                mutex_list: Vec::new(),
                condvar_list: Vec::new(),
            },
            thread,
        ))
    }
}
