#![no_std]
#![deny(warnings)]

#[repr(C)]
pub struct Context {
    x: [usize; 32],
    mstatus: usize,
    mepc: usize,
}
