use crate::{ClockId, SyscallId};
use spin::Once;

pub trait Process: Sync {
    fn exit(&self, status: usize) -> isize;
}

pub trait IO: Sync {
    fn write(&self, fd: usize, buf: usize, count: usize) -> isize;
}

pub trait Scheduling: Sync {
    fn sched_yield(&self) -> isize;
}

pub trait Clock: Sync {
    fn clock_gettime(&self, clock_id: ClockId, tp: usize) -> isize;
}

static PROCESS: Container<dyn Process> = Container::new();
static IO: Container<dyn IO> = Container::new();
static SCHEDULING: Container<dyn Scheduling> = Container::new();
static CLOCK: Container<dyn Clock> = Container::new();

#[inline]
pub fn init_process(process: &'static dyn Process) {
    PROCESS.init(process);
}

#[inline]
pub fn init_io(io: &'static dyn IO) {
    IO.init(io);
}

#[inline]
pub fn init_scheduling(scheduling: &'static dyn Scheduling) {
    SCHEDULING.init(scheduling);
}

#[inline]
pub fn init_clock(clock: &'static dyn Clock) {
    CLOCK.init(clock);
}

pub enum SyscallResult {
    Done(isize),
    Unsupported(SyscallId),
}

pub fn handle(id: SyscallId, args: [usize; 6]) -> SyscallResult {
    use SyscallId as Id;
    match id {
        Id::EXIT => PROCESS.call(id, |proc| proc.exit(args[0])),
        Id::WRITE => IO.call(id, |io| io.write(args[0], args[1], args[2])),
        Id::SCHED_YIELD => SCHEDULING.call(id, |sched| sched.sched_yield()),
        Id::CLOCK_GETTIME => CLOCK.call(id, |clock| clock.clock_gettime(ClockId(args[0]), args[1])),
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
