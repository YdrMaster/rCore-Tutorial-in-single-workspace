//! 内核虚存管理。

#![no_std]
#![deny(warnings, missing_docs)]

mod counter;

pub use counter::PageCounter;

#[macro_use]
extern crate alloc;

use alloc::{
    alloc::{alloc, dealloc, Layout},
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{marker::PhantomData, ops::Range, ptr::NonNull};
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

/// 从全局分配器分配的物理页。
#[repr(transparent)]
pub struct GlobalPage<Meta: VmMeta>(NonNull<u8>, PhantomData<Meta>);

impl<Meta: VmMeta> GlobalPage<Meta> {
    /// 分配物理页。
    #[inline]
    pub fn new() -> Self {
        Self(
            NonNull::new(unsafe { alloc(Self::layout()) }).unwrap(),
            PhantomData,
        )
    }

    #[inline]
    unsafe fn from_raw_parts(ptr: NonNull<u8>) -> Self {
        Self(ptr, PhantomData)
    }

    /// 页存储布局。
    #[inline]
    const fn layout() -> Layout {
        let size = 1 << Meta::PAGE_BITS;
        unsafe { Layout::from_size_align_unchecked(size, size) }
    }

    /// 页在当前地址空间中的虚页号。
    #[inline]
    fn current_vpn(&self) -> VPN<Meta> {
        VAddr::new(self.0.as_ptr() as _).floor()
    }
}

impl<Meta: VmMeta> Drop for GlobalPage<Meta> {
    fn drop(&mut self) {
        unsafe { dealloc(self.0.as_ptr(), Self::layout()) }
    }
}

/// 页存根。
pub struct PageStub<Meta: VmMeta> {
    page: GlobalPage<Meta>,
    _parent: Option<Arc<PageStub<Meta>>>,
}

/// 用户程序地址空间。
pub struct AppSpace<Meta: VmMeta> {
    root: Arc<PageStub<Meta>>,
    tables: Vec<BTreeMap<VPN<Meta>, Weak<PageStub<Meta>>>>,
    pages: BTreeMap<VPN<Meta>, PageStub<Meta>>,
}

impl<Meta: VmMeta> AppSpace<Meta> {
    /// 构造用户地址空间。
    pub fn new() -> Self {
        Self {
            root: Arc::new(PageStub {
                page: GlobalPage::new(),
                _parent: None,
            }),
            tables: vec![BTreeMap::new(); Meta::MAX_LEVEL],
            pages: BTreeMap::new(),
        }
    }

    /// 获取 `pos` 处的页表项上一级页表的引用计数。
    fn get_parent(&self, pos: Pos<Meta>) -> Arc<PageStub<Meta>> {
        self.tables
            .get(pos.level - 1)
            .map_or(self.root.clone(), |map| {
                map.get(&pos.vpn.floor(pos.level))
                    .unwrap()
                    .upgrade()
                    .unwrap()
            })
    }

    /// 建立映射关系。
    pub fn map<F: Fn(PPN<Meta>) -> VPN<Meta>, G: Fn(VPN<Meta>) -> PPN<Meta>>(
        &mut self,
        args: MapArgs<Meta>,
        transform: Transform<Meta, F, G>,
    ) {
        let Transform { f, g, _phantom } = transform;
        let mut shuttle = PageTableShuttle {
            table: unsafe {
                PageTable::<Meta>::from_raw_parts(
                    self.root.page.0.as_ptr().cast(),
                    VPN::new(0),
                    Meta::MAX_LEVEL,
                )
            },
            f,
        };
        shuttle.walk_mut(Mapper {
            space: self,
            f: g,
            args,
            temp: None,
        })
    }
}

/// 映射参数。
pub struct MapArgs<Meta: VmMeta> {
    /// **用户地址空间**待映射的范围。
    range: Range<VPN<Meta>>,
    /// 待映射的物理页。
    pages: NonNull<u8>,
    /// 页表的 flags。
    table_flags: VmFlags<Meta>,
    /// 页的 flags。
    flags: VmFlags<Meta>,
}

/// 当前地址空间与物理地址空间的双向变换。
pub struct Transform<Meta: VmMeta, F: Fn(PPN<Meta>) -> VPN<Meta>, G: Fn(VPN<Meta>) -> PPN<Meta>> {
    /// p -> v
    pub f: F,
    /// v -> p
    pub g: G,
    _phantom: PhantomData<Meta>,
}

impl<Meta: VmMeta> MapArgs<Meta> {
    fn move_next(&mut self) -> bool {
        self.range.start += 1;
        self.pages =
            unsafe { NonNull::new_unchecked(self.pages.as_ptr().offset(1 << Meta::PAGE_BITS)) };
        !self.range.is_empty()
    }
}

struct Mapper<'a, Meta: VmMeta, F: Fn(VPN<Meta>) -> PPN<Meta>> {
    /// 用户地址空间。
    space: &'a mut AppSpace<Meta>,
    /// **当前地址空间**虚页号到物理页号的映射关系。
    f: F,
    /// 映射配置。
    args: MapArgs<Meta>,
    /// 暂存一个页表页的引用计数。
    temp: Option<Arc<PageStub<Meta>>>,
}

impl<'a, Meta: VmMeta, F: Fn(VPN<Meta>) -> PPN<Meta>> Decorator<Meta> for Mapper<'a, Meta, F> {
    #[inline]
    fn start(&mut self, _pos: Pos<Meta>) -> Pos<Meta> {
        Pos::new(self.args.range.start, 0)
    }

    fn arrive(&mut self, pte: &mut Pte<Meta>, target_hint: Pos<Meta>) -> Pos<Meta> {
        assert!(!pte.is_valid());
        // 构造页存根
        let stub = PageStub {
            page: unsafe { GlobalPage::from_raw_parts(self.args.pages) },
            _parent: Some(self.space.get_parent(target_hint)),
        };
        // 建立映射关系
        *pte = self.args.flags.build_pte((self.f)(stub.page.current_vpn()));
        // 保存到地址空间
        self.space.pages.insert(target_hint.vpn, stub);
        // 移动到下一页
        if self.args.move_next() {
            target_hint.next()
        } else {
            Pos::stop()
        }
    }

    fn meet(&mut self, level: usize, pte: Pte<Meta>, target_hint: Pos<Meta>) -> Update<Meta> {
        assert!(!pte.is_valid());
        // 构造页存根
        let stub = Arc::new(PageStub {
            page: GlobalPage::<Meta>::new(),
            _parent: Some(self.space.get_parent(Pos::new(target_hint.vpn, level))),
        });
        // 当前地址空间中的页表的虚页号
        let vpn = stub.page.current_vpn();
        // 弱引用保存
        self.space.tables[level].insert(target_hint.vpn.floor(level), Arc::downgrade(&stub));
        // 卡住，以免直接释放掉
        self.temp = Some(stub);
        Update::Pte(self.args.table_flags.build_pte((self.f)(vpn)), vpn)
    }
}
