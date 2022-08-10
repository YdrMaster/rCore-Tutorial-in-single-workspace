use crate::SyscallId;
use spin::Once;

pub trait Process: Sync {
    fn exit(&self, status: usize) -> isize;
}

pub trait Scheduling: Sync {
    fn sched_yield(&self) -> isize;
}

pub trait IO: Sync {
    fn write(&self, fd: usize, buf: usize, count: usize) -> isize;
}

static PROCESS: Once<&'static dyn Process> = Once::new();
static SCHEDULING: Once<&'static dyn Scheduling> = Once::new();
static IO: Once<&'static dyn IO> = Once::new();

#[inline]
pub fn init_process(process: &'static dyn Process) {
    PROCESS.call_once(|| process);
}

#[inline]
pub fn init_scheduling(scheduling: &'static dyn Scheduling) {
    SCHEDULING.call_once(|| scheduling);
}

#[inline]
pub fn init_io(io: &'static dyn IO) {
    IO.call_once(|| io);
}

pub enum SyscallResult {
    Done(isize),
    Unsupported(SyscallId),
}

impl From<isize> for SyscallResult {
    #[inline]
    fn from(val: isize) -> Self {
        Self::Done(val)
    }
}

impl From<SyscallId> for SyscallResult {
    #[inline]
    fn from(val: SyscallId) -> Self {
        Self::Unsupported(val)
    }
}

pub fn handle(id: SyscallId, args: [usize; 6]) -> SyscallResult {
    use SyscallId as Id;
    match id {
        Id::WRITE => IO
            .get()
            .map_or(id.into(), |io| io.write(args[0], args[1], args[2]).into()),
        Id::EXIT => PROCESS
            .get()
            .map_or(id.into(), |proc| proc.exit(args[0]).into()),
        Id::SCHED_YIELD => SCHEDULING
            .get()
            .map_or(id.into(), |sched| sched.sched_yield().into()),
        _ => id.into(),
    }
}
