use core::{cmp::Ordering, fmt};
use page_table::{VmFlags, VmMeta, PPN, VPN};

#[derive(Clone, Debug)]
pub struct Segment<Meta: VmMeta> {
    pub(super) vbase: VPN<Meta>,
    pub(super) pbase: PPN<Meta>,
    pub(super) count: usize,
    pub(super) flags: VmFlags<Meta>,
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

impl<Meta: VmMeta> fmt::Display for Segment<Meta> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Segment {:#010x} -> {:#010x} (",
            self.pbase.val(),
            self.vbase.val()
        )?;
        Meta::fmt_flags(f, self.flags.0)?;
        write!(f, ") {} pages", self.count)
    }
}
