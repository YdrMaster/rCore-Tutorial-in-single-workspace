extern crate alloc;
use alloc::collections::BTreeMap;

/// 任务管理器
/// `tasks` 中保存所有的任务实体
/// `del` 删除任务的实体
pub struct TaskManager<T, I: Copy + Ord> {
    tasks: BTreeMap<I, T>,
}

impl<T, I: Copy + Ord> TaskManager<T, I> {
    /// 新建任务管理器
    pub const fn new() -> Self {
        Self { 
            tasks: BTreeMap::new(),
        }
    }
    /// 插入一个新任务
    #[inline]
    pub fn insert(&mut self, id: I, task: T) {
        self.tasks.insert(id, task);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    pub fn get_task(&mut self, id: I) -> Option<&mut T> {
        self.tasks.get_mut(&id)
    }
    /// 删除任务实体
    #[inline]
    pub fn delete(&mut self, id: I) {
        self.tasks.remove(&id);
    }
}
