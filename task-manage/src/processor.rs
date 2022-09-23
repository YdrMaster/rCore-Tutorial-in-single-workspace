
use super::{
    manager::TaskManager,
    scheduler::{Schedule, FifoScheduler},
};
use kernel_context::foreign::ForeignPortal;

/// 处理器
pub struct Processor<T, I: Copy + Ord> {
    /// 异界传送门
    pub portal: ForeignPortal,
    manager: TaskManager<T, I>,
    // 进程管理调度
    scheduler: FifoScheduler<I>,
    // 当前正在运行的进程 ID
    current: Option<I>,
}

impl <T, I: Copy + Ord> Processor<T, I> {
    /// 新建 Processor
    pub const fn new() -> Self {
        Self {
            portal: ForeignPortal::EMPTY,
            manager: TaskManager::new(),
            scheduler: FifoScheduler::new(), 
            current: None,
        }
    }
    /// 找到下一个进程
    pub fn find_next(&mut self) -> Option<&mut T>{
        if let Some(id) = self.scheduler.fetch() {
            if let Some(task) = self.manager.get_task(id) {
                self.current = Some(id);
                Some(task)
            } else {
                None
            }
        } else {
            None
        }        
    }
    /// 设置异界传送门
    pub fn set_portal(&mut self, portal: ForeignPortal) {
        self.portal = portal;
    }
    /// 当前进程进入队首，立即被调度
    pub fn make_current_continue(&mut self) {
        let id = self.current.unwrap();
        self.scheduler.add_front(id);
        self.current = None;
    }
    /// 阻塞当前进程
    pub fn make_current_suspend(&mut self) {
        let id = self.current.unwrap();
        self.scheduler.add_back(id);
        self.current = None;
    }
    /// 结束当前进程
    pub fn make_current_exited(&mut self) {
        let id = self.current.unwrap();
        self.manager.delete(id);
        self.current = None;
    }
    /// 添加进程
    pub fn add(&mut self, id: I, task: T) {
        self.manager.insert(id, task);
        self.scheduler.add_back(id);
    }
    /// 当前进程
    pub fn current(&mut self) -> Option<&mut T> {
        let id = self.current.unwrap();
        self.manager.get_task(id)
    }
    /// 获取某个进程
    #[inline]
    pub fn get_task(&mut self, id: I) -> Option<&mut T> {
        self.manager.get_task(id)
    }
}