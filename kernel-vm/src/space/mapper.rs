use crate::{AddressSpace, Ownership, PageManager};
use core::{ops::Range, ptr::NonNull};
use page_table::{Decorator, Pos, Pte, Update, VmFlags, VmMeta, PPN};

pub(super) struct Mapper<'a, Meta: VmMeta, M: PageManager<Meta>> {
    space: &'a mut AddressSpace<Meta, M>,
    range: Range<PPN<Meta>>,
    flags: VmFlags<Meta>,
    done: bool,
}

impl<'a, Meta: VmMeta, M: PageManager<Meta>> Mapper<'a, Meta, M> {
    #[inline]
    pub fn new(
        space: &'a mut AddressSpace<Meta, M>,
        range: Range<PPN<Meta>>,
        flags: VmFlags<Meta>,
    ) -> Self {
        Self {
            space,
            range,
            flags,
            done: false,
        }
    }

    #[inline]
    pub fn ans(self) -> bool {
        self.done
    }
}

impl<Meta: VmMeta, M: PageManager<Meta>> Decorator<Meta> for Mapper<'_, Meta, M> {
    #[inline]
    fn arrive(&mut self, pte: &mut Pte<Meta>, target_hint: Pos<Meta>) -> Pos<Meta> {
        assert!(!pte.is_valid());
        *pte = self.flags.build_pte(self.range.start);
        self.range.start += 1;
        if self.range.start == self.range.end {
            self.done = true;
            Pos::stop()
        } else {
            target_hint.next()
        }
    }

    #[inline]
    fn meet(
        &mut self,
        level: usize,
        pte: Pte<Meta>,
        target: Pos<Meta>,
    ) -> Option<NonNull<Pte<Meta>>> {
        if let Ownership::Owned = self.space.0.check_ownership(Pos { level, ..target }, pte) {
            Some(self.space.0.p_to_v(pte.ppn()))
        } else {
            None
        }
    }

    #[inline]
    fn block(&mut self, _level: usize, pte: Pte<Meta>, _target_hint: Pos<Meta>) -> Update<Meta> {
        assert!(!pte.is_valid());
        let mut flags = VmFlags::VALID;
        let page = self.space.0.allocate(1, &mut flags);
        let ppn = self.space.0.v_to_p(page);
        Update::Pte(flags.build_pte(ppn), page.cast())
    }
}
