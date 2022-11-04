use alloc::collections::BTreeMap;

use super::id::ProcId;
use super::manager::Manage;
use super::scheduler::Schedule;
use super::ProcRel;
use core::marker::PhantomData;

/// ProcManager 数据结构，只管理进程以及进程之间的父子关系
/// P 表示进程
#[cfg(feature = "proc")]
#[doc(cfg(feature = "proc"))]
pub struct PManager<P, MP: Manage<P, ProcId> + Schedule<ProcId>> {
    // 进程之间父子关系
    rel_map: BTreeMap<ProcId, ProcRel>,
    // 进程对象管理和调度
    manager: Option<MP>,
    // 当前正在运行的进程 ID
    current: Option<ProcId>,
    phantom_data: PhantomData<P>,
}

impl<P, MP: Manage<P, ProcId> + Schedule<ProcId>> PManager<P, MP> {
    /// 新建 PManager
    pub const fn new() -> Self {
        Self {
            rel_map: BTreeMap::new(),
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
    /// 阻塞当前进程
    pub fn make_current_suspend(&mut self) {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().add(id);
        self.current = None;
    }
    /// 结束当前进程，只会删除进程的内容，以及与当前进程相关的关系
    pub fn make_current_exited(&mut self, exit_code: isize) {
        let id = self.current.unwrap();
        self.manager.as_mut().unwrap().delete(id);
        let current_rel = self.rel_map.remove(&id).unwrap();
        let parent_pid = current_rel.parent;
        let children = current_rel.children;
        // 从父进程中删除当前进程
        if let Some(parent_rel) = self.rel_map.get_mut(&parent_pid) {
            parent_rel.del_child(id, exit_code);
        }
        // 把当前进程的所有子进程转移到 0 号进程
        for i in children {
            self.rel_map.get_mut(&i).unwrap().parent = ProcId::from_usize(0);
            self.rel_map
                .get_mut(&ProcId::from_usize(0))
                .unwrap()
                .add_child(i);
        }
        self.current = None;
    }
    /// 添加进程，需要指明创建的进程的父进程 Id
    pub fn add(&mut self, id: ProcId, task: P, parent: ProcId) {
        self.manager.as_mut().unwrap().insert(id, task);
        self.manager.as_mut().unwrap().add(id);
        if let Some(parent_relation) = self.rel_map.get_mut(&parent) {
            parent_relation.add_child(id);
        }
        self.rel_map.insert(id, ProcRel::new(parent));
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
    /// wait 系统调用，返回结束的子进程 id 和 exit_code，正在运行的子进程不返回 None，返回 (-2, -1)
    pub fn wait(&mut self, child_pid: ProcId) -> Option<(ProcId, isize)> {
        let id = self.current.unwrap();
        let current_rel = self.rel_map.get_mut(&id).unwrap();
        if child_pid.get_usize() == usize::MAX {
            current_rel.wait_any_child()
        } else {
            current_rel.wait_child(child_pid)
        }
    }
}
