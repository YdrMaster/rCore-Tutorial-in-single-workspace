mod mapper;
mod segment;

pub use segment::Segment;

use crate::ALLOC;
use alloc::{
    collections::{BTreeMap, BinaryHeap},
    vec::Vec,
};
use core::{marker::PhantomData, ops::Range, ptr::NonNull};
use mapper::Mapper;
use page_table::{PageTable, PageTableShuttle, VAddr, VmFlags, VmMeta, PPN, VPN};

/// 地址空间。
pub struct AddressSpace<Meta: VmMeta> {
    /// 所在地址空间和物理地址空间之间的页号偏移。
    vpn_offset: usize,
    /// 页表信息记录。
    tables: Vec<BTreeMap<usize, Table<Meta>>>,
    /// 数据页信息记录。
    pages: BinaryHeap<Segment<Meta>>,
}

impl<Meta: VmMeta> AddressSpace<Meta> {
    #[inline]
    pub fn new(v_offset: usize) -> Self {
        Self {
            vpn_offset: v_offset,
            tables: Vec::from_iter((0..=Meta::MAX_LEVEL).map(|_| BTreeMap::new())),
            pages: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        let root = self.tables[Meta::MAX_LEVEL]
            .entry(0)
            .or_insert_with(|| Table(Page::allocate(), 0))
            .0
            .vpn()
            .base()
            .val();
        let offset = self.vpn_offset;
        let count = range.end.val() - range.start.val();
        PageTableShuttle {
            table: unsafe { PageTable::from_raw_parts(root as _, VPN::new(0), Meta::MAX_LEVEL) },
            f: |p| VPN::new(p.val() + offset),
        }
        .walk_mut(Mapper {
            space: self,
            vbase: range.start,
            prange: pbase..pbase + count,
            flags,
        });
        self.pages.push(Segment {
            vbase: range.start,
            pbase,
            count,
            flags,
        });
        for i in range.start.val()..range.end.val() {
            let mut bits = 0;
            for (level, tables) in self.tables.iter_mut().enumerate() {
                bits += Meta::LEVEL_BITS[level];
                tables.get_mut(&(i >> bits)).unwrap().1 += 1;
            }
        }
    }

    pub fn page_count(&self) -> usize {
        self.tables[Meta::MAX_LEVEL]
            .get(&0)
            .map_or(0, |table| table.1)
    }

    pub fn root_ppn(&self) -> Option<PPN<Meta>> {
        Some(PPN::new(
            self.tables[Meta::MAX_LEVEL].get(&0)?.0.vpn().val() - self.vpn_offset,
        ))
    }

    pub fn shuttle(&self) -> Option<PageTableShuttle<Meta, impl Fn(PPN<Meta>) -> VPN<Meta>>> {
        let root = self.tables[Meta::MAX_LEVEL].get(&0)?.0.vpn().base().val();
        let offset = self.vpn_offset;
        Some(PageTableShuttle {
            table: unsafe { PageTable::from_raw_parts(root as _, VPN::new(0), Meta::MAX_LEVEL) },
            f: move |p| VPN::new(p.val() + offset),
        })
    }

    pub fn segments(&self) -> Vec<Segment<Meta>> {
        self.pages.iter().cloned().rev().collect()
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

impl<Meta: VmMeta> Drop for Page<Meta> {
    #[inline]
    fn drop(&mut self) {
        unsafe { ALLOC.get_unchecked().deallocate(self.0, Meta::PAGE_BITS) };
    }
}

struct Table<Meta: VmMeta>(Page<Meta>, usize);
