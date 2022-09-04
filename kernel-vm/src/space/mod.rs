mod mapper;
mod visitor;

use crate::PageManager;
use core::{fmt, marker::PhantomData, ops::Range, ptr::NonNull};
use mapper::Mapper;
use page_table::{PageTableFormatter, Pos, VAddr, VmFlags, VmMeta, PPN, VPN};
use visitor::Visitor;

/// 地址空间。
pub struct AddressSpace<Meta: VmMeta, M: PageManager<Meta>>(M, PhantomData<Meta>);

impl<Meta: VmMeta, M: PageManager<Meta>> AddressSpace<Meta, M> {
    /// 创建新地址空间。
    #[inline]
    pub fn new() -> Self {
        Self(M::new_root(), PhantomData)
    }

    /// 向地址空间增加映射关系。
    #[inline]
    pub fn push(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        let count = range.end.val() - range.start.val();
        self.0.root().walk_mut(
            Pos::new(range.start, 0),
            &mut Mapper {
                space: self,
                prange: pbase..pbase + count,
                flags,
            },
        )
    }

    /// 向地址空间增加映射关系。
    #[inline]
    pub fn allocate(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        let count = range.end.val() - range.start.val();
        self.0.root().walk_mut(
            Pos::new(range.start, 0),
            &mut Mapper {
                space: self,
                prange: pbase..pbase + count,
                flags,
            },
        )
    }

    /// 地址空间根页表的物理页号。
    #[inline]
    pub fn root_ppn(&self) -> PPN<Meta> {
        self.0.root_ppn()
    }

    /// 检查 `flags` 的属性要求，然后将地址空间中的一个虚地址翻译成当前地址空间中的指针。
    #[inline]
    pub fn translate<T>(&self, addr: VAddr<Meta>, flags: VmFlags<Meta>) -> Option<NonNull<T>> {
        let mut visitor = Visitor::new(self);
        self.0.root().walk(Pos::new(addr.floor(), 0), &mut visitor);
        visitor
            .ans()
            .filter(|pte| pte.flags().contains(flags))
            .map(|pte| unsafe {
                NonNull::new_unchecked(
                    self.0
                        .p_to_v::<u8>(pte.ppn())
                        .as_ptr()
                        .add(addr.offset())
                        .cast(),
                )
            })
    }
}

impl<Meta: VmMeta, P: PageManager<Meta>> fmt::Debug for AddressSpace<Meta, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "root: {:#x}", self.root_ppn().val())?;
        write!(
            f,
            "{:?}",
            PageTableFormatter {
                pt: self.0.root(),
                f: |ppn| self.0.p_to_v(ppn)
            }
        )
    }
}
