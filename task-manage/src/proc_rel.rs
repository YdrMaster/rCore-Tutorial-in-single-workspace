use super::id::ProcId;
use alloc::vec::Vec;

/// 进程之间的关系，通过进程的 Id 来查询这个关系
#[cfg(feature = "proc")]
#[doc(cfg(feature = "proc"))]
pub struct ProcRel {
    /// 父进程 Id
    pub parent: ProcId,
    /// 子进程列表
    pub children: Vec<ProcId>,
    /// 已经结束的进程
    pub dead_children: Vec<(ProcId, isize)>,
}

impl ProcRel {
    /// new/fork 创建进程时使用
    pub fn new(parent_pid: ProcId) -> Self {
        Self {
            parent: parent_pid,
            children: Vec::new(),
            dead_children: Vec::new(),
        }
    }
    /// 添加子进程 Id
    pub fn add_child(&mut self, child_pid: ProcId) {
        self.children.push(child_pid);
    }
    /// 子进程结束，子进程 Id 被移入到 dead_children 队列中，等待 wait 系统调用来处理
    pub fn del_child(&mut self, child_pid: ProcId, exit_code: isize) {
        let pair = self
            .children
            .iter()
            .enumerate()
            .find(|(_, &id)| id == child_pid);
        if let Some((idx, _)) = pair {
            let dead_child = self.children.remove(idx);
            self.dead_children.push((dead_child, exit_code));
        }
    }
    /// 等待任意一个结束的子进程，直接弹出 dead_children 队首，如果队列为空，则返回 -2
    pub fn wait_any_child(&mut self) -> Option<(ProcId, isize)> {
        if self.dead_children.is_empty() {
            if self.children.is_empty() {
                None
            } else {
                Some((ProcId::from_usize(-2 as _), -1))
            }
        } else {
            self.dead_children.pop()
        }
    }
    /// 等待特定的子进程
    pub fn wait_child(&mut self, child_pid: ProcId) -> Option<(ProcId, isize)> {
        let pair = self
            .dead_children
            .iter()
            .enumerate()
            .find(|(_, &(id, _))| id == child_pid);
        if let Some((idx, _)) = pair {
            // 等待的子进程确已结束
            Some(self.dead_children.remove(idx))
        } else {
            let pair = self
                .children
                .iter()
                .enumerate()
                .find(|(_, &id)| id == child_pid);
            if let Some(_) = pair {
                // 等待的子进程正在运行
                Some((ProcId::from_usize(-2 as _), -1))
            } else {
                // 等待的子进程不存在
                None
            }
        }
    }
}
