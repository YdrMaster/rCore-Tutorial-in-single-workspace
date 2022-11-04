use super::UPIntrFreeCell;
use alloc::collections::VecDeque;
use rcore_task_manage::ThreadId;

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// tid 表示的线程试图获取锁，并返回结果
    fn lock(&self, tid: ThreadId) -> bool;
    /// 当前线程释放锁，并唤醒某个阻塞在这个锁上的线程
    fn unlock(&self) -> Option<ThreadId>;
}

/// MutexBlocking
pub struct MutexBlocking {
    inner: UPIntrFreeCell<MutexBlockingInner>,
}

/// MutexBlockingInner
pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<ThreadId>,
}

impl MutexBlocking {
    /// new
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPIntrFreeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    // 获取锁，如果获取成功，返回 true，否则会返回 false，要求阻塞对应的线程
    fn lock(&self, tid: ThreadId) -> bool {
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(tid);
            drop(mutex_inner);
            false
        } else {
            mutex_inner.locked = true;
            true
        }
    }
    // 释放锁，释放之后会唤醒一个被阻塞的进程，要求重新进入调度队列
    fn unlock(&self) -> Option<ThreadId> {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            Some(waking_task)
        } else {
            mutex_inner.locked = false;
            None
        }
    }
}
