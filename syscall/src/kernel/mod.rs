#![allow(unused_variables)]

use crate::{ClockId, SyscallId};
use spin::Once;

/// 系统调用的发起者信息。
///
/// 没有办法（也没有必要？）调整发起者的描述，只好先用两个 `usize` 了。
/// 至少在一个类 Linux 的宏内核系统这是够用的。
pub struct Caller {
    /// 发起者拥有的资源集的标记，相当于进程号。
    pub entity: usize,
    /// 发起者的控制流的标记，相当于线程号。
    pub flow: usize,
}

pub trait Process: Sync {
    fn exit(&self, caller: Caller, status: usize) -> isize {
        unimplemented!()
    }
    fn fork(&self, caller: Caller) -> isize {
        unimplemented!()
    }
    fn exec(&self, caller: Caller, path: usize, count: usize) -> isize {
        unimplemented!()
    }
    fn wait(&self, caller: Caller, pid: isize, exit_code_ptr: usize) -> isize {
        unimplemented!()
    }
    fn getpid(&self, caller: Caller) -> isize {
        unimplemented!()
    }
}

pub trait IO: Sync {
    fn read(&self, caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
        unimplemented!()
    }
    fn write(&self, caller: Caller, fd: usize, buf: usize, count: usize) -> isize {
        unimplemented!()
    }
    fn open(&self, caller: Caller, path: usize, flags: usize) -> isize {
        unimplemented!()
    }
    fn close(&self, caller: Caller, fd: usize) -> isize {
        unimplemented!()
    }
}

pub trait Memory: Sync {
    fn mmap(
        &self,
        caller: Caller,
        addr: usize,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: usize,
    ) -> isize {
        unimplemented!()
    }

    fn munmap(&self, caller: Caller, addr: usize, length: usize) -> isize {
        unimplemented!()
    }
}

pub trait Scheduling: Sync {
    fn sched_yield(&self, caller: Caller) -> isize {
        unimplemented!()
    }
}

pub trait Clock: Sync {
    fn clock_gettime(&self, caller: Caller, clock_id: ClockId, tp: usize) -> isize {
        unimplemented!()
    }
}

pub trait Signal: Sync {
    fn kill(&self, caller: Caller, pid: isize, signum: u8) -> isize {
        unimplemented!()
    }

    fn sigaction(&self, caller: Caller, signum: u8, action: usize, old_action: usize) -> isize {
        unimplemented!()
    }

    fn sigprocmask(&self, caller: Caller, mask: usize) -> isize {
        unimplemented!()
    }

    fn sigreturn(&self, caller: Caller) -> isize {
        unimplemented!()
    }
}

pub trait Thread: Sync {
    fn thread_create(&self, caller: Caller, entry: usize, arg: usize) -> isize {
        unimplemented!()
    }
    fn waittid(&self, caller: Caller, tid: usize) -> isize {
        unimplemented!()
    }
    fn gettid(&self, caller: Caller) -> isize {
        unimplemented!()
    }
}

pub trait SyncMutex: Sync {
    fn semaphore_create(&self, caller: Caller, res_count: usize) -> isize {
        unimplemented!()
    }
    fn semaphore_up(&self, caller: Caller, sem_id: usize) -> isize {
        unimplemented!()
    }
    fn semaphore_down(&self, caller: Caller, sem_id: usize) -> isize {
        unimplemented!()
    }
    fn mutex_create(&self, caller: Caller, blocking: bool) -> isize {
        unimplemented!()
    }
    fn mutex_lock(&self, caller: Caller, mutex_id: usize) -> isize {
        unimplemented!()
    }
    fn mutex_unlock(&self, caller: Caller, mutex_id: usize) -> isize {
        unimplemented!()
    }
    fn condvar_create(&self, caller: Caller, arg: usize) -> isize {
        unimplemented!()
    }
    fn condvar_signal(&self, caller: Caller, condvar_id: usize) -> isize {
        unimplemented!()
    }
    fn condvar_wait(&self, caller: Caller, condvar_id: usize, mutex_id: usize) -> isize {
        unimplemented!()
    }
}

static PROCESS: Container<dyn Process> = Container::new();
static IO: Container<dyn IO> = Container::new();
static MEMORY: Container<dyn Memory> = Container::new();
static SCHEDULING: Container<dyn Scheduling> = Container::new();
static CLOCK: Container<dyn Clock> = Container::new();
static SIGNAL: Container<dyn Signal> = Container::new();
static THREAD: Container<dyn Thread> = Container::new();
static SYNC_MUTEX: Container<dyn SyncMutex> = Container::new();

#[inline]
pub fn init_process(process: &'static dyn Process) {
    PROCESS.init(process);
}

#[inline]
pub fn init_io(io: &'static dyn IO) {
    IO.init(io);
}

