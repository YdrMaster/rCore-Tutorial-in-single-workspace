mod mapper;
mod visitor;

use crate::{ForeignPtr, ALLOC};
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
    root: Page<Meta>,
}

impl<Meta: VmMeta> AddressSpace<Meta> {
    /// 创建新地址空间。
    ///
    /// 此时还没有根页表。
    #[inline]
    pub fn new(v_offset: usize) -> Self {
        Self {
            vpn_offset: v_offset,
            root: Page::allocate(),
        }
    }

    #[inline]
    pub fn push(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        let count = range.end.val() - range.start.val();
        self.shuttle().walk_mut(&mut Mapper {
            vpn_offset: self.vpn_offset,
            vbase: range.start,
            prange: pbase..pbase + count,
            flags,
        });
    }

    #[inline]
    pub fn root_ppn(&self) -> PPN<Meta> {
        PPN::new(self.root.vpn().val() - self.vpn_offset)
    }

    #[inline]
    pub fn translate(&self, addr: VAddr<Meta>) -> Option<ForeignPtr<Meta>> {
        const MASK: usize = (1 << 12) - 1;
        let mut visitor = Visitor::new(addr.floor());
        self.shuttle().walk(&mut visitor);
        visitor.ans().map(|pte| ForeignPtr {
            raw: VPN::<Meta>::new(pte.ppn().val() + self.vpn_offset).base() + (addr.val() & MASK),
            flags: pte.flags(),
        })
    }

    #[inline]
    fn shuttle(&self) -> PageTableShuttle<Meta, impl Fn(PPN<Meta>) -> VPN<Meta>> {
        let root = self.root.vpn().base().val();
        let offset = self.vpn_offset;
        PageTableShuttle {
            table: unsafe { PageTable::from_raw_parts(root as _, VPN::new(0), Meta::MAX_LEVEL) },
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

struct Page<Meta: VmMeta>(NonNull<u8>, PhantomData<Meta>);

impl<Meta: VmMeta> Page<Meta> {
    #[inline]
    fn allocate() -> Self {
        Self(ALLOC.get().unwrap().allocate(Meta::PAGE_BITS), PhantomData)
    }

    #[inline]
    fn vpn(&self) -> VPN<Meta> {
        VAddr::new(self.0.as_ptr() as _).floor()
    }
}
