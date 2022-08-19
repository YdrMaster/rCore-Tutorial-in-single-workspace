use crate::{AllocError, FrameAllocator};
use core::{marker::PhantomData, mem::MaybeUninit};
use page_table::{MaybeInvalidPPN, VmMeta, PPN, VPN};

/// 直接架设在物理页上的双端队列。
///
/// 原理类似 c++ 的 `std::deque`，但每个分片都是一个物理页。
/// 这个数据结构可以用于只有物理页帧分配器的情况。
pub struct Deque<T: 'static, Meta: VmMeta, A: FrameAllocator<Meta>, F: Fn(PPN<Meta>) -> VPN<Meta>> {
    // 第一页的物理页号。
    head_page: MaybeInvalidPPN<Meta>,
    // 最后一页的物理页号。
    tail_page: MaybeInvalidPPN<Meta>,
    /// 第一页的起始序号。
    head_idx: usize,
    /// 最后一页的结束序号。
    tail_idx: usize,
    /// 链表中的页数。
    page_count: usize,
    a: A,
    f: F,
    _phantom: PhantomData<T>,
}

/// 计算每个物理页能容纳元素的数量。
///
/// 从物理页头上去掉链表两个指针的空间再对齐到元素要求的位置，然后连续放置。
const fn page_cap<Meta: VmMeta, T>() -> usize {
    use core::mem::{align_of, size_of};
    let align_mask = align_of::<T>() - 1;
    // 对齐两个指针
    let base = (2 * size_of::<MaybeInvalidPPN<Meta>>() + align_mask) & !align_mask;
    // 对齐每个元素
    let each = (size_of::<T>() + align_mask) & !align_mask;
    // 除！
    ((1 << Meta::PAGE_BITS) - base) / each
}

impl<T: 'static, Meta: VmMeta, A: FrameAllocator<Meta>, F: Fn(PPN<Meta>) -> VPN<Meta>>
    Deque<T, Meta, A, F>
{
    /// 每个物理页容纳的元素数。
    const PAGE_CAP: usize = page_cap::<Meta, T>();

    /// 创建一个空的页上双端队列。
    #[inline]
    pub const fn new(a: A, f: F) -> Self {
        Self {
            head_page: MaybeInvalidPPN::invalid(),
            tail_page: MaybeInvalidPPN::invalid(),
            head_idx: 0,
            tail_idx: Self::PAGE_CAP,
            page_count: 0,
            a,
            f,
            _phantom: PhantomData,
        }
    }

    /// 如果队列为空，则返回 `true`。
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.page_count == 0
    }

    /// 返回队列中的元素数量。
    #[inline]
    pub fn len(&self) -> usize {
        match self.page_count {
            0 => 0,
            1 => self.tail_idx - self.head_idx,
            n => (n - 1) * Self::PAGE_CAP + self.tail_idx - self.head_idx,
        }
    }

    /// 压入一个元素 `val` 到队尾，可能会分配一个页。
    pub fn push_back(&mut self, val: T) -> Result<(), AllocError> {
        let tail = self.tail();
        // 最后一页已满则分配一个新页，否则直接压到最后一页
        if self.tail_idx == Self::PAGE_CAP {
            // 分配一页
            let page = self.a.allocate_one()?;
            // 在页上写链表节点信息
            let node = unsafe { Node::from_ppn(page, &self.f) };
            node.next = MaybeInvalidPPN::invalid();
            node.prev = self.tail_page;
            node.items[0] = MaybeUninit::new(val);
            // 更新链表元数据
            let new_tail = MaybeInvalidPPN::new(page);
            match tail {
                Some(tail) => tail.next = new_tail,
                None => self.head_page = new_tail,
            }
            self.tail_page = new_tail;
            self.tail_idx = 1;
            self.page_count += 1;
        } else {
            tail.unwrap().items[self.tail_idx] = MaybeUninit::new(val);
            self.tail_idx += 1;
        }
        Ok(())
    }

    /// 压入一个元素 `val` 到队首，可能会分配一个页。
    pub fn push_front(&mut self, val: T) -> Result<(), AllocError> {
        let head = self.head();
        // 第一页已满则分配一个新页，否则直接压到第一页
        if self.head_idx == 0 {
            // 分配一页
            let page = self.a.allocate_one()?;
            // 在页上写链表节点信息
            let node = unsafe { Node::from_ppn(page, &self.f) };
            let idx = node.items.len() - 1;
            node.next = self.head_page;
            node.prev = MaybeInvalidPPN::invalid();
            node.items[idx] = MaybeUninit::new(val);
            // 更新链表元数据
            let new_head = MaybeInvalidPPN::new(page);
            match head {
                Some(head) => head.prev = new_head,
                None => self.tail_page = new_head,
            }
            self.head_page = new_head;
            self.head_idx = idx;
            self.page_count += 1;
        } else {
            self.head_idx -= 1;
            head.unwrap().items[self.head_idx] = MaybeUninit::new(val);
        }
        Ok(())
    }

    /// 从队尾排出一个元素，如果清空了最后一页，则释放该页。
    pub fn pop_back(&mut self) -> Option<T> {
        // 如果没有页肯定没有元素
        if self.page_count == 0 {
            return None;
        }
        // 打开第一页
        let tail = self.tail().unwrap();
        // 换出元素
        self.tail_idx -= 1;
        let val = unsafe { tail.take(self.tail_idx) };
        // 如果最后一页已空，则释放该页
        if self.tail_idx == 0 || (self.page_count == 1 && self.head_idx == self.tail_idx) {
            self.deallocate_tail(tail);
            match self.tail() {
                Some(mut tail) => tail.next = MaybeInvalidPPN::invalid(),
                None => {
                    self.head_idx = 0;
                    self.head_page = MaybeInvalidPPN::invalid()
                }
            }
        }
        Some(val)
    }

    /// 从队首排出一个元素，如果清空了第一页，则释放该页。
    pub fn pop_front(&mut self) -> Option<T> {
        // 如果没有页肯定没有元素
        if self.page_count == 0 {
            return None;
        }
        // 打开第一页
        let head = self.head().unwrap();
        // 换出元素
        let val = unsafe { head.take(self.head_idx) };
        self.head_idx += 1;
        // 如果第一页已空，则释放该页
        if self.head_idx == Self::PAGE_CAP
            || (self.page_count == 1 && self.head_idx == self.tail_idx)
        {
            self.deallocate_head(head);
            match self.head() {
                Some(mut head) => head.prev = MaybeInvalidPPN::invalid(),
                None => {
                    self.tail_idx = Self::PAGE_CAP;
                    self.tail_page = MaybeInvalidPPN::invalid()
                }
            }
        }
        Some(val)
    }

    #[inline]
    fn head(&mut self) -> Option<&'static mut Node<Meta, T>> {
        unsafe { Node::try_from_ppn(self.head_page, &self.f) }
    }

    #[inline]
    fn tail(&mut self) -> Option<&'static mut Node<Meta, T>> {
        unsafe { Node::try_from_ppn(self.tail_page, &self.f) }
    }

    /// 释放第一页。
    #[inline]
    fn deallocate_head(&mut self, head: &mut Node<Meta, T>) {
        // 第一页的物理页号
        let old = core::mem::replace(&mut self.head_page, head.next)
            .get()
            .unwrap();
        // 擦除页上数据并释放页
        unsafe {
            head.zero();
            self.a.deallocate(old..old + 1);
        }
        self.head_idx = 0;
        self.page_count -= 1;
    }

    /// 释放最后一页。
    #[inline]
    fn deallocate_tail(&mut self, tail: &mut Node<Meta, T>) {
        // 最后一页的物理页号
        let old = core::mem::replace(&mut self.tail_page, tail.prev)
            .get()
            .unwrap();
        // 擦除页上数据并释放页
        unsafe {
            tail.zero();
            self.a.deallocate(old..old + 1);
        }
        // 修改链表元数据
        self.tail_idx = Self::PAGE_CAP;
        self.page_count -= 1;
    }
}

