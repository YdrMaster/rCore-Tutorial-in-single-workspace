#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use alloc::vec;
use user_lib::{exit, thread_create, waittid};

pub fn thread_a() -> isize {
    for _ in 0..1000 {
        print!("a");
    }
    exit(1)
}

pub fn thread_b() -> isize {
    for _ in 0..1000 {
        print!("b");
    }
    exit(2)
}

pub fn thread_c() -> isize {
    for _ in 0..1000 {
        print!("c");
    }
    exit(3)
}

#[no_mangle]
pub fn main() -> i32 {
    println!("threads test========");
    let mut v = vec![
        thread_create(thread_a as usize, 0),
        thread_create(thread_b as usize, 0),
        thread_create(thread_c as usize, 0),
    ];
    println!("v :{:?}", v);
    let max_len = 5;
    for i in 0..max_len {
        println!("create tid: {}", i + 4);
        let tid = thread_create(thread_b as usize, 0);
        println!("create tid: {} end", i + 4);
        v.push(tid);
    }
    println!("create end");
    for tid in v.iter() {
        let exit_code = waittid(*tid as usize);
        println!("thread#{} exited with code {}", tid, exit_code);
    }
    println!("main thread exited.");
    println!("==========================v size: {:?}", v);
    0
}
