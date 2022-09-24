/// 每个信号处理函数的信息
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    pub handler: usize,
    pub mask: i32,
}

impl Default for SignalAction {
    fn default() -> Self {
        Self {
            handler: 0,
            mask: 40,
        }
    }
}
