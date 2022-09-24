use core::marker::PhantomData;

use super::manager::Manage;
use kernel_context::foreign::ForeignPortal;

/// Processor 数据结构
pub struct Processor<T, I: Copy + Ord, M: Manage<T, I>> {
    /// 异界传送门
    pub portal: ForeignPortal,
    // 进程对象管理和调度
    manager: Option<M>,
    // 当前正在运行的进程 ID
    current: Option<I>,
    phantom_data: PhantomData<T>,
}

impl<T, I: Copy + Ord, M: Manage<T, I>> Processor<T, I, M> {
    /// 新建 Processor
    pub const fn new() -> Self {
        Self {
            portal: ForeignPortal::EMPTY,
            manager: None,
            current: None,
            phantom_data: PhantomData::<T>,
        }
    }
    /// 找到下一个进程
    pub fn find_next(&mut self) -> Option<&mut T> {
        if let Some(id) = self.manager.as_mut().unwrap().fetch() {
            if let Some(task) = self.manager.as_mut().unwrap().get_mut(id) {
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
    /// 设置 manager
    pub fn set_manager(&mut self, manager: M) {
        self.manager = Some(manager);
    }
    /// 当前进程进入队首，立即被调度，TODO
    pub fn make_current_continue(&mut self) {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().add(id);
        self.current = None;
        todo!("not complete");
    }
    /// 阻塞当前进程
    pub fn make_current_suspend(&mut self) {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().add(id);
        self.current = None;
    }
    /// 结束当前进程
    pub fn make_current_exited(&mut self) {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().delete(id);
        self.current = None;
    }
    /// 添加进程
    pub fn add(&mut self, id: I, task: T) {
        self.manager.as_mut().unwrap().insert(id, task);
        self.manager.as_mut().unwrap().add(id);
    }
    /// 当前进程
    pub fn current(&mut self) -> Option<&mut T> {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().get_mut(id)
    }
    /// 获取某个进程
    #[inline]
    pub fn get_task(&mut self, id: I) -> Option<&mut T> {
        self.manager.as_mut().unwrap().get_mut(id)
    }
}
