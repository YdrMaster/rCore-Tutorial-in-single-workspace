# 第七章

已提供信号部分的用户库接口及测例，但内核没有相关实现

## 主体实现

目前 `/ch7` 模块复制自 `/ch6`，已经有了基本的信号实现。

在 `/xtask/src/user.rs` 和 `/user/cases.toml` 中加入了第七章相关信息。

## 用户测例

添加测例：`sig_ctrlc` `sig_simple` `sig_simple2` `sig_tests`，其中 `sig_ctrlc` `sig_simple` `sig_simple2` 可通过。

目前仅添加了信号相关的测例，后续还会加入其他测例。



（本项目与原 `rCore-Tutorial-v3`对用户程序的部分接口有所不同，因此引入时会修改部分代码）

## 信号部分

目前已在 `/syscall/src/user.rs` 添加用户库对应需要的 syscall：

- `kill` 发送信号
- `sigaction` 设置信号处理函数
- `sigprocmask` 修改信号掩码
- `sigreturn` 从信号处理函数中返回

并添加 `/signal-defs`，包含一些用户程序和内核通用的信号标号和处理函数定义。

> 这里 `SignalAction::mask` 使用 `usize` 而非 `i32`，是为了兼容将来可能会有的标号在 `[32,64)` 之间的实时信号。
> 
> 这里信号标号使用 `SignalNo`，是为了与上面的 `mask` 区分，提示用户程序在 `kill()` 和 `sigaction()` 中应使用信号的标号，而在 `sigprocmask` 中应使用信号的掩码

### 额外添加的 syscall 和代码

由于信号模块依赖一些前面章节的 syscall，但它们还没有实现，所以这里也添加和修改了一些信号之外的 syscall 和代码：

- 添加 `syscall: getpid`，应属于第五章。
- 添加 `/user/src/lib.rs: sleep(period_ms: usize)` ，应属于第三章。这里为了适应用户程序，还在 `/syscall/lib/time.rs` 中添加了从毫秒数(`usize`)转换为 `TimeSpec`的方法