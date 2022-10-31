use alloc::collections::BTreeMap;

use super::manager::Manage;
use super::scheduler::Schedule;
use super::id::ProcId;
use super::ProcRelation;
use core::marker::PhantomData;

/// Processor 数据结构，只管理进程以及进程之间的父子关系
/// P 表示进程
pub struct Processor<P, MP: Manage<P, ProcId> + Schedule<ProcId>> {
    // 进程之间父子关系
    relation: BTreeMap<ProcId, ProcRelation>,
    // 进程对象管理和调度
    manager: Option<MP>,
    // 当前正在运行的进程 ID
    current: Option<ProcId>,
    phantom_data: PhantomData<P>,
}

impl<P, MP: Manage<P, ProcId> + Schedule<ProcId>> Processor<P, MP> {
    /// 新建 Processor
    pub const fn new() -> Self {
        Self {
            relation: BTreeMap::new(),
            manager: None,
            current: None,
            phantom_data: PhantomData::<P>,
        }
    }
    /// 找到下一个进程
    pub fn find_next(&mut self) -> Option<&mut P> {
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
    /// 设置 manager
    pub fn set_manager(&mut self, manager: MP) {
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
        // 进程结束时维护父子关系，进程删除后，所有的子进程交给 0 号进程来维护
        let current_relation = self.relation.remove(&id).unwrap();
        if let Some(parent_relation) = self.relation.get_mut(&current_relation.parent) {
            parent_relation.del_children(id);
        }
        if let Some(root_relation) = self.relation.get_mut(&ProcId::from_usize(0)) {
            for i in &current_relation.children {
                root_relation.add_children(*i);
            }
        }
        self.current = None;
    }
    /// 添加进程
    pub fn add(&mut self, id: ProcId, task: P, parent: ProcId) {
        self.manager.as_mut().unwrap().insert(id, task);
        self.manager.as_mut().unwrap().add(id);
        if let Some(parent_relation) = self.relation.get_mut(&parent) {
            parent_relation.add_children(id);
        }
        self.relation.insert(id, ProcRelation::new(parent));
    }
    /// 当前进程
    pub fn current(&mut self) -> Option<&mut P> {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().get_mut(id)
    }
    /// 获取某个进程
    #[inline]
    pub fn get_task(&mut self, id: ProcId) -> Option<&mut P> {
        self.manager.as_mut().unwrap().get_mut(id)
    }
    /// 某个进程的子进程是否全部执行结束，如果全部执行结束，则表示这个进程也可以结束
    pub fn can_end(&self, id: ProcId) -> bool {
        self.relation.get(&id).unwrap().children.is_empty()
    }
}
