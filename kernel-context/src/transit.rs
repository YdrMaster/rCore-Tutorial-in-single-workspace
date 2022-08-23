use crate::Context;

/// 中转内核布局。
#[repr(C)]
pub struct TransitKernel {
    /// 共享任务上下文。
    pub shared_context: ForeignContext,
    /// `execute` 的拷贝。
    ///
    /// 512 Bytes，4 字节对齐。
    pub execute_copy: [u32; 128],
    /// `trap` 的拷贝。
    ///
    /// 512 Bytes，4 字节对齐。
    pub trap_copy: [u32; 128],
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

/// 中转内核控制流。
#[inline(never)]
#[link_section = ".transit.entry"]
pub extern "C" fn transit_main(
    _ctx: &'static mut ForeignContext,
    _execute_copy: unsafe extern "C" fn(),
    _trap_copy: unsafe extern "C" fn(),
) {
    todo!()
}

impl TransitKernel {
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

    /// 中转内核运行时初始化。
    pub unsafe fn init(&mut self) {
        use core::mem::size_of_val;

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
