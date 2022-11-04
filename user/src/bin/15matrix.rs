#![no_std]
#![no_main]
#![allow(clippy::needless_range_loop)]

#[macro_use]
extern crate user_lib;

use user_lib::{clock_gettime, exit, fork, getpid, sched_yield, wait, ClockId, TimeSpec};

static NUM: usize = 30;
const N: usize = 10;
static P: i32 = 10007;
type Arr = [[i32; N]; N];

fn work(times: isize) {
    let mut a: Arr = Default::default();
    let mut b: Arr = Default::default();
    let mut c: Arr = Default::default();
    for i in 0..N {
        for j in 0..N {
            a[i][j] = 1;
            b[i][j] = 1;
        }
    }
    sched_yield();
    println!("pid {} is running ({} times)!.", getpid(), times);
    for _ in 0..times {
        for i in 0..N {
            for j in 0..N {
                c[i][j] = 0;
                for k in 0..N {
                    c[i][j] = (c[i][j] + a[i][k] * b[k][j]) % P;
                }
            }
        }
        for i in 0..N {
            for j in 0..N {
                a[i][j] = c[i][j];
                b[i][j] = c[i][j];
            }
        }
    }
    println!("pid {} done!.", getpid());
    exit(0);
}

#[no_mangle]
pub fn main() -> i32 {
    for _ in 0..NUM {
        let pid = fork();
        if pid == 0 {
            let mut time: TimeSpec = TimeSpec::ZERO;
            clock_gettime(ClockId::CLOCK_MONOTONIC, &mut time as *mut _ as _);
            let current_time = (time.tv_sec * 1000) + (time.tv_nsec / 1000000);
            let times = (current_time as i32 as isize) * (current_time as i32 as isize) % 1000;
            work(times * 10);
        }
    }

    println!("fork ok.");

    let mut exit_code: i32 = 0;
    for _ in 0..NUM {
        if wait(&mut exit_code) < 0 {
            panic!("wait failed.");
        }
    }
    assert!(wait(&mut exit_code) < 0);
    println!("matrix passed.");
    0
}
