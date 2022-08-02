# 第一章

第一章旨在展示一个尽量简单的**内核态裸机应用程序**：

- 只有[一个文件](src/main.rs)；
- 链接脚本在 [build.rs](build.rs)；
- 只依赖 [*sbi-rt*](https://github.com/rustsbi/sbi-rt/) 以获得封装好的 SBI 调用；
- 这个程序被 SEE 引导，工作在 S 态；
- 这个程序不需要环境：
  - 从汇编进入并为 rust 准备栈；
  - 依赖 SBI 提供的 legacy/console_putchar 打印 `Hello, world!`；
  - 依赖 SBI 提供的 system_reset 调用关机；
- 这大约不配被称作一个操作系统，因为它没有操作（硬件），也不构造系统；