#[inline]
pub fn init_memory(memory: &'static dyn Memory) {
    MEMORY.init(memory);
}

#[inline]
pub fn init_scheduling(scheduling: &'static dyn Scheduling) {
    SCHEDULING.init(scheduling);
}

#[inline]
pub fn init_clock(clock: &'static dyn Clock) {
    CLOCK.init(clock);
}

#[inline]
pub fn init_signal(signal: &'static dyn Signal) {
    SIGNAL.init(signal);
}

#[inline]
pub fn init_thread(thread: &'static dyn Thread) {
    THREAD.init(thread);
}

#[inline]
pub fn init_sync_mutex(sync_mutex: &'static dyn SyncMutex) {
    SYNC_MUTEX.init(sync_mutex);
}

pub enum SyscallResult {
    Done(isize),
    Unsupported(SyscallId),
}

pub fn handle(caller: Caller, id: SyscallId, args: [usize; 6]) -> SyscallResult {
    use SyscallId as Id;
    match id {
        Id::WRITE => IO.call(id, |io| io.write(caller, args[0], args[1], args[2])),
        Id::READ => IO.call(id, |io| io.read(caller, args[0], args[1], args[2])),
        Id::OPENAT => IO.call(id, |io| io.open(caller, args[0], args[1])),
        Id::CLOSE => IO.call(id, |io| io.close(caller, args[0])),
        Id::EXIT => PROCESS.call(id, |proc| proc.exit(caller, args[0])),
        Id::CLONE => PROCESS.call(id, |proc| proc.fork(caller)),
        Id::EXECVE => PROCESS.call(id, |proc| proc.exec(caller, args[0], args[1])),
        Id::WAIT4 => PROCESS.call(id, |proc| proc.wait(caller, args[0] as _, args[1])),
        Id::GETPID => PROCESS.call(id, |proc| proc.getpid(caller)),
        Id::CLOCK_GETTIME => CLOCK.call(id, |clock| {
            clock.clock_gettime(caller, ClockId(args[0]), args[1])
        }),
        Id::SCHED_YIELD => SCHEDULING.call(id, |sched| sched.sched_yield(caller)),
        Id::MUNMAP => MEMORY.call(id, |memory| memory.munmap(caller, args[0], args[1])),
        Id::MMAP => MEMORY.call(id, |memory| {
            let [addr, length, prot, flags, fd, offset] = args;
            memory.mmap(caller, addr, length, prot as _, flags as _, fd as _, offset)
        }),
        Id::KILL => SIGNAL.call(id, |signal| signal.kill(caller, args[0] as _, args[1] as _)),
        Id::RT_SIGACTION => SIGNAL.call(id, |signal| {
            signal.sigaction(caller, args[0] as _, args[1], args[2])
        }),
        Id::RT_SIGPROCMASK => SIGNAL.call(id, |signal| signal.sigprocmask(caller, args[0])),
        Id::RT_SIGRETURN => SIGNAL.call(id, |signal| signal.sigreturn(caller)),
        Id::WAITID => THREAD.call(id, |thread| thread.waittid(caller, args[0])),
        Id::GETTID => THREAD.call(id, |thread| thread.gettid(caller)),
        Id::THREAD_CREATE => THREAD.call(id, |thread| thread.thread_create(caller, args[0], args[1])),
        Id::SEMAPHORE_CREATE => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.semaphore_create(caller, args[0])),
        Id::SEMAPHORE_UP => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.semaphore_up(caller, args[0])),
        Id::SEMAPHORE_DOWN => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.semaphore_down(caller, args[0])),
        Id::MUTEX_CREATE => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.mutex_create(caller, args[0] != 0)),
        Id::MUTEX_LOCK => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.mutex_lock(caller, args[0])),
        Id::MUTEX_UNLOCK => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.mutex_unlock(caller, args[0])),
        Id::CONDVAR_CREATE => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.condvar_create(caller, args[0])),
        Id::CONDVAR_SIGNAL => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.condvar_signal(caller, args[0])),
        Id::CONDVAR_WAIT => SYNC_MUTEX.call(id, |sync_mutex| sync_mutex.condvar_wait(caller, args[0], args[1])),
        _ => SyscallResult::Unsupported(id),
    }
}

struct Container<T: 'static + ?Sized>(spin::Once<&'static T>);

impl<T: 'static + ?Sized> Container<T> {
    #[inline]
    const fn new() -> Self {
        Self(Once::new())
    }

    #[inline]
    fn init(&self, val: &'static T) {
        self.0.call_once(|| val);
    }

    #[inline]
    fn call(&self, id: SyscallId, f: impl FnOnce(&T) -> isize) -> SyscallResult {
        self.0
            .get()
            .map_or(SyscallResult::Unsupported(id), |clock| {
                SyscallResult::Done(f(clock))
            })
    }
}
