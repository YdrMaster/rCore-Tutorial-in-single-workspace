//! 任务管理 lib

#![no_std]
#![feature(const_btree_new)]
#![deny(warnings, missing_docs)]

extern crate alloc;
use alloc::{collections::BTreeMap, vec::Vec};

/// 任务管理器
/// `tasks` 中保存所有的任务实体
/// `task_queue` 负责进行调度，任务需要调度，则任务的 id 会在 task_queue 中
/// 从中取出任务，并不会删除任务，之后需要调度则需要将 id 重新插回 `task_queue` 中
/// 只能通过 `del` 才可以删除任务的实体
pub struct TaskManager<T, I: Copy + Ord> {
    tasks: BTreeMap<I, T>,
    task_queue: Vec<I>,
    current: Option<I>,
}

impl<T, I: Copy + Ord> TaskManager<T, I> {
    /// 新建任务管理器
    pub const fn new() -> Self {
        Self { 
            tasks: BTreeMap::new(),
            task_queue: Vec::new(),
            current: None,
        }
    }
    /// 插入一个新任务
    #[inline]
    pub fn insert(&mut self, id: I, task: T) {
        self.add(id);
        self.tasks.insert(id, task);
    }
    /// 根据 id 获取对应的任务
    #[inline]
    pub fn get_task(&mut self, id: I) -> Option<&mut T> {
        self.tasks.get_mut(&id)
    }
    /// 将没有执行完的任务重新插回调度队列中
    #[inline]
    pub fn add(&mut self, id: I) {
        self.task_queue.push(id);
    }
    /// 删除任务实体
    #[inline]
    pub fn del(&mut self, id: I) {
        self.tasks.remove(&id);
    }
    /// 取出任务
    #[inline]
    pub fn fetch(&mut self) -> Option<&mut T> {
        if !self.task_queue.is_empty() {
            let id = self.task_queue.remove(0);
            self.current = Some(id);
            self.get_task(id)
        } else {
            None
        }
    }
    /// 获取当前正在执行的任务
    #[inline]
    pub fn current(&mut self) -> Option<&mut T> {
        self.get_task(self.current.unwrap())
    }
}
