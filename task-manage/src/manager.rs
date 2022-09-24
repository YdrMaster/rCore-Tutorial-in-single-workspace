/// Manager trait
pub trait Manage<T, I: Copy + Ord> {
    /// 插入 item
    fn insert(&mut self, id: I, item: T);
    /// 删除 item
    fn delete(&mut self, id: I);
    /// 获取 mut item
    fn get_mut(&mut self, id: I) -> Option<&mut T>;
    /// 添加 id 进入调度队列
    fn add(&mut self, id: I);
    /// 从调度队列中取出 id
    fn fetch(&mut self) -> Option<I>;
}
