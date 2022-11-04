use crate::process::Process;
use alloc::collections::{BTreeMap, VecDeque};
use rcore_task_manage::{Manage, PManager, ProcId, Schedule};

pub static mut PROCESSOR: PManager<Process, ProcManager> = PManager::new();

/// 任务管理器
/// `tasks` 中保存所有的任务实体
/// `ready_queue` 删除任务的实体
pub struct ProcManager {
    tasks: BTreeMap<ProcId, Process>,
    ready_queue: VecDeque<ProcId>,
}

impl ProcManager {
    /// 新建任务管理器
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            ready_queue: VecDeque::new(),
        }
    }
}

impl Manage<Process, ProcId> for ProcManager {
    /// 插入一个新任务
    #[inline]
    fn insert(&mut self, id: ProcId, task: Process) {
        self.tasks.insert(id, task);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    fn get_mut(&mut self, id: ProcId) -> Option<&mut Process> {
        self.tasks.get_mut(&id)
    }
    /// 删除任务实体
    #[inline]
    fn delete(&mut self, id: ProcId) {
        self.tasks.remove(&id);
    }
}

impl Schedule<ProcId> for ProcManager {
    /// 添加 id 进入调度队列
    fn add(&mut self, id: ProcId) {
        self.ready_queue.push_back(id);
    }
    /// 从调度队列中取出 id
    fn fetch(&mut self) -> Option<ProcId> {
        self.ready_queue.pop_front()
    }
}
