# 第一章

第一章旨在展示一个尽量简单的**特权态裸机应用程序**：

- 只有[一个文件](src/main.rs)；
- 链接脚本在 [build.rs](build.rs)，以免增加依赖；
- 只依赖 [*sbi-rt*](https://crates.io/crates/sbi-rt) 以获得封装好的 SBI 调用；
- 这个程序被 SEE 引导，工作在 S 态；
- 这个程序不需要环境：
  - 从汇编进入并为 Rust 准备栈；
  - 依赖 SBI 提供的 `legacy::console_putchar` 打印 `Hello, world!`；
  - 依赖 SBI 提供的 `system_reset` 调用关机；

它不配被称作一个操作系统，因为它没有操作（硬件），也不构造（执行用户程序的）系统；

## sbi-rt

这个库就是 kernel 的 libc。

它根据 [SBI 标准](https://github.com/riscv-non-isa/riscv-sbi-doc)封装了一系列函数，通过 `ecall` 命令调用 SBI 提供的响应功能。本章需要使用 `legacy::console_putchar` 向控制台打印字符，以及 `system_reset` 在程序运行完后关机。

## 定制链接脚本

build.rs 的用法见[文档](https://doc.rust-lang.org/cargo/reference/build-scripts.html)。这个定制的链接脚本是特殊的：

```ld
OUTPUT_ARCH(riscv)
SECTIONS {
    .text 0x80200000 : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        *(.bss.uninit)
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
}
```

1. 为了被引导，它的 `.text` 在最前面。一般是 `.rodata` 在最前面。`.text` 的最前面是 `.text.entry`，有且只有一个汇编入口放在这个节，实现引导；
2. 正常情况下，裸机应用程序需要清除自己的 `.bss` 节，所以需要定义全局符号以便动态定位 `.bss`。但这一章的程序并不依赖 清空的 `.bss`，所以没有导出符号。`.bss` 本身仍然需要，因为栈会放在里面。

## 工作流程解读

1. SBI 初始化完成后，将固定跳转到 0x8020_0000 地址；
2. 根据链接脚本，汇编入口函数被放置在这个地址。它叫做 `_start`，这个名字是特殊的！GNU LD 及兼容其脚本的链接器会将这个名字认为是默认的入口，否则需要指定。这个函数是一个 rust 裸函数（[`#[naked]`](https://github.com/rust-lang/rust/issues/90957)），编译器不会为它添加任何序言和尾声，因此可以在没有栈的情况下执行。它将栈指针指向预留的栈空间，然后跳转到 `rust_main` 函数；
3. `rust_main` 函数在一个最简单的循环打印调用 sbi 打印 `Hello, world!` 字符串，然后关机。
