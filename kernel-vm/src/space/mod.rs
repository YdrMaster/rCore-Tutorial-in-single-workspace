mod mapper;
mod visitor;

extern crate alloc;

use crate::PageManager;
use alloc::vec::Vec;
use core::{fmt, marker::PhantomData, ops::Range, ptr::NonNull};
use mapper::Mapper;
use page_table::{PageTable, PageTableFormatter, Pos, VAddr, VmFlags, VmMeta, PPN, VPN};
use visitor::Visitor;

/// 地址空间。
pub struct AddressSpace<Meta: VmMeta, M: PageManager<Meta>> {
    /// 虚拟地址块
    pub areas: Vec<Range<VPN<Meta>>>,
    page_manager: M,
    phantom_data: PhantomData<Meta>,
    /// 异界传送门的属性
    pub tramp: (PPN<Meta>, VmFlags<Meta>),
}

impl<Meta: VmMeta, M: PageManager<Meta>> AddressSpace<Meta, M> {
    /// 创建新地址空间。
    #[inline]
    pub fn new() -> Self {
        Self {
            areas: Vec::new(),
            page_manager: M::new_root(),
            phantom_data: PhantomData,
            tramp: (PPN::INVALID, VmFlags::ZERO),
        }
    }

    /// 地址空间根页表的物理页号。
    #[inline]
    pub fn root_ppn(&self) -> PPN<Meta> {
        self.page_manager.root_ppn()
    }

    /// 地址空间根页表
    #[inline]
    pub fn root(&self) -> PageTable<Meta> {
        unsafe { PageTable::from_root(self.page_manager.root_ptr()) }
    }

    /// 向地址空间增加异界传送门映射关系。
    pub fn map_portal(&mut self, tramp: (PPN<Meta>, VmFlags<Meta>)) {
        self.tramp = tramp;
        let vpn = VPN::MAX;
        let mut root = self.root();
        let mut mapper = Mapper::new(self, tramp.0..tramp.0 + 1, tramp.1);
        root.walk_mut(Pos::new(vpn, 0), &mut mapper);
    }

    /// 向地址空间增加映射关系。
    pub fn map_extern(&mut self, range: Range<VPN<Meta>>, pbase: PPN<Meta>, flags: VmFlags<Meta>) {
        self.areas.push(range.start..range.end);
        let count = range.end.val() - range.start.val();
        let mut root = self.root();
        let mut mapper = Mapper::new(self, pbase..pbase + count, flags);
        root.walk_mut(Pos::new(range.start, 0), &mut mapper);
        if !mapper.ans() {
            // 映射失败，需要回滚吗？
            todo!()
        }
    }

    /// 分配新的物理页，拷贝数据并建立映射。
    pub fn map(
        &mut self,
        range: Range<VPN<Meta>>,
        data: &[u8],
        offset: usize,
        mut flags: VmFlags<Meta>,
    ) {
        let count = range.end.val() - range.start.val();
        let size = count << Meta::PAGE_BITS;
        assert!(size >= data.len() + offset);
        let page = self.page_manager.allocate(count, &mut flags);
        unsafe {
            use core::slice::from_raw_parts_mut as slice;
            let mut ptr = page.as_ptr();
            slice(ptr, offset).fill(0);
            ptr = ptr.add(offset);
            slice(ptr, data.len()).copy_from_slice(data);
            ptr = ptr.add(data.len());
            slice(ptr, page.as_ptr().add(size).offset_from(ptr) as _).fill(0);
        }
        self.map_extern(range, self.page_manager.v_to_p(page), flags)
    }

    /// 检查 `flags` 的属性要求，然后将地址空间中的一个虚地址翻译成当前地址空间中的指针。
    pub fn translate<T>(&self, addr: VAddr<Meta>, flags: VmFlags<Meta>) -> Option<NonNull<T>> {
        let mut visitor = Visitor::new(self);
        self.root().walk(Pos::new(addr.floor(), 0), &mut visitor);
        visitor
            .ans()
            .filter(|pte| pte.flags().contains(flags))
            .map(|pte| unsafe {
                NonNull::new_unchecked(
                    self.page_manager
                        .p_to_v::<u8>(pte.ppn())
                        .as_ptr()
                        .add(addr.offset())
                        .cast(),
                )
            })
    }

    /// 遍历地址空间，将其中的地址映射添加进自己的地址空间中，重新分配物理页并拷贝所有数据及代码
    pub fn cloneself(&self, new_addrspace: &mut AddressSpace<Meta, M>) {
        let root = self.root();
        let areas = &self.areas;
        for (_, range) in areas.iter().enumerate() {
            let mut visitor = Visitor::new(self);
            // 虚拟地址块的首地址的 vpn
            let vpn = range.start;
            // 利用 visitor 访问页表，并获取这个虚拟地址块的页属性
            root.walk(Pos::new(vpn, 0), &mut visitor);
            // 利用 visitor 获取这个虚拟地址块的页属性，以及起始地址
            let (mut flags, mut data_ptr) = visitor
                .ans()
                .filter(|pte| pte.is_valid())
                .map(|pte| {
                    (pte.flags(), unsafe {
                        NonNull::new_unchecked(self.page_manager.p_to_v::<u8>(pte.ppn()).as_ptr())
                    })
                })
                .unwrap();
            let vpn_range = range.start..range.end;
            // 虚拟地址块中页数量
            let count = range.end.val() - range.start.val();
            let size = count << Meta::PAGE_BITS;
            // 分配 count 个 flags 属性的物理页面
            let paddr = new_addrspace.page_manager.allocate(count, &mut flags);
            let ppn = new_addrspace.page_manager.v_to_p(paddr);
            unsafe {
                use core::slice::from_raw_parts_mut as slice;
                let data = slice(data_ptr.as_mut(), size);
                let ptr = paddr.as_ptr();
                slice(ptr, size).copy_from_slice(data);
            }
            new_addrspace.map_extern(vpn_range, ppn, flags);
        }
        let tramp = self.tramp;
        new_addrspace.map_portal(tramp);
    }
}

impl<Meta: VmMeta, P: PageManager<Meta>> fmt::Debug for AddressSpace<Meta, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "root: {:#x}", self.root_ppn().val())?;
        write!(
            f,
            "{:?}",
            PageTableFormatter {
                pt: self.root(),
                f: |ppn| self.page_manager.p_to_v(ppn)
            }
        )
    }
}
