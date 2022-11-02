use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::ThreadId;

use super::manager::Manage;
use super::scheduler::Schedule;
use super::id::ProcId;
use super::ProcThreadRel;
use core::marker::PhantomData;

#[cfg(feature = "thread")]

/// PThreadManager 数据结构，只管理进程以及进程之间的父子关系
/// P 表示进程, T 表示线程
pub struct PThreadManager<P, T, MT: Manage<T, ThreadId> + Schedule<ThreadId>, MP: Manage<P, ProcId>> {
    // 进程之间父子关系
    relation: BTreeMap<ProcId, ProcThreadRel>,
    // 进程管理
    proc_manager: Option<MP>,
    // 线程所属的进程之间的映射关系
    tid2pid: BTreeMap<ThreadId, ProcId>,
    // 进程对象管理和调度
    manager: Option<MT>,
    // 当前正在运行的线程 ID
    current: Option<ThreadId>,
    phantom_t: PhantomData<T>,
    phantom_p: PhantomData<P>,
}

impl<P, T, MT: Manage<T, ThreadId> + Schedule<ThreadId>, MP: Manage<P, ProcId>> PThreadManager<P, T, MT, MP> {
    /// 新建 PThreadManager
    pub const fn new() -> Self {
        Self {
            relation: BTreeMap::new(),
            proc_manager: None,
            tid2pid: BTreeMap::new(),
            manager: None,
            current: None,
            phantom_t: PhantomData::<T>,
            phantom_p: PhantomData::<P>,
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
    /// 设置 manager
    pub fn set_manager(&mut self, manager: MT) {
        self.manager = Some(manager);
    }
    /// 设置 proc_manager
    pub fn set_proc_manager(&mut self, proc_manager: MP) {
        self.proc_manager = Some(proc_manager);
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
        let pid = self.tid2pid.remove(&id).unwrap();
        if let Some(current_relation) = self.relation.get_mut(&pid) {
            current_relation.del_thread(id);
        }
        if self.thread_count(pid) == 0 {
            self.del_proc(pid);
        }
        self.current = None;
    }
    /// 添加线程
    pub fn add(&mut self, id: ThreadId, task: T, pid: ProcId) {
        self.manager.as_mut().unwrap().insert(id, task);
        self.manager.as_mut().unwrap().add(id);
        // 增加线程与进程之间的从属关系
        if let Some(parent_relation) = self.relation.get_mut(&pid) {
            parent_relation.add_thread(id);
            self.tid2pid.insert(id, pid);
        }
    }
    /// 当前线程
    pub fn current(&mut self) -> Option<&mut T> {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().get_mut(id)
    }
    /// 获取某个线程
    #[inline]
    pub fn get_task(&mut self, id: ThreadId) -> Option<&mut T> {
        self.manager.as_mut().unwrap().get_mut(id)
    }
    /// 添加进程
    pub fn add_proc(&mut self, id: ProcId, proc: P, parent: ProcId) {
        self.proc_manager.as_mut().unwrap().insert(id, proc);
        if let Some(parent_relation) = self.relation.get_mut(&parent) {
            parent_relation.add_child(id);
        }
        self.relation.insert(id, ProcThreadRel::new(parent));
    }
    /// 查询进程
    pub fn get_proc(&mut self, id: ProcId) -> Option<&mut P> {
        self.proc_manager.as_mut().unwrap().get_mut(id)
    }
    /// 结束当前进程
    pub fn del_proc(&mut self, id: ProcId) {
        self.proc_manager.as_mut().unwrap().delete(id);
        // 进程结束时维护父子关系，进程删除后，所有的子进程交给 0 号进程来维护
        assert!(self.relation.get_mut(&id).unwrap().threads.is_empty());
        let current_relation = self.relation.remove(&id).unwrap();
        if let Some(parent_relation) = self.relation.get_mut(&current_relation.parent) {
            parent_relation.del_child(id);
        }
        if let Some(root_relation) = self.relation.get_mut(&ProcId::from_usize(0)) {
            for i in &current_relation.children {
                root_relation.add_child(*i);
            }
        }
        self.current = None;
    }
    /// 某个进程的子进程是否全部执行结束，如果全部执行结束，则表示这个进程也可以结束
    pub fn has_child(&self, id: ProcId) -> bool {
        !self.relation.get(&id).unwrap().children.is_empty()
    }
    /// 某个进程的线程数量
    pub fn thread_count(&self, id: ProcId) -> usize {
        self.relation.get(&id).unwrap().threads.len()
    }
    /// 查询进程的线程
    pub fn get_thread(&mut self, id: ProcId) -> Option<&Vec<ThreadId>> {
        self.relation.get_mut(&id).map(|p| &p.threads)
    }
    /// 获取当前线程所属的进程
    pub fn get_current_proc(&mut self) -> Option<&mut P> {
        let id = self.current.unwrap();
        let pid = self.tid2pid.get(&id).unwrap();
        self.proc_manager.as_mut().unwrap().get_mut(*pid)
    }
}
