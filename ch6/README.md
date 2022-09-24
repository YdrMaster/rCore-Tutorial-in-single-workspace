# 第六章

目前的进度：

- 在 `xtask` 中对 `easy-fs` 进行封装制作镜像，在 QEMU 中添加虚拟块设备
- 在 `ch6` 框架下利用现有模块重新实现 `VirtIO` 驱动，需要用到内核地址空间及当前的内存分配机制
- 修复内核栈溢出等关于内存的问题
- 增加下述接口并重现第五章的功能，移除 `loader`，通过 `easy-fs` 加载程序并执行
- 完成 `IO` 系统调用 `read/write/open/close`，通过原来的 `filetest_simple` 和 `cat_filea` 测试

总结：

- `easy-fs` 关于模块化的设计已经相对比较完善了，主要是如何将其接入现有框架
- 还需要继续讨论内存分配和回收的问题
- `fuse` 的设计对于文件系统模块化有很好的启发作用
- 现在的 `driver` 耦合性还比较强，还需要进一步完善
- 多级目录参考实现，目前还是只实现了 `root_inode` 下的扁平目录

## EasyFS

### Block

```rust
pub trait BlockDevice: Send + Sync + Any {
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    fn write_block(&self, block_id: usize, buf: &[u8]);
    fn handle_irq(&self);
}
```

- 需要外部提供块设备驱动，对块缓存层暴露根据块号读写块设备的接口。
- 这个部分不需要改动，目前已经可以和块设备驱动模块进行交互，具备灵活性和泛用性。

```rust
pub struct BlockCache {
    cache: Vec<u8>,
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
    modified: bool,
}
```

- 缓冲区的生命周期结束后其占用的内存空间会被回收，同时根据脏位判断是否要写回块设备。
- 这个部分可以考虑不同的查找和替换算法，目前 `BlockCacheManager` 默认实现了  `get_block_cache` 方法；实现其他算法的关键在于使用的数据结构，可以利用 Rust 的泛型机制。

### Layout

```txt
+------------+--------------+-------+-------------+------+
| SuperBlock | Inode Bitmap | Inode | Data Bitmap | Data |
+------------+--------------+-------+-------------+------+
```

- 往年留给同学们的实验是硬链接，需要修改文件系统内的 Inode 结构，增加持久化的链接数量信息。模块化的设计不太容易支持这种结构本身的修改，所以如果保留该实验题目，应该预先给出完整的结构体，不要求同学们修改这一部分。
- 目前大部分实现不需要改动，与 `BlockCacheManager` 的交互已经很好地屏蔽了内部信息。

### VFS

改动的主体，需要对暴露的接口进行修改和完善来实现更好的抽象。

```rust
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}
```

- `Inode`结构是暴露给上层的，可以让调用者对文件和目录进行直接操作。
- 目前的文件系统默认是扁平化的设计，无法访问多级目录，为了增加灵活性，可以将一部分查找接口暴露给上层进行定义和使用:

```rust
pub trait FSManager {
    /// Open a file
    fn open(&self, path: &str, flags: OpenFlags) -> Option<Arc<FileHandle>>;

    /// Find a file
    fn find(&self, path: &str) -> Option<Arc<Inode>>;

    /// Create a hard link to source file
    fn link(&self, src: &str, dst: &str) -> isize;

    /// Remove a hard link
    fn unlink(&self, path: &str) -> isize;

    /// List inodes under the target directory
    fn readdir(&self, path: &str) -> Option<Vec<String>>;
}
```

- 内核可以根据提供的接口自行定义按路径查找的逻辑，`easy-fs` 的实现中，已经给出了 `Inode` 的大部分操作，列举如下：

```rust
impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self;

    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V;

    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V;

    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32>;

    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>>;

    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    );

    /// Create inode under current inode by name.
    /// Attention: use find previously to ensure the new file not existing.
    pub fn create(&self, name: &str) -> Option<Arc<Inode>>;

    /// List inodes by id under current inode
    pub fn readdir(&self) -> Vec<String>;

    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize;

    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize;

    /// Clear the data in current inode
    pub fn clear(&self);
}
```

- 内核对 `FileHandle` 进行维护时，可以自行实现路径到 `FileHandle` 的映射缓存（参考 Linux 相关代码
