//! 信号的管理和处理模块
//!
//! 信号模块的实际实现见 `signal_impl` 子模块
//!
//!

#![no_std]

extern crate alloc;
use alloc::boxed::Box;
use kernel_context::LocalContext;
pub use signal_defs::{SignalAction, SignalNo, MAX_SIG};

mod signal_result;
pub use signal_result::SignalResult;

/// 一个信号模块需要对外暴露的接口
pub trait Signal: Send + Sync {
    /// 当 fork 一个任务时(在通常的`linux syscall`中，fork是某种参数形式的sys_clone)，
    /// 需要**继承原任务的信号处理函数和掩码**。
    /// 此时 `task` 模块会调用此函数，根据原任务的信号模块生成新任务的信号模块
    fn from_fork(&mut self) -> Box<dyn Signal>;

    /// `sys_exec`会使用。** `sys_exec` 不会继承信号处理函数和掩码**
    fn clear(&mut self);

    /// 添加一个信号
    fn add_signal(&mut self, signal: SignalNo);

    /// 是否当前正在处理信号
    fn is_handling_signal(&self) -> bool;

    /// 设置一个信号处理函数，返回设置是否成功。`sys_sigaction` 会使用。
    /// （**不成功说明设置是无效的，需要在 sig_action 中返回EINVAL**）
    fn set_action(&mut self, signum: SignalNo, action: &SignalAction) -> bool;

    /// 获取一个信号处理函数的值，返回设置是否成功。`sys_sigaction` 会使用
    ///（**不成功说明设置是无效的，需要在 sig_action 中返回EINVAL**）
    fn get_action_ref(&self, signum: SignalNo) -> Option<SignalAction>;

    /// 设置信号掩码，并获取旧的信号掩码，`sys_procmask` 会使用
    fn update_mask(&mut self, mask: usize) -> usize;

    /// 进程执行结果，可能是直接返回用户程序或存栈或暂停或退出
    fn handle_signals(&mut self, current_context: &mut LocalContext) -> SignalResult;

    /// 从信号处理函数中退出，返回值表示是否成功。`sys_sigreturn` 会使用
    fn sig_return(&mut self, current_context: &mut LocalContext) -> bool;
}
