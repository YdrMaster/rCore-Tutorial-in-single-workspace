use crate::FrameAllocator;
use core::ptr::NonNull;
use page_table::{VmMeta, PPN, VPN};

/// 保存待映射帧的队列。
///
/// 是一个双链表。
pub struct FrameQueue<'a, Meta: VmMeta, A: FrameAllocator<Meta>> {
    head: Option<NonNull<FrameNode<Meta>>>,
    tail: Option<NonNull<FrameNode<Meta>>>,
    a: &'a A,
}

struct FrameNode<Meta: VmMeta> {
    next: Option<NonNull<FrameNode<Meta>>>,
    prev: Option<NonNull<FrameNode<Meta>>>,
    info: FrameInfo<Meta>,
}

/// 物理页帧节点。
pub struct FrameInfo<Meta: VmMeta> {
    /// 页帧的物理页号。
    pub ppn: PPN<Meta>,
    /// 分配时预期将映射的虚页号。
    pub vpn: VPN<Meta>,
    /// 分配的页表级别。
    pub level: usize,
    /// 连续页帧数量。
    pub count: usize,
}

impl<Meta: VmMeta> FrameInfo<Meta> {
    #[inline]
    pub unsafe fn deallocate_to<A: FrameAllocator<Meta>>(&self, a: &A) {
        let len = self.count * Meta::pages_in_table(self.level);
        a.deallocate(self.ppn..self.ppn + len);
    }
}

impl<'a, Meta: VmMeta, A: FrameAllocator<Meta>> FrameQueue<'a, Meta, A> {
    /// 创建一个空的物理页帧队列。
    #[inline]
    pub const fn new(a: &'a A) -> Self {
        Self {
            head: None,
            tail: None,
            a,
        }
    }

    // /// 分配一个页，建立中间页表用。
    // #[inline]
    // pub(crate) fn allocate_one(&self) -> Result<PPN<Meta>, AllocError> {
    //     self.a.allocate_one()
    // }

    /// 将一组页面压入队列。
    ///
    /// 相当于将这组物理页帧的所有权传入队列。
    pub unsafe fn push(&mut self, info: FrameInfo<Meta>, p_to_v: impl Fn(PPN<Meta>) -> VPN<Meta>) {
        let node = &mut *p_to_v(info.ppn).base().as_mut_ptr();
        *node = FrameNode {
            next: None,
            prev: self.tail,
            info,
        };
        let node = Some(NonNull::new_unchecked(node));
        match self.tail {
            Some(mut tail) => tail.as_mut().next = node,
            None => self.head = node,
        }
        self.tail = node;
    }

    /// 将一组页面弹出队列。
    ///
    /// 这组物理页帧的所有权也弹出了队列。
    pub unsafe fn pop(&mut self) -> Option<FrameInfo<Meta>> {
        self.head.map(|mut node| {
            let node = core::mem::replace(
                node.as_mut(),
                FrameNode {
                    next: None,
                    prev: None,
                    info: FrameInfo {
                        ppn: PPN::new(0),
                        vpn: VPN::new(0),
                        level: 0,
                        count: 0,
                    },
                },
            );

            self.head = node.next;
            match self.head {
                Some(mut head) => head.as_mut().prev = None,
                None => self.tail = None,
            }

            node.info
        })
    }
}

impl<Meta: VmMeta> Drop for FrameInfo<Meta> {
    #[inline]
    fn drop(&mut self) {
        if self.count != 0 {
            panic!("dropping non-empty frame info");
        }
    }
}

impl<Meta: VmMeta, A: FrameAllocator<Meta>> Drop for FrameQueue<'_, Meta, A> {
    /// 如果队列还没用完就被释放，其中储存的物理页帧也被释放。
    fn drop(&mut self) {
        unsafe {
            while let Some(node) = self.pop() {
                node.deallocate_to(self.a);
            }
        }
    }
}
