
use alloc::{vec::Vec};

/// Schedule trait，根据进程的 id 进行调度
/// add 添加 id
/// fetch 取出 id
pub trait Schedule<I: Copy + Ord> {
    /// 添加 id 进入调度队列
    fn add_back(&mut self, id: I);

    /// 从调度队列中取出 id
    fn fetch(&mut self) -> Option<I>;

    /// 添加 id 到调度队列首部
    fn add_front(&mut self, id: I);
}


pub struct FifoScheduler<I: Copy + Ord> {
    task_queue: Vec<I>,
}

impl<I: Copy + Ord> FifoScheduler<I> {
    pub const fn new() -> Self {
        Self {
            task_queue: Vec::new(),
        }
    }

    
}

impl<I: Copy + Ord> Schedule<I> for FifoScheduler<I> {
    fn add_back(&mut self, id: I) {
        self.task_queue.push(id);
    }

    fn fetch(&mut self) -> Option<I> {
        if !self.task_queue.is_empty() {
            Some(self.task_queue.remove(0))
        } else {
            None
        }
    }

    fn add_front(&mut self, id: I) {
        self.task_queue.insert(0, id);
    }
}