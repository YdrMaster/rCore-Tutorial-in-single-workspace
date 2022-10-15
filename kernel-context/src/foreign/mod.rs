mod multislot_portal;

pub use multislot_portal::MultislotPortal;

use crate::{build_sstatus, LocalContext};
use spin::Lazy;

/// 传送门缓存。
///
/// 映射到公共地址空间，在传送门一次往返期间暂存信息。
#[repr(C)]
pub struct PortalCache {
    a0: usize,       //    (a0) 目标控制流 a0
    ra: usize,       // 1*8(a0) 目标控制流 ra      （寄存，不用初始化）
    satp: usize,     // 2*8(a0) 目标控制流 satp
    sstatus: usize,  // 3*8(a0) 目标控制流 sstatus
    sepc: usize,     // 4*8(a0) 目标控制流 sepc
    stvec: usize,    // 5*8(a0) 当前控制流 stvec   （寄存，不用初始化）
    sscratch: usize, // 6*8(a0) 当前控制流 sscratch（寄存，不用初始化）
}

impl PortalCache {
    /// 初始化传送门缓存。
    #[inline]
    pub fn init(&mut self, satp: usize, pc: usize, a0: usize, supervisor: bool, interrupt: bool) {
        self.satp = satp;
        self.sepc = pc;
        self.a0 = a0;
        self.sstatus = build_sstatus(supervisor, interrupt);
    }

    /// 返回缓存地址。
    #[inline]
    pub fn address(&mut self) -> usize {
        self as *mut _ as _
    }
}

/// 异界传送门。
///
/// 用于将线程传送到另一个地址空间上执行的基础设施。
pub trait ForeignPortal {
    /// 映射到公共地址空间的代码入口。
    unsafe fn transit_entry(&self) -> usize;
    /// 映射到公共地址空间的 `key` 号传送门缓存。
    unsafe fn transit_cache(&mut self, key: usize) -> &mut PortalCache;
}

/// 整体式异界传送门。
///
/// 传送门代码和插槽紧挨着放置。这样的传送门对象映射到公共地址空间时应同时具有读、写和执行权限。
pub trait MonoForeignPortal {
    /// 传送门对象的总字节数。
    fn total_size(&self) -> usize;

    /// 传送门对象在公共地址空间上的地址。
    fn transit_address(&self) -> usize;

    /// 传送门代码在对象中的偏移。
    fn text_offset(&self) -> usize;

    /// `key` 号插槽在传送门对象中的偏移。
    fn cache_offset(&self, key: usize) -> usize;
}

impl<T: MonoForeignPortal> ForeignPortal for T {
    #[inline]
    unsafe fn transit_entry(&self) -> usize {
        self.transit_address() + self.text_offset()
    }

    #[inline]
    unsafe fn transit_cache(&mut self, key: usize) -> &mut PortalCache {
        &mut *((self.transit_address() + self.cache_offset(key)) as *mut _)
    }
}

/// 异界线程上下文。
///
/// 不在当前地址空间的线程。
pub struct ForeignContext {
    /// 目标地址空间上的线程上下文。
    pub context: LocalContext,
    /// 目标地址空间。
    pub satp: usize,
}

impl ForeignContext {
    /// 执行异界线程。
    pub unsafe fn execute(&mut self, portal: &mut impl ForeignPortal, cache_key: usize) -> usize {
        use core::mem::replace;
        // 异界传送门需要特权态执行
        let supervisor = replace(&mut self.context.supervisor, true);
        // 异界传送门不能打开中断
        let interrupt = replace(&mut self.context.interrupt, false);
        // 找到公共空间上的缓存
        let entry = portal.transit_entry();
        let cache = portal.transit_cache(cache_key);
        // 重置传送门上下文
        cache.init(
            self.satp,
            self.context.sepc,
            self.context.a(0),
            supervisor,
            interrupt,
        );
        // 执行传送门代码
        *self.context.pc_mut() = entry;
        *self.context.a_mut(0) = cache.address();
        let sstatus = self.context.execute();
        // 恢复线程属性
        self.context.supervisor = supervisor;
        self.context.interrupt = interrupt;
        // 从传送门读取上下文
        *self.context.a_mut(0) = cache.a0;
        sstatus
    }
}

/// 传送门代码
struct PortalText(&'static [u16]);

/// 定位传送门代码段。
///
/// 通过寻找结尾的 `jr a0` 和 `options(noreturn)`，在运行时定位传送门工作的裸函数代码段。
/// 不必在链接时决定代码位置，可以在运行时将这段代码加载到任意位置。
static PORTAL_TEXT: Lazy<PortalText> = Lazy::new(PortalText::new);

impl PortalText {
    pub fn new() -> Self {
        // 32 是一个任取的不可能的下限
        for len in 32.. {
            let slice = unsafe { core::slice::from_raw_parts(foreign_execute as *const _, len) };
            // 裸函数的 `options(noreturn)` 会在结尾生成一个 0 指令，这是一个 unstable 特性所以不一定可靠
            if slice.ends_with(&[0x8502, 0]) {
                return Self(slice);
            }
        }
        unreachable!()
    }

    #[inline]
    pub fn aligned_size(&self) -> usize {
        const USIZE_MASK: usize = core::mem::size_of::<usize>() - 1;
        (self.0.len() * core::mem::size_of::<u16>() + USIZE_MASK) & !USIZE_MASK
    }

    #[inline]
    pub unsafe fn copy_to(&self, address: usize) {
        (address as *mut u16).copy_from_nonoverlapping(self.0.as_ptr(), self.0.len());
    }
}

/// 切换地址空间然后 sret。
/// 地址空间恢复后一切都会恢复原状。
#[naked]
unsafe extern "C" fn foreign_execute(ctx: *mut PortalCache) {
    core::arch::asm!(
        // 位置无关加载
        "   .option push
            .option nopic
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
            csrrw ra, satp, ra
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
        // 离开异界传送门直接跳到正常上下文的 stvec
        "   jr    a0",
        "   .option pop",
        options(noreturn)
    )
}
