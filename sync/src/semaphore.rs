use super::UPIntrFreeCell;
use alloc::collections::VecDeque;
use rcore_task_manage::ThreadId;

/// Semaphore
pub struct Semaphore {
    /// UPIntrFreeCell<SemaphoreInner>
    pub inner: UPIntrFreeCell<SemaphoreInner>,
}

/// SemaphoreInner
pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<ThreadId>,
}

impl Semaphore {
    /// new
    pub fn new(res_count: usize) -> Self {
        Self {
            inner: unsafe {
                UPIntrFreeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
    /// 当前线程释放信号量表示的一个资源，并唤醒一个阻塞的线程
    pub fn up(&self) -> Option<ThreadId> {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        inner.wait_queue.pop_front()
    }
    /// 当前线程试图获取信号量表示的资源，并返回结果
    pub fn down(&self, tid: ThreadId) -> bool {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(tid);
            drop(inner);
            false
        } else {
            true
        }
    }
}
