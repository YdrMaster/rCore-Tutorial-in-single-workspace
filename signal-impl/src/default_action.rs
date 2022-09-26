use super::{SignalNo, SignalResult};

/// 没有处理函数时的默认行为。
/// 参见 `https://venam.nixers.net/blog/unix/2016/10/21/unix-signals.html`
pub enum DefaultAction {
    Terminate(i32), // 结束进程。其实更标准的实现应该细分为 terminate / terminate(core dump) / stop
    Ignore,         // 忽略信号
}

impl From<SignalNo> for DefaultAction {
    fn from(signal_no: SignalNo) -> Self {
        match signal_no {
            SignalNo::SIGCHLD | SignalNo::SIGURG => Self::Ignore,
            _ => Self::Terminate(-(signal_no as i32)),
        }
    }
}

impl Into<SignalResult> for DefaultAction {
    fn into(self) -> SignalResult {
        match self {
            Self::Terminate(exit_code) => SignalResult::ProcessKilled(exit_code),
            Self::Ignore => SignalResult::Ignored,
        }
    }
}
