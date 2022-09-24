#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::*;

fn func() {
    println!("user_sig_test succsess");
    sigreturn();
}

#[no_mangle]
pub fn main() -> i32 {
    let mut new = SignalAction::default();
    let old = SignalAction::default();
    new.handler = func as usize;
    println!("pid = {}", getpid());
    println!("signal_simple: sigaction");
    if sigaction(SignalNo::SIGUSR1, &new, &old) < 0 {
        panic!("Sigaction failed!");
    }
    println!("signal_simple: kill");
    if kill(getpid(), SignalNo::SIGUSR1) < 0 {
        println!("Kill failed!");
        exit(1);
    }
    println!("signal_simple: Done");
    0
}
