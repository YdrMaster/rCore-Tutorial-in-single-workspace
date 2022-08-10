use kernel_context::Context;
use syscall::SyscallId;

/// 任务控制块。
///
/// 包含任务的上下文、状态和资源。
pub struct TaskControlBlock {
    ctx: Context,
    pub finish: bool,
    stack: [u8; 4096],
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
        ctx: Context::new(0),
        finish: false,
        stack: [0; 4096],
    };

    /// 初始化一个任务。
    pub fn init(&mut self, entry: usize) {
        self.stack.fill(0);
        self.finish = false;
        self.ctx = Context::new(entry);
        self.ctx.set_sstatus_as_user();
        *self.ctx.sp_mut() = self.stack.as_ptr() as usize + self.stack.len();
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
        match syscall::handle(id, args) {
            Ret::Done(ret) => match id {
                Id::SCHED_YIELD => Event::Yield,
                Id::EXIT => Event::Exit(self.ctx.a(0)),
                _ => {
                    *self.ctx.a_mut(0) = ret as _;
                    self.ctx.sepc += 4;
                    Event::None
                }
            },
            Ret::Unsupported(_) => Event::UnsupportedSyscall(id),
        }
    }
}
