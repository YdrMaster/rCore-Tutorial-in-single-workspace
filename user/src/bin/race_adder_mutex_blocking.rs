#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use alloc::vec::Vec;
use user_lib::{clock_gettime, exit, thread_create, waittid, ClockId, TimeSpec};
use user_lib::{mutex_create, mutex_lock, mutex_unlock};

static mut A: usize = 0;
const PER_THREAD: usize = 1000;
const THREAD_COUNT: usize = 16;

unsafe fn f() -> isize {
    let mut t = 2usize;
    for _ in 0..PER_THREAD {
        mutex_lock(0);
        let a = &mut A as *mut usize;
        let cur = a.read_volatile();
        for _ in 0..500 {
            t = t * t % 10007;
        }
        a.write_volatile(cur + 1);
        mutex_unlock(0);
    }
    exit(t as i32)
}

#[no_mangle]
pub fn main() -> i32 {
    let mut start_time: TimeSpec = TimeSpec::ZERO;
    let mut end_time: TimeSpec = TimeSpec::ZERO;
    clock_gettime(ClockId::CLOCK_MONOTONIC, &mut start_time as *mut _ as _);
    assert_eq!(mutex_create(true), 0);
    let mut v = Vec::new();
    for _ in 0..THREAD_COUNT {
        v.push(thread_create(f as usize, 0) as usize);
    }
    let mut time_cost = Vec::new();
    for tid in v.iter() {
        time_cost.push(waittid(*tid));
    }
    clock_gettime(ClockId::CLOCK_MONOTONIC, &mut end_time as *mut _ as _);
    let total_time = end_time.tv_sec * 1000 + end_time.tv_nsec / 1_000_000
        - start_time.tv_sec * 1000
        - start_time.tv_nsec / 1_000_000;
    println!("time cost is {}ms", total_time);
    assert_eq!(unsafe { A }, PER_THREAD * THREAD_COUNT);
    0
}
