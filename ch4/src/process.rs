use crate::mm::PAGE;
use core::alloc::Layout;
use kernel_context::{foreign::ForeignContext, LocalContext};
use kernel_vm::AddressSpace;
use output::log;
use page_table::{MmuMeta, Sv39, VAddr, VmFlags, PPN, VPN};
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

            let mut flags = 0b10001;
            if program.flags().is_read() {
                flags |= 0b0010;
            }
            if program.flags().is_write() {
                flags |= 0b0100;
            }
            if program.flags().is_execute() {
                flags |= 0b1000;
            }

            unsafe {
                use core::slice::from_raw_parts_mut;

                let mut ptr = pages.as_ptr();
                from_raw_parts_mut(ptr, off_mem & PAGE_MASK).fill(0);
                ptr = ptr.add(off_mem & PAGE_MASK);
                ptr.copy_from_nonoverlapping(elf.input[off_file..].as_ptr(), len_file);
                ptr = ptr.add(len_file);
                from_raw_parts_mut(ptr, (1 << 12) - ((off_file + len_file) & PAGE_MASK)).fill(0);
            }

            address_space.push(
                svpn..evpn,
                PPN::new(pages.as_ptr() as usize >> 12),
                unsafe { VmFlags::from_raw(flags) },
            );
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
                VmFlags::from_raw(0b10111),
            );
        }

        log::info!("process entry = {:#x}", entry);
        log::info!("process page count = {:?}", address_space.page_count());
        for seg in address_space.segments() {
            log::info!("{seg}");
        }
        // log::debug!("\n{:?}", address_space.shuttle().unwrap());

        let mut context = LocalContext::user(entry);
        let satp = (8 << 60) | address_space.root_ppn().unwrap().val();
        *context.sp_mut() = 1 << 38;
        Some(Self {
            context: ForeignContext { context, satp },
            address_space,
        })
    }
}
