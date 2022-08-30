use core::{marker::PhantomData, ops::Range};
use page_table::{VAddr, VmMeta};

/// 虚地址空间页数计算器。
///
/// 计算 `Meta` 虚存方案下加载应用程序总共需要多少个页。包括存储各个加载段数据的页和页表页。
///
/// 计算总是比分配快。预先计算可以方便建立地址空间时一次分配所有必要的物理页。
pub struct PageCounter<Meta: VmMeta> {
    /// 需要的总页数
    count: usize,
    /// 每级已分配页表的覆盖范围
    /// elf 中程序段是按虚址升序排列的，只需要记一个末页号
    /// N 级页表需要 N-1 个号，因为根页表不需要算
    /// 所以最大 5 级页表只需要 4 个
    indices: [usize; 4],
    _phantom: PhantomData<Meta>,
}

impl<Meta: VmMeta> PageCounter<Meta> {
    /// 新建空的计算器。
    #[inline]
    pub fn new() -> Self {
        assert!(Meta::MAX_LEVEL <= 4);
        Self {
            // 至少一个根页表
            count: 1,
            indices: [0; 4],
            _phantom: PhantomData,
        }
    }

    /// 当前页数。
    #[inline]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// 插入一段需要映射的虚存。
    ///
    /// # NOTICE
    ///
    /// 必须按虚地址升序传入。
    pub fn insert(&mut self, segments: impl Iterator<Item = Range<VAddr<Meta>>>) {
        // 遍历虚存段
        for area in segments {
            let start = area.start.floor().val();
            let end = area.end.ceil().val();
            // 更新中间页表占用页数量
            for (i, bits) in bits_iter::<Meta>() {
                let start = core::cmp::max(self.indices[i], start >> bits);
                let end = (end + (1 << bits) - 1) >> bits;
                if end > start {
                    self.indices[i] = end;
                    self.count += end - start;
                } else {
                    break;
                }
            }
            // 更新数据页数量
            self.count += end - start;
        }
    }
}

/// 各级页表页号位数迭代器。
#[inline]
fn bits_iter<Meta: VmMeta>() -> impl Iterator<Item = (usize, usize)> {
    Meta::LEVEL_BITS
        .iter()
        .take(Meta::MAX_LEVEL)
        .copied()
        .scan(0, |tail, bits| {
            *tail += bits;
            Some(*tail)
        })
        .enumerate()
}
