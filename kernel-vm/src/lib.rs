//! 内核虚存管理。

#![no_std]
// #![deny(warnings, missing_docs)]

extern crate alloc;

use alloc::{
    collections::{BTreeMap, BinaryHeap},
    vec::Vec,
};
use core::{alloc::Layout, cmp::Ordering, marker::PhantomData, ops::Range, ptr::NonNull};
use page_table::{
    Decorator, PageTable, PageTableShuttle, Pos, Pte, Update, VAddr, VmFlags, VmMeta, PPN, VPN,
};

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

pub trait PageAllocator: Sync {
    fn allocate(&self, bits: usize) -> NonNull<u8>;

    fn deallocate(&self, ptr: NonNull<u8>, bits: usize);
}

static ALLOC: spin::Once<&'static dyn PageAllocator> = spin::Once::new();

pub fn init_allocator(a: &'static dyn PageAllocator) {
    ALLOC.call_once(|| a);
}

pub struct Page<Meta: VmMeta>(NonNull<u8>, PhantomData<Meta>);

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
            pbase,
            pend: pbase + count,
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
        self.pages.iter().cloned().collect()
    }
}

struct Table<Meta: VmMeta>(Page<Meta>, usize);

#[derive(Clone, Debug)]
pub struct Segment<Meta: VmMeta> {
    vbase: VPN<Meta>,
    pbase: PPN<Meta>,
    count: usize,
    flags: VmFlags<Meta>,
}

impl<Meta: VmMeta> PartialOrd for Segment<Meta> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.vbase.partial_cmp(&other.vbase)
    }
}

impl<Meta: VmMeta> Ord for Segment<Meta> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.vbase.cmp(&other.vbase)
    }
}

impl<Meta: VmMeta> PartialEq for Segment<Meta> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.vbase.eq(&other.vbase)
    }
}

impl<Meta: VmMeta> Eq for Segment<Meta> {}

struct Mapper<'a, Meta: VmMeta> {
    space: &'a mut AddressSpace<Meta>,
    vbase: VPN<Meta>,
    pbase: PPN<Meta>,
    pend: PPN<Meta>,
    flags: VmFlags<Meta>,
}

impl<'a, Meta: VmMeta> Decorator<Meta> for Mapper<'a, Meta> {
    fn start(&mut self, _pos: Pos<Meta>) -> Pos<Meta> {
        Pos {
            vpn: self.vbase,
            level: 0,
        }
    }

    fn arrive(&mut self, pte: &mut Pte<Meta>, target_hint: Pos<Meta>) -> Pos<Meta> {
        assert!(!pte.is_valid());
        *pte = self.flags.build_pte(self.pbase);
        self.pbase += 1;
        if self.pbase == self.pend {
            Pos::stop()
        } else {
            target_hint.next()
        }
    }

    fn meet(&mut self, level: usize, pte: Pte<Meta>, target_hint: Pos<Meta>) -> Update<Meta> {
        assert!(!pte.is_valid());
        let page = Page::<Meta>::allocate();
        let vpn = page.vpn();
        let ppn = PPN::new(vpn.val() - self.space.vpn_offset);
        self.space.tables[level - 1].insert(
            target_hint.vpn.val() >> Meta::LEVEL_BITS[..level].iter().sum::<usize>(),
            Table(page, 0),
        );
        Update::Pte(unsafe { VmFlags::from_raw(1) }.build_pte(ppn), vpn)
    }
}
