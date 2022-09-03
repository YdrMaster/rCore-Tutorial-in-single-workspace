use super::Page;
use core::ops::Range;
use page_table::{Decorator, Pos, Pte, Update, VmFlags, VmMeta, PPN, VPN};

pub(super) struct Mapper<Meta: VmMeta> {
    pub vpn_offset: usize,
    pub vbase: VPN<Meta>,
    pub prange: Range<PPN<Meta>>,
    pub flags: VmFlags<Meta>,
}

impl<Meta: VmMeta> Decorator<Meta> for Mapper<Meta> {
    #[inline]
    fn start(&mut self, _pos: Pos<Meta>) -> Pos<Meta> {
        Pos {
            vpn: self.vbase,
            level: 0,
        }
    }

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
    fn meet(&mut self, _level: usize, pte: Pte<Meta>, _target_hint: Pos<Meta>) -> Update<Meta> {
        assert!(!pte.is_valid());
        let page = Page::<Meta>::allocate();
        let vpn = page.vpn();
        let ppn = PPN::new(vpn.val() - self.vpn_offset);
        Update::Pte(unsafe { VmFlags::from_raw(1) }.build_pte(ppn), vpn)
    }
}
