use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::ThreadId;

use super::id::ProcId;
use super::manager::Manage;
use super::scheduler::Schedule;
use super::ProcThreadRel;
use core::marker::PhantomData;

#[cfg(feature = "thread")]
#[doc(cfg(feature = "thread"))]
/// PThreadManager 数据结构，只管理进程以及进程之间的父子关系
/// P 表示进程, T 表示线程
pub struct PThreadManager<P, T, MT: Manage<T, ThreadId> + Schedule<ThreadId>, MP: Manage<P, ProcId>>
{
    // 进程之间父子关系
    rel_map: BTreeMap<ProcId, ProcThreadRel>,
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

impl<P, T, MT: Manage<T, ThreadId> + Schedule<ThreadId>, MP: Manage<P, ProcId>>
    PThreadManager<P, T, MT, MP>
{
    /// 新建 PThreadManager
    pub const fn new() -> Self {
        Self {
            rel_map: BTreeMap::new(),
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
    /// 当前线程重新入队
    pub fn make_current_suspend(&mut self) {
        if let Some(id) = self.current {
            self.manager.as_mut().unwrap().add(id);
            self.current = None;
        }
    }
    /// 结束当前线程
    pub fn make_current_exited(&mut self, exit_code: isize) {
        if let Some(id) = self.current {
            self.manager.as_mut().unwrap().delete(id);
            // 线程结束时维护与父进程之间的关系
            let pid = self.tid2pid.remove(&id).unwrap();
            let mut flag = false;
            if let Some(current_rel) = self.rel_map.get_mut(&pid) {
                current_rel.del_thread(id, exit_code);
                // 如果线程数量为 0，则需要把当前线程所属的进程给删除掉（所有等待的线程都已经结束）
                if current_rel.threads.is_empty() {
                    flag = true;
                }
            }
            if flag {
                self.del_proc(pid, exit_code);
            }
            self.current = None;
        }
    }
    /// 让当前线程阻塞
    pub fn make_current_blocked(&mut self) {
        if let Some(_) = self.current {
            self.current = None;
        }
    }
    /// 某个线程重新入队
    pub fn re_enque(&mut self, id: ThreadId) {
        self.manager.as_mut().unwrap().add(id);
    }
    /// 添加线程
    pub fn add(&mut self, id: ThreadId, task: T, pid: ProcId) {
        self.manager.as_mut().unwrap().insert(id, task);
        self.manager.as_mut().unwrap().add(id);
        // 增加线程与进程之间的从属关系
        if let Some(parent_rel) = self.rel_map.get_mut(&pid) {
            parent_rel.add_thread(id);
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
        if let Some(parent_rel) = self.rel_map.get_mut(&parent) {
            parent_rel.add_child(id);
        }
        self.rel_map.insert(id, ProcThreadRel::new(parent));
    }
    /// 查询进程
    pub fn get_proc(&mut self, id: ProcId) -> Option<&mut P> {
        self.proc_manager.as_mut().unwrap().get_mut(id)
    }
    /// 结束当前进程
    pub fn del_proc(&mut self, id: ProcId, exit_code: isize) {
        // 删除进程实体
        self.proc_manager.as_mut().unwrap().delete(id);
        // 进程结束时维护父子关系，进程删除后，所有的子进程交给 0 号进程来维护
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
    }
    /// wait 系统调用，返回结束的子进程 id 和 exit_code，正在运行的子进程不返回 None，返回 (-2, -1)
    pub fn wait(&mut self, child_pid: ProcId) -> Option<(ProcId, isize)> {
        let id = self.current.unwrap();
        let pid = self.tid2pid.get(&id).unwrap();
        let current_rel = self.rel_map.get_mut(pid).unwrap();
        if child_pid.get_usize() == usize::MAX {
            current_rel.wait_any_child()
        } else {
            current_rel.wait_child(child_pid)
        }
    }
    /// wait_tid 系统调用
    pub fn waittid(&mut self, thread_tid: ThreadId) -> Option<isize> {
        let id = self.current.unwrap();
        let pid = self.tid2pid.get(&id).unwrap();
        let current_rel = self.rel_map.get_mut(pid).unwrap();
        current_rel.wait_thread(thread_tid)
    }
    /// 某个进程的线程数量
    pub fn thread_count(&self, id: ProcId) -> usize {
        self.rel_map.get(&id).unwrap().threads.len()
    }
    /// 查询进程的线程
    pub fn get_thread(&mut self, id: ProcId) -> Option<&Vec<ThreadId>> {
        self.rel_map.get_mut(&id).map(|p| &p.threads)
    }
    /// 获取当前线程所属的进程
    pub fn get_current_proc(&mut self) -> Option<&mut P> {
        if let Some(id) = self.current {
            let pid = self.tid2pid.get(&id).unwrap();
            self.proc_manager.as_mut().unwrap().get_mut(*pid)
        } else {
            None
        }
    }
}
