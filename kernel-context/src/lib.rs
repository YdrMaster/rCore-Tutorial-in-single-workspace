#![no_std]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

use core::arch::asm;

/// 陷入上下文。
#[repr(C)]
pub struct Context {
    sctx: usize,
    x: [usize; 31],
    sstatus: usize,
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

pub enum Previlege {
    User,
    Supervisor,
}

const PREVILEGE_BIT: usize = 1 << 8;
const INTERRUPT_BIT: usize = 1 << 5;

impl Context {
    /// 初始化指定入口的用户上下文。
    ///
    /// 切换到用户态时会打开内核中断。
    #[inline]
    pub const fn new(entry: usize) -> Self {
        Self {
            sctx: 0,
            x: [0; 31],
            sstatus: 0,
            sepc: entry,
        }
    }

    /// 设置 [`execute`] 时切换到这个上下文。
    #[inline]
    pub fn be_next(&mut self) {
        unsafe { asm!("csrw sscratch, {}", in(reg) self) };
    }

    /// 设置一个标准的用户态上下文，在当前状态基础上具有用户特权级并开启中断。
    #[inline]
    pub fn set_sstatus_as_user(&mut self) {
        self.load_sstatus();
        self.set_privilege(Previlege::User);
        self.set_interrupt(true);
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
    pub fn sp(&self) -> usize {
        self.x(2)
    }

    /// 修改用户栈指针。
    #[inline]
    pub fn sp_mut(&mut self) -> &mut usize {
        self.x_mut(2)
    }

    /// 从当前上下文加载 `sstatus`。
    #[inline]
    pub fn load_sstatus(&mut self) {
        unsafe { asm!("csrr {}, sstatus", out(reg) self.sstatus) };
    }

    /// 读取上下文特权级。
    #[inline]
    pub fn privilege(&self) -> Previlege {
        if self.sstatus & PREVILEGE_BIT == 0 {
            Previlege::User
        } else {
            Previlege::Supervisor
        }
    }

    /// 设置上下文特权级。
    #[inline]
    pub fn set_privilege(&mut self, previlige: Previlege) {
        match previlige {
            Previlege::User => self.sstatus &= !PREVILEGE_BIT,
            Previlege::Supervisor => self.sstatus |= PREVILEGE_BIT,
        }
    }

    /// 读取上下文中断是否开启。
    #[inline]
    pub fn interrupt(&self) -> bool {
        self.sstatus & INTERRUPT_BIT != 0
    }

    /// 设置上下文特权级。
    #[inline]
    pub fn set_interrupt(&mut self, enabled: bool) {
        if enabled {
            self.sstatus |= INTERRUPT_BIT;
        } else {
            self.sstatus &= !INTERRUPT_BIT;
        }
    }

    /// 执行当前上下文。
    #[inline]
    pub unsafe fn execute(&mut self) {
        self.be_next();
        execute();
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
#[link_section = ".text.trampoline"]
pub unsafe extern "C" fn execute() {
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
        // 恢复 csr
        "   ld   t0, 32*8(sp)
            ld   t1, 33*8(sp)
            csrw sstatus, t0
            csrw    sepc, t1
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
#[link_section = ".text.trampoline"]
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
        // 保存 csr
        "   csrr t1, sstatus
            csrr t2, sepc
            sd   t1, 32*8(sp)
            sd   t2, 33*8(sp)
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
