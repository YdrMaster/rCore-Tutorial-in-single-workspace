use crate::{AddressSpace, PageManager};
use core::{ops::Range, ptr::NonNull};
use page_table::{Decorator, Pos, Pte, Update, VmFlags, VmMeta, PPN};

pub(super) struct Mapper<'a, Meta: VmMeta, P: PageManager<Meta>> {
    pub space: &'a mut AddressSpace<Meta, P>,
    pub prange: Range<PPN<Meta>>,
    pub flags: VmFlags<Meta>,
}

impl<Meta: VmMeta, M: PageManager<Meta>> Decorator<Meta> for Mapper<'_, Meta, M> {
    #[inline]
    fn arrive(&mut self, pte: &mut Pte<Meta>, target_hint: Pos<Meta>) -> Pos<Meta> {
        assert!(!pte.is_valid());
        *pte = self.flags.build_pte(self.prange.start);
        self.prange.start += 1;
        if self.prange.start == self.prange.end {
            Pos::stop()
        } else {
            target_hint.next()
        }
    }

    #[inline]
    fn meet(
        &mut self,
        _level: usize,
        pte: Pte<Meta>,
        _target_hint: Pos<Meta>,
    ) -> Option<NonNull<Pte<Meta>>> {
        Some(self.space.manager.p_to_v(pte.ppn()))
    }

    #[inline]
    fn block(&mut self, _level: usize, pte: Pte<Meta>, _target_hint: Pos<Meta>) -> Update<Meta> {
        assert!(!pte.is_valid());
        let mut flags = VmFlags::VALID;
        let page = self.space.manager.allocate(1, &mut flags);
        let ppn = self.space.manager.v_to_p(page);
        Update::Pte(flags.build_pte(ppn), page.cast())
    }
}
