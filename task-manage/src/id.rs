use core::sync::atomic::{AtomicUsize, Ordering};

/// 进程 Id
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd)]
pub struct ProcId(usize);

impl ProcId {
    ///
    pub fn new() -> Self {
        // 任务编号计数器，任务编号自增
        static PID_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = PID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(id)
    }
    ///
    pub fn from_usize(v: usize) -> Self {
        Self(v)
    }
    ///
    pub fn get_usize(&self) -> usize {
        self.0
    }
}

/// 线程 Id
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd)]
pub struct ThreadId(usize);

impl ThreadId {
    ///
    pub fn new() -> Self {
        // 任务编号计数器，任务编号自增
        static TID_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = TID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(id)
    }
    ///
    pub fn from_usize(v: usize) -> Self {
        Self(v)
    }
    ///
    pub fn get_usize(&self) -> usize {
        self.0
    }
}

/// 协程 Id
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd)]
pub struct CoroId(usize);

impl CoroId {
    ///
    pub fn new() -> Self {
        // 任务编号计数器，任务编号自增
        static CID_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = CID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(id)
    }
    ///
    pub fn from_usize(v: usize) -> Self {
        Self(v)
    }
    ///
    pub fn get_usize(&self) -> usize {
        self.0
    }
}
