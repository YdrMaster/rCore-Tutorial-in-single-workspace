use super::{AddressSpace, Page, Table};
use core::ops::Range;
use page_table::{Decorator, Pos, Pte, Update, VmFlags, VmMeta, PPN, VPN};

pub(super) struct Mapper<'a, Meta: VmMeta> {
    pub space: &'a mut AddressSpace<Meta>,
    pub vbase: VPN<Meta>,
    pub prange: Range<PPN<Meta>>,
    pub flags: VmFlags<Meta>,
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
        *pte = self.flags.build_pte(self.prange.start);
        self.prange.start += 1;
        if self.prange.start == self.prange.end {
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
