use alloc::vec::Vec;

use super::id::{ProcId, ThreadId};

/// 线程、进程之间的关系，通过进程的 Id 来查询这个关系
#[cfg(feature = "thread")]
#[doc(cfg(feature = "thread"))]
pub struct ProcThreadRel {
    /// 父进程 Id
    pub parent: ProcId,
    /// 子进程列表
    pub children: Vec<ProcId>,
    /// 已经结束的子进程
    pub dead_children: Vec<(ProcId, isize)>,
    /// 线程
    pub threads: Vec<ThreadId>,
    /// 已经结束的线程
    pub dead_threads: Vec<(ThreadId, isize)>,
}

impl ProcThreadRel {
    /// new/fork 创建进程时使用
    pub fn new(parent_pid: ProcId) -> Self {
        Self {
            parent: parent_pid,
            children: Vec::new(),
            dead_children: Vec::new(),
            threads: Vec::new(),
            dead_threads: Vec::new(),
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
    /// 添加线程
    pub fn add_thread(&mut self, tid: ThreadId) {
        self.threads.push(tid);
    }
    /// 删除线程
    pub fn del_thread(&mut self, tid: ThreadId, exit_code: isize) {
        let pair = self.threads.iter().enumerate().find(|(_, &id)| id == tid);
        if let Some((idx, _)) = pair {
            let dead_thread = self.threads.remove(idx);
            self.dead_threads.push((dead_thread, exit_code));
        }
    }
    /// 等待特定的线程结束
    pub fn wait_thread(&mut self, thread_tid: ThreadId) -> Option<isize> {
        let pair = self
            .dead_threads
            .iter()
            .enumerate()
            .find(|(_, &(id, _))| id == thread_tid);
        if let Some((idx, _)) = pair {
            // 等待的子进程确已结束
            Some(self.dead_threads.remove(idx).1)
        } else {
            let pair = self
                .threads
                .iter()
                .enumerate()
                .find(|(_, &id)| id == thread_tid);
            if let Some(_) = pair {
                // 等待的子进程正在运行
                Some(-2)
            } else {
                // 等待的子进程不存在
                None
            }
        }
    }
}
