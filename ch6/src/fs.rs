use alloc::string::String;
use alloc::sync::Arc;
use easy_fs::{BlockDevice, EasyFileSystem, FSManager, FileHandle, Inode, OpenFlags};
use lazy_static::*;

use crate::virtio_block::BLOCK_DEVICE;

pub struct FileSystem {
    root: Arc<Inode>,
}

impl FSManager for FileSystem {
    fn open(&self, path: &str, flags: OpenFlags) -> Option<Arc<FileHandle>> {
        let (readable, writable) = flags.read_write();
        if flags.contains(OpenFlags::CREATE) {
            if let Some(inode) = self.find(path) {
                // Clear size
                inode.clear();
                Some(Arc::new(FileHandle::new(readable, writable, inode)))
            } else {
                // Create new file
                self.root
                    .create(path)
                    .map(|new_inode| Arc::new(FileHandle::new(readable, writable, new_inode)))
            }
        } else {
            self.find(path).map(|inode| {
                if flags.contains(OpenFlags::TRUNC) {
                    inode.clear();
                }
                Arc::new(FileHandle::new(readable, writable, inode))
            })
        }
    }

    fn find(&self, path: &str) -> Option<Arc<Inode>> {
        self.root.find(path)
    }

    fn readdir(&self, path: &str) -> Option<alloc::vec::Vec<String>> {
        Some(self.root.readdir())
    }

    fn link(&self, src: &str, dst: &str) -> isize {
        todo!()
    }

    fn unlink(&self, path: &str) -> isize {
        todo!()
    }
}

impl FileSystem {
    fn new(block_device: Arc<dyn BlockDevice>) -> Self {
        let efs = EasyFileSystem::open(block_device.clone());
        Self {
            root: Arc::new(EasyFileSystem::root_inode(&efs)),
        }
    }
}

lazy_static! {
    pub static ref FS: Arc<FileSystem> = Arc::new(FileSystem::new(BLOCK_DEVICE.clone()));
}
