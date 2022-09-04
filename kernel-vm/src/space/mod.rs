mod mapper;
mod visitor;

use crate::PageManager;
use core::{fmt, ops::Range, ptr::NonNull};
use mapper::Mapper;
use page_table::{PageTable, PageTableFormatter, Pos, Pte, VAddr, VmFlags, VmMeta, PPN, VPN};
use visitor::Visitor;

/// 地址空间。
pub struct AddressSpace<Meta: VmMeta, P: PageManager<Meta>> {
    manager: P,
    root: NonNull<Pte<Meta>>,
}

impl<Meta: VmMeta, M: PageManager<Meta>> AddressSpace<Meta, M> {
    /// 创建新地址空间。
    #[inline]
    pub fn new() -> Self {
        let mut manager = M::default();
        let mut flags = VmFlags::VALID;
        let root = manager.allocate(1, &mut flags).cast();
        Self { manager, root }
    }

    /// 向地址空间增加映射关系。
    #[inline]
    pub fn push(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        let count = range.end.val() - range.start.val();
        unsafe { PageTable::from_root(self.root) }.walk_mut(
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
        self.manager.v_to_p(self.root)
    }

    /// 检查 `flags` 的属性呢要求，然后将地址空间中的一个虚地址翻译成当前地址空间中的指针。
    #[inline]
    pub fn translate<T>(&self, addr: VAddr<Meta>, flags: VmFlags<Meta>) -> Option<NonNull<T>> {
        let mut visitor = Visitor::new(self);
        unsafe { PageTable::from_root(self.root) }.walk(Pos::new(addr.floor(), 0), &mut visitor);
        visitor
            .ans()
            .filter(|pte| pte.flags().contains(flags))
            .map(|pte| unsafe {
                NonNull::new_unchecked(
                    self.manager
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
                pt: unsafe { PageTable::from_root(self.root) },
                f: |ppn| unsafe {
                    NonNull::new_unchecked(VPN::<Meta>::new(ppn.val()).base().as_mut_ptr())
                }
            }
        )
    }
}
