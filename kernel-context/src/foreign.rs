use crate::{build_sstatus, LocalContext};

/// 异界传送门。
///
/// 必须位于公共地址空间中。
#[repr(C)]
pub struct ForeignPortal {
    a0: usize,              //    (a0) 目标控制流 a0
    ra: usize,              // 1*8(a0) 目标控制流 ra
    satp: usize,            // 2*8(a0) 目标控制流 satp
    sstatus: usize,         // 3*8(a0) 目标控制流 sstatus
    sepc: usize,            // 4*8(a0) 目标控制流 sepc
    stvec: usize,           // 5*8(a0) 当前控制流 stvec   （寄存，不用初始化）
    sscratch: usize,        // 6*8(a0) 当前控制流 sscratch（寄存，不用初始化）
    execute: [usize; 1024], // 7*8(a0) 执行代码
}

/// 异界线程上下文。
///
/// 不在当前地址空间的线程。
pub struct ForeignContext {
    context: LocalContext,
    satp: usize,
}

impl ForeignContext {
    /// 执行异界线程。
    ///
    /// `portal` 是线性地址空间上的传送门对象。`protal_transit` 是公共地址空间上的传送门对象。
    pub unsafe fn execute(&mut self, portal: &mut ForeignPortal, protal_transit: usize) -> usize {
        use core::mem::replace;

        let supervisor = replace(&mut self.context.supervisor, true);
        let interrupt = replace(&mut self.context.interrupt, false);
        let sstatus: usize;
        core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);

        portal.sstatus = build_sstatus(sstatus, supervisor, interrupt);
        portal.satp = self.satp;
        portal.a0 = self.context.a(0);
        portal.sepc = self.context.sepc;

        self.context.sepc = protal_transit + 7 * 8;

        *self.context.a_mut(0) = protal_transit;
        let sstatus = self.context.execute();
        *self.context.a_mut(0) = portal.a0;

        sstatus
    }
}

/// 切换地址空间然后 sret。
///
/// 地址空间恢复后一切都会恢复原状。
#[naked]
unsafe extern "C" fn _foreign_execute(ctx: *mut ForeignPortal) {
    core::arch::asm!(
        // 位置无关加载
        "   .option push
            .option pic
        ",
        // 保存 ra，ra 会用来寄存
        "   sd    ra, 1*8(a0)",
        // 交换地址空间
        "   ld    ra, 2*8(a0)
            csrrw ra, satp, ra
            sfence.vma
            sd    ra, 2*8(a0)
        ",
        // 加载 sstatus
        "   ld    ra, 3*8(a0)
            csrw      sstatus, ra
        ",
        // 加载 sepc
        "   ld    ra, 4*8(a0)
            csrw      sepc, ra
        ",
        // 交换陷入入口
        "   la    ra, 1f
            csrrw ra, stvec, ra
            sd    ra, 5*8(a0)
        ",
        // 交换 sscratch
        "   csrrw ra, sscratch, a0
            sd    ra, 6*8(a0)
        ",
        // 加载通用寄存器
        "   ld    ra, 1*8(a0)
            ld    a0,    (a0)
        ",
        // 出发！
        "   sret",
        // 陷入
        "   .align 2",
        // 加载 a0
        "1: csrrw a0, sscratch, a0",
        // 保存 ra，ra 会用来寄存
        "   sd    ra, 1*8(a0)",
        // 交换 sscratch 并保存 a0
        "   ld    ra, 6*8(a0)
            csrrw ra, sscratch, ra
            sd    ra,    (a0)
        ",
        // 恢复地址空间
        "   ld    ra, 2*8(a0)
            csrrw ra, stap, ra
            sfence.vma
            sd    ra, 2*8(a0)
        ",
        // 恢复通用寄存器
        "   ld    ra, 1*8(a0)",
        // 恢复陷入入口
        "   ld    a0, 5*8(a0)
            csrw      stvec, a0
        ",
        // 回家！
        "   jr    a0",
        "   .option pop",
        options(noreturn)
    )
}
