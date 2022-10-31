use alloc::vec::Vec;

use super::id::*;

#[cfg(feature = "proc")]
/// 进程之间的关系，通过进程的 Id 来查询这个关系
pub struct ProcRelation {
    pub parent: ProcId,
    pub children: Vec<ProcId>,
}
#[cfg(feature = "proc")]
impl ProcRelation {
    /// new/fork 创建进程时使用
    pub fn new(parent_pid: ProcId) -> Self {
        Self { 
            parent: parent_pid, 
            children: Vec::new() 
        }
    }
    /// fork 创建子进程时使用
    pub fn add_child(&mut self, child_pid: ProcId) {
        self.children.push(child_pid);
    }
    /// wait 等待子进程结束使用
    pub fn del_child(&mut self, child_pid: ProcId) {
        let pair = self.children
            .iter()
            .enumerate()
            .find(|(_, &id)| id == child_pid);
        if let Some((idx, _)) = pair {
            self.children.remove(idx);
        }
    }
}

#[cfg(feature = "thread")]
/// 线程、进程之间的关系，通过进程的 Id 来查询这个关系
pub struct ProcThreadRel {
    pub parent: ProcId,
    pub children: Vec<ProcId>,
    pub threads: Vec<ThreadId>,
}
#[cfg(feature = "thread")]
impl ProcThreadRel {
    /// new/fork 创建进程时使用
    pub fn new(parent_pid: ProcId, ) -> Self {
        Self { 
            parent: parent_pid, 
            children: Vec::new(),
            threads: Vec::new(),
        }
    }
    /// fork 创建子进程时使用
    pub fn add_child(&mut self, child_pid: ProcId) {
        self.children.push(child_pid);
    }
    /// wait 等待子进程结束使用
    pub fn del_child(&mut self, child_pid: ProcId) {
        let pair = self.children
            .iter()
            .enumerate()
            .find(|(_, &id)| id == child_pid);
        if let Some((idx, _)) = pair {
            self.children.remove(idx);
        }
    }
    /// 添加线程
    pub fn add_thread(&mut self, tid: ThreadId) {
        self.threads.push(tid);
    }
    /// 删除线程
    pub fn del_thread(&mut self, tid: ThreadId) {
        let pair = self.threads
            .iter()
            .enumerate()
            .find(|(_, &id)| id == tid);
        if let Some((idx, _)) = pair {
            self.threads.remove(idx);
        }
    }
}