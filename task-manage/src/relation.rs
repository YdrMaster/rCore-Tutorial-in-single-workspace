use alloc::vec::Vec;

use super::id::*;

#[cfg(feature = "proc")]
pub struct ProcRelation {
    pub parent: ProcId,
    pub children: Vec<ProcId>,
}

impl ProcRelation {
    /// new/fork 创建进程时使用
    pub fn new(parent_pid: ProcId) -> Self {
        Self { 
            parent: parent_pid, 
            children: Vec::new() 
        }
    }
    /// fork 创建子进程时使用
    pub fn add_children(&mut self, child_pid: ProcId) {
        self.children.push(child_pid);
    }
    /// wait 等待子进程结束使用
    pub fn del_children(&mut self, child_pid: ProcId) {
        let pair = self.children
            .iter()
            .enumerate()
            .find(|(_, &id)| id == child_pid);
        if let Some((idx, _)) = pair {
            self.children.remove(idx);
        }
    }
}