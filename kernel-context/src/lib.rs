//! 内核上下文控制。

#![no_std]
#![feature(naked_functions, asm_sym, asm_const)]
// #![deny(warnings, missing_docs)]

// 不同地址空间的上下文控制。
// pub mod foreign;

use core::arch::asm;

/// 陷入上下文。
#[repr(C)]
pub struct Context {
    sctx: usize,
    x: [usize; 31],
    pub prev: Previlege,
    pub intr: bool,
    sepc: usize,
}

/// 内核上下文。
///
/// 切换到用户态之前会这个结构压在栈上。
#[repr(C)]
pub struct KernelContext {
    uctx: usize,
    x: [usize; 31],
}

/// 任务特权级。
pub enum Previlege {
    /// 用户态。
    User,
    /// 特权态。
    Supervisor,
}

const PREVILEGE_BIT: usize = 1 << 8;
const INTERRUPT_BIT: usize = 1 << 5;

impl Context {
    /// 初始化指定入口的用户上下文。
    ///
    /// 切换到用户态时会打开内核中断。
    #[inline]
    pub const fn user(entry: usize) -> Self {
        Self {
            sctx: 0,
            x: [0; 31],
            prev: Previlege::User,
            intr: true,
            sepc: entry,
        }
    }

    /// 读取用户通用寄存器。
    #[inline]
    pub fn x(&self, n: usize) -> usize {
        self.x[n - 1]
    }

    /// 修改用户通用寄存器。
    #[inline]
    pub fn x_mut(&mut self, n: usize) -> &mut usize {
        &mut self.x[n - 1]
    }

    /// 读取用户参数寄存器。
    #[inline]
    pub fn a(&self, n: usize) -> usize {
        self.x(n + 10)
    }

    /// 修改用户参数寄存器。
    #[inline]
    pub fn a_mut(&mut self, n: usize) -> &mut usize {
        self.x_mut(n + 10)
    }

    /// 读取用户栈指针。
    #[inline]
    pub fn ra(&self) -> usize {
        self.x(1)
    }

    /// 读取用户栈指针。
    #[inline]
    pub fn sp(&self) -> usize {
        self.x(2)
    }

    /// 修改用户栈指针。
    #[inline]
    pub fn sp_mut(&mut self) -> &mut usize {
        self.x_mut(2)
    }

    /// 执行当前上下文。
    #[inline]
    pub unsafe fn execute(&mut self) -> usize {
        let mut sstatus: usize;
        asm!("csrr {}, sstatus", out(reg) sstatus);
        match self.prev {
            Previlege::User => sstatus &= !PREVILEGE_BIT,
            Previlege::Supervisor => sstatus |= PREVILEGE_BIT,
        }
        match self.intr {
            false => sstatus &= !INTERRUPT_BIT,
            true => sstatus |= INTERRUPT_BIT,
        }
        asm!(
            "   csrw sscratch, {}
                csrw sepc    , {}
                csrw sstatus , {}
            ",
            in(reg) self,
            in(reg) self.sepc,
            in(reg) sstatus,
        );
        execute_naked();
        asm!(
            "   csrr {}, sepc
                csrr {}, sstatus
            ",
            out(reg) self.sepc,
            out(reg) sstatus,
        );
        sstatus
    }

    /// 当前上下文的 pc。
    #[inline]
    pub fn pc(&self) -> usize {
        self.sepc
    }

    /// 将 pc 移至下一条指令。
    ///
    /// # Notice
    ///
    /// 假设这一条指令不是压缩版本。
    #[inline]
    pub fn move_next(&mut self) {
        self.sepc = self.sepc.wrapping_add(4);
    }
}

/// 内核态切换到用户态。
///
/// # Safety
///
/// 裸函数。手动保存所有上下文环境。
#[naked]
unsafe extern "C" fn execute_naked() {
    asm!(
        r"  .altmacro
            .macro SAVE_S n
                sd x\n, \n*8(sp)
            .endm
            .macro LOAD_U n
                ld x\n, \n*8(sp)
            .endm
        ",
        // 初始化栈帧：sp = Sctx
        "   addi sp, sp, -32*8",
        // 用户上下文地址保存到内核上下文
        "   csrr  t0, sscratch
            sd    t0, (sp)
        ",
        // 保存内核上下文
        "   .set n, 1
            .rept 31
                SAVE_S %n
                .set n, n+1
            .endr
        ",
        // 切换上下文：sp = Uctx
        "   csrrw sp, sscratch, sp",
        // 内核上下文地址保存到用户上下文
        "   csrr  t0, sscratch
            sd    t0, (sp)
        ",
        // 恢复用户上下文
        "   ld x1, 1*8(sp)
            .set n, 3
            .rept 29
                LOAD_U %n
                .set n, n+1
            .endr
            ld sp, 2*8(sp)
        ",
        // 执行用户程序
        "   sret",
        options(noreturn)
    )
}

/// 用户态陷入内核态。
///
/// # Safety
///
/// 裸函数。利用恢复的 ra 回到 [`execute`] 的返回地址。
#[naked]
pub unsafe extern "C" fn trap() {
    asm!(
        r"
        .altmacro
        .macro SAVE_U n
            sd x\n, \n*8(sp)
        .endm
        .macro LOAD_S n
            ld x\n, \n*8(sp)
        .endm
        ",
        // 作为陷入地址需要 4 字节对齐
        "   .align 2",
        // 切换上下文：sp = Uctx
        "   csrrw sp, sscratch, sp
            ld    sp, (sp)
        ",
        // 保存用户上下文
        "   sd x1, 1*8(sp)
            .set n, 3
            .rept 29
                SAVE_U %n
                .set n, n+1
            .endr
            csrrw t0, sscratch, sp
            sd    t0, 2*8(sp)
        ",
        // 切换上下文：sp = Mctx
        "   ld sp, (sp)",
        // 恢复机器上下文
        "   .set n, 1
            .rept 31
                LOAD_S %n
                .set n, n+1
            .endr
        ",
        // 栈帧释放，返回
        "   addi sp, sp, 32*8
            ret
        ",
        options(noreturn)
    )
}
