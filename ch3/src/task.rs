use kernel_context::LocalContext;
use syscall::{Caller, SyscallId};

/// 任务控制块。
///
/// 包含任务的上下文、状态和资源。
pub struct TaskControlBlock {
    ctx: LocalContext,
    pub finish: bool,
    stack: [usize; 256],
}

/// 调度事件。
pub enum SchedulingEvent {
    None,
    Yield,
    Exit(usize),
    UnsupportedSyscall(SyscallId),
}

impl TaskControlBlock {
    pub const ZERO: Self = Self {
        ctx: LocalContext::empty(),
        finish: false,
        stack: [0; 256],
    };

    /// 初始化一个任务。
    pub fn init(&mut self, entry: usize) {
        self.stack.fill(0);
        self.finish = false;
        self.ctx = LocalContext::user(entry);
        *self.ctx.sp_mut() = self.stack.as_ptr() as usize + core::mem::size_of_val(&self.stack);
    }

    /// 执行此任务。
    #[inline]
    pub unsafe fn execute(&mut self) {
        self.ctx.execute();
    }

    /// 处理系统调用，返回是否应该终止程序。
    pub fn handle_syscall(&mut self) -> SchedulingEvent {
        use syscall::{SyscallId as Id, SyscallResult as Ret};
        use SchedulingEvent as Event;

        let id = self.ctx.a(7).into();
        let args = [
            self.ctx.a(0),
            self.ctx.a(1),
            self.ctx.a(2),
            self.ctx.a(3),
            self.ctx.a(4),
            self.ctx.a(5),
        ];
        match syscall::handle(Caller { entity: 0, flow: 0 }, id, args) {
            Ret::Done(ret) => match id {
                Id::EXIT => Event::Exit(self.ctx.a(0)),
                Id::SCHED_YIELD => {
                    *self.ctx.a_mut(0) = ret as _;
                    self.ctx.move_next();
                    Event::Yield
                }
                _ => {
                    *self.ctx.a_mut(0) = ret as _;
                    self.ctx.move_next();
                    Event::None
                }
            },
            Ret::Unsupported(_) => Event::UnsupportedSyscall(id),
        }
    }
}
