/// Manager trait
pub trait Manage<T, I: Copy + Ord> {
    /// 插入 item
    fn insert(&mut self, id: I, item: T);
    /// 删除 item
    fn delete(&mut self, id: I);
    /// 获取 mut item
    fn get_mut(&mut self, id: I) -> Option<&mut T>;
}
