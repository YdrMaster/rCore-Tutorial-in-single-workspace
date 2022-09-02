
use crate::mm::{MutAllocator, Page};
// use core::cmp::max;
use page_table::{Decorator, Pos, Pte, Sv39, Update, VAddr, VmFlags, PPN};
// use xmas_elf::{program, ElfFile};

pub struct KernelSpaceBuilder<'a, const N: usize>(pub &'a mut MutAllocator<N>);

impl<'a, const N: usize> Decorator<Sv39> for KernelSpaceBuilder<'a, N> {
    #[inline]
    fn start(&mut self, _: Pos<Sv39>) -> Pos<Sv39> {
        Pos::new(VAddr::new(__text as usize).floor(), 0)
    }

    #[inline]
    fn arrive(&mut self, pte: &mut Pte<Sv39>, target_hint: Pos<Sv39>) -> Pos<Sv39> {
        let addr = target_hint.vpn.base().val();
        let bits = if addr < __transit as usize {
            0b1011 // X_RV <- .text
        } else if addr < __rodata as usize {
            0b1111 // XWRV <- .trampline
        } else if addr < __data as usize {
            0b0011 // __RV <- .rodata
        } else if addr < __end as usize {
            0b0111 // _WRV <- .data + .bss
        } else {
            return Pos::stop(); // end of kernel sections
        };
        *pte = unsafe { VmFlags::from_raw(bits) }.build_pte(PPN::new(target_hint.vpn.val()));
        target_hint.next()
    }

    #[inline]
    fn meet(
        &mut self,
        _level: usize,
        _pte: Pte<Sv39>,
        _target_hint: Pos<Sv39>,
    ) -> Update<Sv39> {
        let (ptr, size) = self.0.allocate::<Page>(Page::LAYOUT).unwrap();
        assert_eq!(size, Page::LAYOUT.size());
        let vpn = VAddr::new(ptr.as_ptr() as _).floor();
        let ppn = PPN::new(vpn.val());
        Update::Pte(unsafe { VmFlags::from_raw(1) }.build_pte(ppn), vpn)
    }
}

// /// 计算应用程序总共需要多少个页。
// ///
// /// 包括存储各个加载段数据的页和页表页，以及位于低 256 GiB 最后两个 4 KiB 页的用户栈和它们的页表页。
// pub fn calculate_page_count(elf: &ElfFile) -> usize {
//     // 需要的总页计数
//     const COUNT_512G: usize = 1; // 2 级页表
//     let mut count_1g = 0usize; // 1 级页表数量
//     let mut count_2m = 0usize; // 0 级页表数量
//     let mut count_4k = 0usize; // 0 级页数量

//     // NOTICE ELF 文件中程序段是按虚存位置排序的，且段间不重叠，因此可以用一个指针表示已覆盖的范围
//     let mut end_1g = 0usize; // 2 级页表覆盖范围
//     let mut end_2m = 0usize; // 1 级页表覆盖范围

//     for program in elf.program_iter() {
//         if let Ok(program::Type::Load) = program.get_type() {
//             let off_file = program.offset();
//             let end_file = off_file + program.file_size();
//             let off_mem = program.virtual_addr();
//             let end_mem = off_mem + program.mem_size();
//             println!("LOAD {off_file:#08x}..{end_file:#08x} -> {off_mem:#08x}..{end_mem:#08x} with {:?}", program.flags());

//             // 更新 0 级页数量
//             {
//                 let off_mem = off_mem as usize >> 12;
//                 let end_mem = (end_mem as usize + mask(12)) >> 12;
//                 count_4k += end_mem - off_mem;
//             }
//             // 更新 0 级页表覆盖范围
//             {
//                 let mask_2m = mask(12 + 9);
//                 end_2m = max(end_2m, off_mem as usize & !mask_2m);
//                 let end_program = (end_mem as usize + mask_2m) & !mask_2m;
//                 while end_2m < end_program {
//                     count_2m += 1;
//                     end_2m += mask_2m + 1;
//                 }
//             }
//             // 更新 1 级页表覆盖范围
//             {
//                 let mask_1g = mask(12 + 9 + 9);
//                 end_1g = max(end_1g, off_mem as usize & !mask_1g);
//                 let end_program = (end_mem as usize + mask_1g) & !mask_1g;
//                 while end_1g < end_program {
//                     count_1g += 1;
//                     end_1g += mask_1g + 1;
//                 }
//             }
//         }
//     }
//     // 补充栈空间
//     count_4k += 2;
//     count_2m += 1;
//     count_1g += 1;
//     count_4k + count_2m + count_1g + COUNT_512G
// }

// #[inline]
// const fn mask(bits: usize) -> usize {
//     (1 << bits) - 1
// }

extern "C" {
    fn __text();
    fn __transit();
    fn __rodata();
    fn __data();
    fn __end();
}

