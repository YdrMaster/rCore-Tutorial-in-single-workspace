#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exec, fork, println, sched_yield, wait};

#[no_mangle]
fn main() -> i32 {
    if fork() == 0 {
        // exec("user_shell\0", &[core::ptr::null::<u8>()]);
        exec("user_shell");
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            println!("pid is {}", pid);
            if pid == -1 {
                sched_yield();
                continue;
            } else {
                break;
            }
        }
    }
    0
}
