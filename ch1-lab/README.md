# 第一章实验

第一章实验的示例，展示如何依赖 `console` crate。

在 [Cargo.toml](Cargo.toml#L9) 里添加：

```toml
console = { path = "../console"}
```

在 [main.rs](src/main.rs#L49) 里初始化：

```rust
init_console(&Console);
```

后续的章节都可以这样依赖 `console`。
