# crate 类型

对项目中用到的和设计的 crates 分类如下：

- `[[bin]]`
  - `ch_`
  - `xtask`
- `[[lib]]`
  - 封装
  - 接口
  - 实现

## `[[bin]]`

`ch_` 是章节 crate，每个章节都能编译出一个独立的、适用于 qemu-virt 的内核。

`xtask` 是整个项目的构建工具。

## `[[lib]]`

**封装**、**接口**和**实现**是一个库 crate可能具有的标签，一个 crate 可能具有多个标签。

封装类型是对底层细节的直接封装。这种库的核心指标是无开销抽象，必须充分暴露底层细节，并且不能引入不必要的时间和空间复杂度。封装库常常是根据某种成文规范实现的库，类似 [*riscv*](https://crates.io/crates/riscv)，在这种情况下，库应该指出其遵循的规范文本的获取方式，注明其实现的规范版本和不一致/不完整之处。

接口类型通常提供至少一个 trait 定义和一组相关结构体。每个 trait 会定义了一类对象的接口，从而将接口与实现分离，类似 [*log*](https://crates.io/crates/log) 和 [*rustsbi*](https://crates.io/crates/rustsbi)。实现库则依赖接口库实现这些 trait，典型的是各种 logger 和 allocator。