impl<T: 'static, Meta: VmMeta, A: FrameAllocator<Meta>, F: Fn(PPN<Meta>) -> VPN<Meta>> Drop
    for Deque<T, Meta, A, F>
{
    fn drop(&mut self) {
        // 释放前面的页
        while self.page_count > 1 {
            let head = self.head().unwrap();
            for i in self.head_idx..Self::PAGE_CAP {
                unsafe { head.items[i].assume_init_drop() };
            }
            self.deallocate_head(head);
        }
        // 释放最后一页
        if self.page_count == 1 {
            let tail = self.tail().unwrap();
            for i in self.head_idx..self.tail_idx {
                unsafe { tail.items[i].assume_init_drop() };
            }
            self.deallocate_tail(tail);
        }
    }
}

/// 双端队列的页节点映射。
#[repr(C)]
struct Node<Meta: VmMeta, T> {
    next: MaybeInvalidPPN<Meta>,
    prev: MaybeInvalidPPN<Meta>,
    items: [MaybeUninit<T>],
}

impl<Meta: VmMeta, T> Node<Meta, T> {
    /// 每个物理页容纳的字节数。
    const PAGE_SIZE: usize = 1 << Meta::PAGE_BITS;

    /// 每个物理页容纳的元素数。
    const PAGE_CAP: usize = page_cap::<Meta, T>();

    /// 翻译物理页号，产生一个节点引用。
    #[inline]
    unsafe fn from_ppn(ppn: PPN<Meta>, f: impl Fn(PPN<Meta>) -> VPN<Meta>) -> &'static mut Self {
        &mut *core::ptr::from_raw_parts_mut(f(ppn).base().as_mut_ptr(), Self::PAGE_CAP)
    }

    /// 如果物理页号有效，则翻译物理页号产生一个节点引用。
    #[inline]
    unsafe fn try_from_ppn(
        ppn: MaybeInvalidPPN<Meta>,
        f: impl Fn(PPN<Meta>) -> VPN<Meta>,
    ) -> Option<&'static mut Self> {
        ppn.get().map(|ppn| Self::from_ppn(ppn, f))
    }

    /// 清零物理页。
    #[inline]
    unsafe fn zero(&mut self) {
        core::ptr::write_bytes(self as *mut _ as *mut u8, 0, Self::PAGE_SIZE);
    }

    /// 从物理页中取出第 `i` 项，留下一个未初始化内存。
    #[inline]
    unsafe fn take(&mut self, i: usize) -> T {
        core::mem::replace(&mut self.items[i], MaybeUninit::uninit()).assume_init()
    }
}
