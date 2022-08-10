#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{clock_gettime, sched_yield, ClockId, TimeSpec};

#[no_mangle]
fn main() -> i32 {
    let mut time: TimeSpec = TimeSpec::ZERO;
    clock_gettime(ClockId::CLOCK_MONOTONIC, &mut time as *mut _ as _);
    let time = time + TimeSpec::SECOND;
    loop {
        let mut now: TimeSpec = TimeSpec::ZERO;
        clock_gettime(ClockId::CLOCK_MONOTONIC, &mut now as *mut _ as _);
        if now > time {
            break;
        }
        sched_yield();
    }
    println!("Test sleep OK!");
    0
}
