/// 异世界传送门。
///
/// 必须位于公共地址空间中。
#[repr(C)]
pub struct ForeignPortal {
    a0: usize,              //    (a0) 目标控制流 a0
    ra: usize,              // 1*8(a0) 目标控制流 ra
    satp: usize,            // 2*8(a0) 目标控制流 satp
    ra_: usize,             // 3*8(a0) 当前控制流 ra      （寄存，不用初始化）
    stvec: usize,           // 4*8(a0) 当前控制流 stvec   （寄存，不用初始化）
    sscratch: usize,        // 5*8(a0) 当前控制流 sscratch（寄存，不用初始化）
    execute: [usize; 1024], // 6*8(a0) 执行代码
}

/// 切换地址空间然后 sret。
///
/// 地址空间恢复后一切都会恢复原状。
#[naked]
unsafe extern "C" fn foreign_execute(ctx: *mut ForeignPortal) {
    core::arch::asm!(
        // 位置无关加载
        "   .option push
            .option pic
        ",
        // 保存 ra，ra 会用来寄存
        "   sd    ra, 3*8(a0)",
        // 交换地址空间
        "   ld    ra, 2*8(a0)
            csrrw ra, satp, ra
            sfence.vma
            sd    ra, 2*8(a0)
        ",
        // 交换陷入入口
        "   la    ra, 1f
            csrrw ra, stvec, ra
            sd    ra, 4*8(a0)
        ",
        // 交换 sscratch
        "   csrrw ra, sscratch, a0
            sd    ra, 5*8(a0)
        ",
        // 加载通用寄存器
        "   ld    ra, 1*8(a0)
            ld    a0,    (a0)
        ",
        // 出发！
        "   sret",
        // 陷入
        "1: .align 2",
        // 加载 a0
        "   csrrw a0, sscratch, a0",
        // 保存 ra，ra 会用来寄存
        "   sd    ra, 1*8(a0)",
        // 交换 sscratch 并保存 a0
        "   ld    ra, 5*8(a0)
            csrrw ra, sscratch, ra
            sd    ra,    (a0)
        ",
        // 恢复陷入入口
        "   ld    ra, 4*8(a0)
            csrw      stvec, ra
        ",
        // 恢复地址空间
        "   ld    ra, 2*8(a0)
            csrrw ra, stap, ra
            sfence.vma
            sd    ra, 2*8(a0)
        ",
        // 恢复通用寄存器
        "   ld    ra, 3*8(a0)",
        // 回家！
        "   ret",
        "   .option pop",
        options(noreturn)
    )
}
