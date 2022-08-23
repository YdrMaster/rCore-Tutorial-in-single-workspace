use crate::Context;

/// 异世界执行器。
///
/// 执行不在当前地址空间的任务。
///
/// > 勇敢的 HART，快去异世界冒险吧！
pub trait ForeignExecutor {
    /// 初始化一个中转页上的执行器。
    ///
    /// `base` 是执行器在中转地址空间上的虚地址。
    ///
    /// 返回可启动执行器的上下文。
    fn init(&mut self, base: usize) -> Context;

    /// 访问执行器共享的任务上下文。
    fn shared_context(&mut self) -> &mut ForeignContext;
}

/// Rust 异世界执行器。
#[repr(C)]
pub struct RustForeignExecutor {
    /// 共享任务上下文。
    shared_context: ForeignContext,
    /// `execute` 的拷贝。
    ///
    /// 512 Bytes，4 字节对齐。
    execute_copy: [u32; 128],
    /// `trap` 的拷贝。
    ///
    /// 512 Bytes，4 字节对齐。
    trap_copy: [u32; 128],
    // 中转内核控制流，直接链接进来。
    // pub main: [u32; 512],
    // 页上其余部分用作栈，运行时设置。
    // pub stack: [u8],
}

/// 位于不同地址空间的任务上下文。
#[repr(C)]
pub struct ForeignContext {
    /// `satp` 寄存器值指定地址空间。
    pub satp: usize,
    /// 正常的任务上下文。
    pub context: Context,
}

impl RustForeignExecutor {
    /// 构造空白的中转内核。
    pub const fn new() -> Self {
        Self {
            shared_context: ForeignContext {
                satp: 0,
                context: Context::new(0),
            },
            execute_copy: [0; 128],
            trap_copy: [0; 128],
        }
    }
}

impl ForeignExecutor for RustForeignExecutor {
    fn init(&mut self, base: usize) -> Context {
        use core::mem::size_of_val;
        unsafe {
            // sret + unimp
            let execute = locate_function(crate::execute as _, [0x0073, 0x1020, 0x0000]);
            assert!(
                size_of_val(&self.execute_copy) >= execute.len(),
                "`execute_copy` is too small in transit kernel"
            );
            self.execute_copy
                .as_mut_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(execute.as_ptr(), execute.len());

            // ret + unimp
            let trap = locate_function(crate::trap as _, [0x8082, 0x0000]);
            assert!(
                size_of_val(&self.trap_copy) >= trap.len(),
                "`trap_copy` is too small in transit kernel"
            );
            self.trap_copy
                .as_mut_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(trap.as_ptr(), trap.len());
        }

        let base_struct = self as *const _ as usize;
        let mut ans = Context::new(base + (executor_main_rust as usize - base_struct));
        *ans.a_mut(0) = base;
        *ans.a_mut(1) = base + (&self.execute_copy as *const _ as usize - base_struct);
        *ans.a_mut(2) = base + (&self.trap_copy as *const _ as usize - base_struct);
        ans.set_sstatus_as_executor(); // 可能调度时需要重新执行
        ans
    }

    #[inline]
    fn shared_context(&mut self) -> &mut ForeignContext {
        &mut self.shared_context
    }
}

/// Rust 执行器控制流。
#[inline(never)]
#[link_section = ".transit.entry.rust"]
pub extern "C" fn executor_main_rust(
    _ctx: &'static mut ForeignContext,
    _execute_copy: unsafe extern "C" fn(),
    _trap_copy: unsafe extern "C" fn(),
) {
    todo!()
}

/// 通过寻找结尾的指令在运行时定位一个函数。
unsafe fn locate_function<const N: usize>(entry: usize, key: [u16; N]) -> &'static [u8] {
    use core::{mem::size_of, slice::from_raw_parts};
    let entry = entry as *const u16;
    for len in 1.. {
        let ptr = entry.add(len);
        if key == from_raw_parts(ptr, key.len()) {
            return from_raw_parts(entry.cast(), size_of::<u16>() * (len + key.len()));
        }
    }
    unreachable!()
}
