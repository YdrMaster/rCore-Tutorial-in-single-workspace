//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

use core::alloc::Layout;
use page_table::VmMeta;
use xmas_elf::ElfFile;

/// 4 KiB 页类型。
#[repr(C, align(4096))]
pub struct Page4K([u8; 4096]);

impl Page4K {
    /// 空白页。
    pub const ZERO: Self = Self([0; 4096]);
    /// 页布局。
    pub const LAYOUT: Layout = Layout::new::<Self>();
    /// 页虚存地址。
    #[inline]
    pub fn addr(&self) -> usize {
        self as *const _ as _
    }
}

/// 计算 `Meta` 虚存方案下加载应用程序总共需要多少个页。包括存储各个加载段数据的页和页表页。
pub fn count_pages<Meta: VmMeta>(elf: &ElfFile) -> usize {
    use page_table::VAddr;
    use xmas_elf::program::Type::*;

    // 至少一个根页表
    let mut ans = 1usize;
    // 每级已分配页表的覆盖范围
    // elf 中程序段是按虚址升序排列的，只需要记一个末页号
    // N 级页表需要 N-1 个号，因为根页表不需要算
    // 所以最大 5 级页表只需要 4 个
    let mut indices = [0usize; 4];

    assert!(Meta::MAX_LEVEL <= indices.len());

    // 各级页表页号位数迭代器
    let iter = Meta::LEVEL_BITS
        .iter()
        .take(Meta::MAX_LEVEL)
        .copied()
        .scan(0, |tail, bits| {
            *tail += bits;
            Some(*tail)
        })
        .enumerate();
    // 遍历程序段
    for program in elf.program_iter() {
        if let Ok(Load) = program.get_type() {
            let base = program.virtual_addr() as usize;
            let size = program.mem_size() as usize;
            let start = VAddr::<Meta>::new(base).floor().val();
            let end = VAddr::<Meta>::new(base + size).ceil().val();
            // 更新中间页表占用页数量
            for (i, bits) in iter.clone() {
                let start = core::cmp::max(indices[i], start >> bits);
                let end = (end + (1 << bits) - 1) >> bits;
                if end > start {
                    indices[i] = end;
                    ans += end - start;
                } else {
                    break;
                }
            }
            // 更新数据页数量
            ans += end - start;
        }
    }
    ans
}
