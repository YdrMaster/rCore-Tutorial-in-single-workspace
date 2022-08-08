use crate::SyscallId;
use spin::Once;

pub trait Process: Sync {
    fn exit(&self, status: usize) -> isize;
}

pub trait IO: Sync {
    fn write(&self, fd: usize, buf: usize, count: usize) -> isize;
}

static PROCESS: Once<&'static dyn Process> = Once::new();
static IO: Once<&'static dyn IO> = Once::new();

#[inline]
pub fn init_process(process: &'static dyn Process) {
    PROCESS.call_once(|| process);
}

#[inline]
pub fn init_io(io: &'static dyn IO) {
    IO.call_once(|| io);
}

pub fn handle(id: SyscallId, args: [usize; 6]) -> isize {
    match id {
        SyscallId::WRITE => IO.get().unwrap().write(args[0], args[1], args[2]),
        SyscallId::EXIT => PROCESS.get().unwrap().exit(args[0]),
        _ => unimplemented!(),
    }
}
