use crate::process::{Process, TaskId};
use alloc::collections::{BTreeMap, VecDeque};
use kernel_context::foreign::ForeignPortal;
use task_manage::{Manage, Processor};

pub static mut PROCESSOR: Processor<Process, TaskId, ProcManager> = Processor::new();

pub fn init_processor() {
    unsafe {
        PROCESSOR.set_manager(ProcManager::new());
        PROCESSOR.set_portal(ForeignPortal::new());
    }
}

/// 任务管理器
/// `tasks` 中保存所有的任务实体
/// `ready_queue` 删除任务的实体
pub struct ProcManager {
    tasks: BTreeMap<TaskId, Process>,
    ready_queue: VecDeque<TaskId>,
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

impl Manage<Process, TaskId> for ProcManager {
    /// 插入一个新任务
    #[inline]
    fn insert(&mut self, id: TaskId, task: Process) {
        self.tasks.insert(id, task);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    fn get_mut(&mut self, id: TaskId) -> Option<&mut Process> {
        self.tasks.get_mut(&id)
    }
    /// 删除任务实体
    #[inline]
    fn delete(&mut self, id: TaskId) {
        self.tasks.remove(&id);
    }
    /// 添加 id 进入调度队列
    fn add(&mut self, id: TaskId) {
        self.ready_queue.push_back(id);
    }
    /// 从调度队列中取出 id
    fn fetch(&mut self) -> Option<TaskId> {
        self.ready_queue.pop_front()
    }
}
