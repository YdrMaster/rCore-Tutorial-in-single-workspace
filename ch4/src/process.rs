use crate::mm::PAGE;
use core::{alloc::Layout, str::FromStr};
use kernel_context::{foreign::ForeignContext, LocalContext};
use kernel_vm::{
    page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN},
    AddressSpace,
};
use output::log;
use xmas_elf::{
    header::{self, HeaderPt2, Machine},
    program, ElfFile,
};
/// 进程。
pub struct Process {
    pub context: ForeignContext,
    pub address_space: AddressSpace<Sv39>,
}

impl Process {
    pub fn new(elf: ElfFile) -> Option<Self> {
        let entry = match elf.header.pt2 {
            HeaderPt2::Header64(pt2)
                if pt2.type_.as_type() == header::Type::Executable
                    && pt2.machine.as_machine() == Machine::RISC_V =>
            {
                pt2.entry_point as usize
            }
            _ => None?,
        };

        let mut address_space = AddressSpace::<Sv39>::new(0);
        for program in elf.program_iter() {
            if !matches!(program.get_type(), Ok(program::Type::Load)) {
                continue;
            }

            const PAGE_MASK: usize = (1 << 12) - 1;

            let off_file = program.offset() as usize;
            let len_file = program.file_size() as usize;
            let off_mem = program.virtual_addr() as usize;
            let end_mem = off_mem + program.mem_size() as usize;
            assert_eq!(off_file & PAGE_MASK, off_mem & PAGE_MASK);

            let svpn = VAddr::<Sv39>::new(off_mem).floor();
            let evpn = VAddr::<Sv39>::new(end_mem).ceil();
            let (pages, size) = unsafe {
                PAGE.allocate_layout::<u8>(Layout::from_size_align_unchecked(
                    (evpn.val() - svpn.val()) << 12,
                    1 << 12,
                ))
                .unwrap()
            };
            assert_eq!(size, (evpn.val() - svpn.val()) << 12);

            let mut flags: [u8; 5] = *b"U___V";
            if program.flags().is_execute() {
                flags[1] = b'X';
            }
            if program.flags().is_write() {
                flags[2] = b'W';
            }
            if program.flags().is_read() {
                flags[3] = b'R';
            }
            let flags =
                VmFlags::from_str(unsafe { core::str::from_utf8_unchecked(&flags) }).unwrap();

            unsafe {
                use core::slice::from_raw_parts_mut;

                let mut ptr = pages.as_ptr();
                from_raw_parts_mut(ptr, off_mem & PAGE_MASK).fill(0);
                ptr = ptr.add(off_mem & PAGE_MASK);
                ptr.copy_from_nonoverlapping(elf.input[off_file..].as_ptr(), len_file);
                ptr = ptr.add(len_file);
                from_raw_parts_mut(ptr, (1 << 12) - ((off_file + len_file) & PAGE_MASK)).fill(0);
            }

            address_space.push(svpn..evpn, PPN::new(pages.as_ptr() as usize >> 12), flags);
        }
        unsafe {
            const STACK_SIZE: usize = 2 << Sv39::PAGE_BITS;
            let (pages, size) = PAGE
                .allocate_layout::<u8>(Layout::from_size_align_unchecked(STACK_SIZE, 1 << 12))
                .unwrap();
            assert_eq!(size, STACK_SIZE);
            core::slice::from_raw_parts_mut(pages.as_ptr(), STACK_SIZE).fill(0);
            address_space.push(
                VPN::new((1 << 26) - 2)..VPN::new(1 << 26),
                PPN::new(pages.as_ptr() as usize >> 12),
                VmFlags::build_from_str("U_WRV"),
            );
        }

        log::info!("process entry = {:#x}", entry);
        log::debug!("{address_space:?}");

        let mut context = LocalContext::user(entry);
        let satp = (8 << 60) | address_space.root_ppn().val();
        *context.sp_mut() = 1 << 38;
        Some(Self {
            context: ForeignContext { context, satp },
            address_space,
        })
    }
}
