/// 应用程序元数据。
#[repr(C)]
pub struct AppMeta {
    base: u64,
    step: u64,
    count: u64,
    first: u64,
}

impl AppMeta {
    /// 定位应用程序。
    #[inline]
    pub fn locate() -> &'static Self {
        extern "C" {
            static apps: AppMeta;
        }
        unsafe { &apps }
    }

    /// 遍历链接进来的应用程序。
    #[inline]
    pub fn iter(&'static self) -> AppIterator {
        AppIterator { meta: self, i: 0 }
    }
}

/// 应用程序迭代器。
pub struct AppIterator {
    meta: &'static AppMeta,
    i: u64,
}

impl Iterator for AppIterator {
    type Item = &'static [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.meta.count {
            None
        } else {
            let i = self.i as usize;
            self.i += 1;
            unsafe {
                let slice = core::slice::from_raw_parts(
                    &self.meta.first as *const _ as *const usize,
                    (self.meta.count + 1) as _,
                );
                let pos = slice[i];
                let size = slice[i + 1] - pos;
                let base = self.meta.base as usize + i * self.meta.step as usize;
                if base != 0 {
                    core::ptr::copy_nonoverlapping::<u8>(pos as _, base as _, size);
                    core::slice::from_raw_parts_mut(base as *mut u8, 0x20_0000)[size..].fill(0);
                    Some(core::slice::from_raw_parts(base as _, size))
                } else {
                    Some(core::slice::from_raw_parts(pos as _, size))
                }
            }
        }
    }
}
