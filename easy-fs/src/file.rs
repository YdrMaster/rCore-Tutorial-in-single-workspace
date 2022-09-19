use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;

use crate::Inode;

///Array of u8 slice that user communicate with os
pub struct UserBuffer {
    ///U8 vec
    pub buffers: Vec<&'static mut [u8]>,
}

impl UserBuffer {
    ///Create a `UserBuffer` by parameter
    pub fn new(buffers: Vec<&'static mut [u8]>) -> Self {
        Self { buffers }
    }
    ///Length of `UserBuffer`
    pub fn len(&self) -> usize {
        let mut total: usize = 0;
        for b in self.buffers.iter() {
            total += b.len();
        }
        total
    }
}

/// File trait
pub trait File: Send + Sync {
    /// If readable
    fn readable(&self) -> bool;
    /// If writable
    fn writable(&self) -> bool;
    /// Read file to `UserBuffer`
    fn read(&mut self, buf: UserBuffer) -> usize;
    /// Write `UserBuffer` to file
    fn write(&mut self, buf: UserBuffer) -> usize;
}

/// Standard input
pub struct Stdin;

/// Standard output
pub struct Stdout;

bitflags! {
  /// Open file flags
  pub struct OpenFlags: u32 {
      ///Read only
      const RDONLY = 0;
      ///Write only
      const WRONLY = 1 << 0;
      ///Read & Write
      const RDWR = 1 << 1;
      ///Allow create
      const CREATE = 1 << 9;
      ///Clear file and return an empty one
      const TRUNC = 1 << 10;
  }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

/// Cached file metadata in memory
#[derive(Clone)]
pub struct FileHandle {
    /// FileSystem Inode
    pub inode: Arc<Inode>,
    /// Open options: able to read
    pub read: bool,
    /// Open options: able to write
    pub write: bool,
    /// Current offset
    pub offset: usize,
    // TODO: CH7
    // /// Specify if this is pipe
    // pub pipe: bool,
}

impl FileHandle {
    pub fn new(read: bool, write: bool, inode: Arc<Inode>) -> Self {
        Self {
            inode,
            read,
            write,
            offset: 0,
        }
    }
}

impl File for FileHandle {
    fn readable(&self) -> bool {
        self.read
    }

    fn writable(&self) -> bool {
        self.write
    }

    fn read(&mut self, mut buf: UserBuffer) -> usize {
        let mut total_read_size: usize = 0;
        for slice in buf.buffers.iter_mut() {
            let read_size = self.inode.read_at(self.offset, *slice);
            if read_size == 0 {
                break;
            }
            self.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }

    fn write(&mut self, buf: UserBuffer) -> usize {
        let mut total_write_size: usize = 0;
        for slice in buf.buffers.iter() {
            let write_size = self.inode.write_at(self.offset, *slice);
            assert_eq!(write_size, slice.len());
            self.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
}

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
