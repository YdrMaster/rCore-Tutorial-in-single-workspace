/// 根据应用名加载用户进程
// use xmas_elf::{
//     header::{self, HeaderPt2, Machine},
//     ElfFile,
// };
use lazy_static::*;
use alloc::vec::Vec;

lazy_static! {
    static ref APP_NAMES: Vec<&'static str> = {
        extern "C" {
            static apps: utils::AppMeta;
            fn app_names();
        }
        let app_num = unsafe { apps.get_app_num() }; 
        let mut start = app_names as usize as *const u8;
        let mut v = Vec::new();
        unsafe {
            for _ in 0..app_num {
                let mut end = start;
                while end.read_volatile() != b'\0' {
                    end = end.add(1);
                }
                let slice = core::slice::from_raw_parts(start, end as usize - start as usize);
                let str = core::str::from_utf8(slice).unwrap();
                v.push(str);
                start = end.add(1);
            }
        }
        v
    };
}

/// 获取应用程序 elf 数据 
pub fn get_app_data(app_name: &str) -> Option<&'static [u8]> {
    extern "C" {
        static apps: utils::AppMeta;
    }
    let app_num = apps.get_app_num();
    (0..app_num).find(|&i| APP_NAMES[i] == app_name).map(|&i|
        apps.iter_elf().nth(i)
    )
}

/// 获取 app_list
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in APP_NAMES.iter() {
        println!("{}", app);
    }
    println!("**************/");
}

