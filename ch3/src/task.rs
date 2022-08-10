﻿use kernel_context::Context;

pub struct TaskControlBlock {
    ctx: Context,
    stack: [u8; 4096],
    finish: bool,
}

pub enum SchedulingEvent {
    None,
    Exit(usize),
    Yield,
}

impl TaskControlBlock {
    pub const UNINIT: Self = Self {
        ctx: Context::new(0),
        stack: [0; 4096],
        finish: false,
    };

    pub fn init(&mut self, entry: usize) {
        self.stack.fill(0);
        self.finish = false;
        self.ctx = Context::new(entry);
        self.ctx.set_sstatus_as_user();
        *self.ctx.sp_mut() = self.stack.as_ptr() as usize + self.stack.len();
    }

    #[inline]
    pub unsafe fn execute(&mut self) {
        self.ctx.execute();
    }

    /// 处理系统调用，返回是否应该终止程序。
    pub fn handle_syscall(&mut self) -> SchedulingEvent {
        use syscall::SyscallId as Id;
        use SchedulingEvent as Event;

        let id = self.ctx.a(7).into();
        let ret = syscall::handle(
            id,
            [
                self.ctx.a(0),
                self.ctx.a(1),
                self.ctx.a(2),
                self.ctx.a(3),
                self.ctx.a(4),
                self.ctx.a(5),
            ],
        );
        match id {
            Id::EXIT => Event::Exit(self.ctx.a(0)),
            Id::SCHED_YIELD => Event::Yield,
            _ => {
                *self.ctx.a_mut(0) = ret as _;
                self.ctx.sepc += 4;
                Event::None
            }
        }
    }
}