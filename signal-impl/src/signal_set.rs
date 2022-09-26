//! 字符数组。可取并集和差集，也可对给定的 mask 取首位
//!

#[derive(Clone, Copy, Debug)]
/// bit数组
pub struct SignalSet(pub usize);

impl SignalSet {
    /// 新建一个空的数组
    pub fn empty() -> Self {
        Self(0)
    }
    /// 新建一个数组，长为 usize = 8Byte
    pub fn new(v: usize) -> Self {
        Self(v)
    }
    /// 直接暴力写入 SignalSet
    pub fn reset(&mut self, v: usize) {
        self.0 = v;
    }
    /// 清空 SignalSet
    pub fn clear(&mut self) {
        self.0 = 0;
    }
    /// 是否包含第 k 个 bit
    pub fn contain_bit(&self, kth: usize) -> bool {
        ((self.0 >> kth) & 1) > 0
    }
    /// 新增一个 bit
    pub fn add_bit(&mut self, kth: usize) {
        self.0 |= 1 << kth;
    }
    /// 删除一个 bit
    pub fn remove_bit(&mut self, kth: usize) {
        self.0 &= !(1 << kth);
    }
    /// 取交集
    pub fn get_union(&mut self, set: SignalSet) {
        self.0 |= set.0;
    }
    /// 取差集，即去掉 set 中的内容
    pub fn get_difference(&mut self, set: SignalSet) {
        self.0 &= !(set.0);
    }
    /// 直接设置为新值
    pub fn set_new(&mut self, set: SignalSet) -> usize {
        let old = self.0;
        self.0 = set.0;
        old
    }
    /// 获取后缀0个数，可以用来寻找最小的1
    pub fn get_trailing_zeros(&self) -> u32 {
        self.0.trailing_zeros()
    }
    /// 寻找不在mask中的最小的 1 的位置，如果有，返回其位置，如没有则返回 None。
    pub fn find_first_one(&self, mask: SignalSet) -> Option<usize> {
        let ans = (self.0 & !mask.0).trailing_zeros() as usize;
        if ans == 64 {
            None
        } else {
            Some(ans)
        }
    }
}

impl From<usize> for SignalSet {
    fn from(v: usize) -> Self {
        Self(v)
    }
}
