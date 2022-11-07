# 第一章实验

第一章实验的示例，展示如何依赖 `rcore_console` crate。

在 [Cargo.toml](Cargo.toml#L9) 里添加：

```toml
rcore_console = { path = "../rcore_console"}
```

在 [main.rs](src/main.rs#L38) 里初始化：

```rust
rcore_console::init_console(&Console);
```

后续的章节都可以这样依赖 `rcore_console`。
