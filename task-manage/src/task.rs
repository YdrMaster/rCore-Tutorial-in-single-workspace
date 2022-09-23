
/// Execute 特性，进程、线程、协程都需要实现这个 trait
pub trait Execute {
    /// base 表示异界传送门的地址
    fn execute(&mut self, base: usize);
}