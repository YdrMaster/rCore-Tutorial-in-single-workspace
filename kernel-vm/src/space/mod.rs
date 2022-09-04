mod mapper;
mod visitor;

use crate::ALLOC;
use core::{fmt, marker::PhantomData, ops::Range, ptr::NonNull};
use mapper::Mapper;
use page_table::{PageTable, PageTableShuttle, VAddr, VmFlags, VmMeta, PPN, VPN};
use visitor::Visitor;

/// 地址空间。
pub struct AddressSpace<Meta: VmMeta> {
    /// 所在地址空间和物理地址空间之间的页号偏移。
    ///
    /// 地址空间对象本身必须位于一个固定的地址空间中（内核地址空间），这样才能正常使用指针。
    ///
    /// 设定这个地址空间是线性地址空间，与物理地址空间只有一个整页的偏移。
    vpn_offset: usize,
    /// 根页表。
    root: NonNull<u8>,
    _phantom: PhantomData<Meta>,
}

impl<Meta: VmMeta> AddressSpace<Meta> {
    /// 创建新地址空间。
    #[inline]
    pub fn new(v_offset: usize) -> Self {
        let root = unsafe {
            ALLOC
                .get()
                .expect("allocator uninitialized for kernel-vm")
                .create(Meta::PAGE_BITS)
        };
        Self {
            vpn_offset: v_offset,
            root,
            _phantom: PhantomData,
        }
    }

    /// 向地址空间增加映射关系。
    #[inline]
    pub fn push(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        let count = range.end.val() - range.start.val();
        self.shuttle().walk_mut(&mut Mapper {
            space: self,
            vbase: range.start,
            prange: pbase..pbase + count,
            flags,
        });
    }

    /// 地址空间根页表的物理页号。
    #[inline]
    pub fn root_ppn(&self) -> PPN<Meta> {
        PPN::new((self.root.as_ptr() as usize >> Meta::PAGE_BITS) - self.vpn_offset)
    }

    /// 检查 `flags` 的属性呢要求，然后将地址空间中的一个虚地址翻译成当前地址空间中的指针。
    #[inline]
    pub fn translate<T>(&self, addr: VAddr<Meta>, flags: VmFlags<Meta>) -> Option<NonNull<T>> {
        let mut visitor = Visitor::new(addr.floor());
        self.shuttle().walk(&mut visitor);
        visitor
            .ans()
            .filter(|pte| pte.flags().0 & flags.0 == flags.0)
            .map(|pte| unsafe {
                let vpn = VPN::<Meta>::new(pte.ppn().val() + self.vpn_offset);
                NonNull::new_unchecked((vpn.base().val() + addr.offset()) as _)
            })
    }

    /// 这个地址空间的页表穿梭机。
    #[inline]
    fn shuttle(&self) -> PageTableShuttle<Meta, impl Fn(PPN<Meta>) -> VPN<Meta>> {
        let offset = self.vpn_offset;
        PageTableShuttle {
            table: unsafe {
                PageTable::from_raw_parts(self.root.cast().as_ptr(), VPN::new(0), Meta::MAX_LEVEL)
            },
            f: move |p| VPN::new(p.val() + offset),
        }
    }
}

impl<Meta: VmMeta> fmt::Debug for AddressSpace<Meta> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "root: {:#x}", self.root_ppn().val())?;
        write!(f, "{:?}", self.shuttle())
    }
}
