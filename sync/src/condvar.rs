use super::{Mutex, UPIntrFreeCell};
use alloc::{collections::VecDeque, sync::Arc};
use task_manage::ThreadId;

/// Condvar
pub struct Condvar {
    /// UPIntrFreeCell<CondvarInner>
    pub inner: UPIntrFreeCell<CondvarInner>,
}

/// CondvarInner
pub struct CondvarInner {
    /// block queue
    pub wait_queue: VecDeque<ThreadId>,
}

impl Condvar {
    /// new
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPIntrFreeCell::new(CondvarInner {
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
    /// 唤醒某个阻塞在当前条件变量上的线程
    pub fn signal(&self) -> Option<ThreadId> {
        let mut inner = self.inner.exclusive_access();
        inner.wait_queue.pop_front()
    }

    /*
    pub fn wait(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.wait_queue.push_back(current_task().unwrap());
        drop(inner);
        block_current_and_run_next();
    }
    */
    /// 将当前线程阻塞在条件变量上
    pub fn wait_no_sched(&self, tid: ThreadId) -> bool {
        self.inner.exclusive_session(|inner| {
            inner.wait_queue.push_back(tid);
        });
        false
    }
    /// 从 mutex 的锁中释放一个线程，并将其阻塞在自己的等待队列中，当前的线程再试图获取这个锁
    pub fn wait_with_mutex(&self, tid: ThreadId, mutex: Arc<dyn Mutex>) -> bool {
        let waking_tid = mutex.unlock().unwrap();
        // 将当前线程阻塞到 condvar 的阻塞队列中
        self.inner.exclusive_session(|inner| {
            inner.wait_queue.push_back(tid);
        });
        mutex.lock(waking_tid);
        false
    }
}
