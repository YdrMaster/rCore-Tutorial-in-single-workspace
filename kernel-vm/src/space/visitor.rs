use page_table::{Pos, Pte, VmMeta, VPN};

pub(super) struct Visitor<Meta: VmMeta> {
    vpn: VPN<Meta>,
    ans: Option<Pte<Meta>>,
}

impl<Meta: VmMeta> Visitor<Meta> {
    #[inline]
    pub const fn new(vpn: VPN<Meta>) -> Self {
        Self { vpn, ans: None }
    }

    #[inline]
    pub const fn ans(self) -> Option<Pte<Meta>> {
        self.ans
    }
}

impl<Meta: VmMeta> page_table::Visitor<Meta> for Visitor<Meta> {
    #[inline]
    fn start(&mut self, _pos: Pos<Meta>) -> Pos<Meta> {
        Pos {
            vpn: self.vpn,
            level: 0,
        }
    }

    #[inline]
    fn arrive(&mut self, pte: Pte<Meta>, _target_hint: Pos<Meta>) -> Pos<Meta> {
        if pte.is_valid() {
            self.ans = Some(pte);
        }
        Pos::stop()
    }

    #[inline]
    fn meet(&mut self, _level: usize, _pte: Pte<Meta>, _target_hint: Pos<Meta>) -> Pos<Meta> {
        Pos::stop()
    }
}
