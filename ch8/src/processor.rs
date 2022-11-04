use crate::process::{Process, Thread};
use alloc::collections::{BTreeMap, VecDeque};
use rcore_task_manage::{Manage, PThreadManager, ProcId, Schedule, ThreadId};

pub static mut PROCESSOR: PThreadManager<Process, Thread, ThreadManager, ProcManager> =
    PThreadManager::new();

/// 任务管理器
/// `tasks` 中保存所有的任务实体
/// `ready_queue` 删除任务的实体
pub struct ThreadManager {
    tasks: BTreeMap<ThreadId, Thread>,
    ready_queue: VecDeque<ThreadId>,
}

impl ThreadManager {
    /// 新建任务管理器
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            ready_queue: VecDeque::new(),
        }
    }
}

impl Manage<Thread, ThreadId> for ThreadManager {
    /// 插入一个新任务
    #[inline]
    fn insert(&mut self, id: ThreadId, task: Thread) {
        self.tasks.insert(id, task);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    fn get_mut(&mut self, id: ThreadId) -> Option<&mut Thread> {
        self.tasks.get_mut(&id)
    }
    /// 删除任务实体
    #[inline]
    fn delete(&mut self, id: ThreadId) {
        self.tasks.remove(&id);
    }
}

impl Schedule<ThreadId> for ThreadManager {
    /// 添加 id 进入调度队列
    fn add(&mut self, id: ThreadId) {
        self.ready_queue.push_back(id);
    }
    /// 从调度队列中取出 id
    fn fetch(&mut self) -> Option<ThreadId> {
        self.ready_queue.pop_front()
    }
}

/// 进程管理器
/// `procs` 中保存所有的进程实体
pub struct ProcManager {
    procs: BTreeMap<ProcId, Process>,
}

impl ProcManager {
    /// 新建进程管理器
    pub fn new() -> Self {
        Self {
            procs: BTreeMap::new(),
        }
    }
}

impl Manage<Process, ProcId> for ProcManager {
    /// 插入一个新任务
    #[inline]
    fn insert(&mut self, id: ProcId, item: Process) {
        self.procs.insert(id, item);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    fn get_mut(&mut self, id: ProcId) -> Option<&mut Process> {
        self.procs.get_mut(&id)
    }
    /// 删除任务实体
    #[inline]
    fn delete(&mut self, id: ProcId) {
        self.procs.remove(&id);
    }
}
