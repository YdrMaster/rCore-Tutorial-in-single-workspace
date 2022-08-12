use core::sync::atomic::{
    AtomicU32,
    Ordering::{Relaxed, SeqCst},
};
use idx_alloc::{AllocError, IdxAllocator};

/// 低能型位图序号分配器，由 N 个 u32 组成。
///
/// 如果用作页帧分配器，每个 u32 管理 32 * 4 KiB = 128 KiB。
pub struct NaiveBitmapIdxAllocator<const N: usize> {
    /// 对分配可能成功的组号的估计。
    hint: AtomicU32,
    /// 分配器的位图。
    bits: [AtomicU32; N],
}

impl<const N: usize> NaiveBitmapIdxAllocator<N> {
    pub const fn new() -> Self {
        const FULL: AtomicU32 = AtomicU32::new(!0);
        Self {
            hint: AtomicU32::new(0),
            bits: [FULL; N],
        }
    }
}

impl<const N: usize> IdxAllocator for NaiveBitmapIdxAllocator<N> {
    fn allocate_fixed(&self, _base: usize, _size: usize) -> Result<(), AllocError> {
        return Err(AllocError::Unsupported);
    }

    fn allocate(&self, align: usize, size: usize) -> Result<usize, AllocError> {
        // TODO 现在只支持单点分配，因此 align 和 size 必须都为 1
        if !align.is_power_of_two() || size != 1 || align != 1 {
            return Err(AllocError::Unsupported);
        }
        // 组数
        let n = self.bits.len() as u32;
        // 暂存一个提示序号
        let mut hint = self.hint.load(Relaxed);
        // 重试次数
        let mut retry = 0;
        while retry < n {
            // 假设提示指示的组 0 号位置空闲
            let mut j = 0u32;
            while j < 32 {
                // 尝试占用位置
                let last = self.bits[hint as usize].fetch_and(!(1 << j), SeqCst);
                // 占用成功
                if last & (1 << j) != 0 {
                    // 如果还有空位，修改分配提示
                    if last & !(1 << j) != 0 {
                        self.hint.store(hint, Relaxed);
                    }
                    return Ok((hint * 32 + j) as _);
                }
                // 占用失败，再找一个空位
                j = last.trailing_ones();
            }
            // 这一组已满，更新提示
            let new_hint = self.hint.load(Relaxed);
            if new_hint == hint {
                // 如果提示未被更新，消耗一次重试机会，尝试下一个位置
                hint = (hint + 1) % n;
                retry += 1;
            } else {
                // 否则尝试新的提示，这次检查不算
                hint = new_hint;
            }
        }
        Err(AllocError::Failed)
    }

    #[inline]
    fn deallocate(&self, base: usize, size: usize) {
        for idx in base..base + size {
            self.bits[idx / 32].fetch_or(1 << (idx % 32), SeqCst);
            self.hint.store(idx as u32 / 32, Relaxed);
        }
    }
}
