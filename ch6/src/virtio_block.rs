use alloc::sync::Arc;
use core::{alloc::Layout, ptr::NonNull};
use easy_fs::BlockDevice;
use kernel_vm::page_table::{MmuMeta, Sv39, VAddr, VmFlags};
use lazy_static::*;
use spin::Mutex;
use virtio_drivers::{Hal, VirtIOBlk, VirtIOHeader};

use crate::{mm::PAGE, KERNEL_SPACE};

const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock(Mutex<VirtIOBlk<'static, VirtioHal>>);

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.0
            .lock()
            .read_block(block_id, buf)
            .expect("Error when reading VirtIOBlk");
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0
            .lock()
            .write_block(block_id, buf)
            .expect("Error when writing VirtIOBlk");
    }
}

impl VirtIOBlock {
    pub fn new() -> Self {
        unsafe {
            Self(Mutex::new(
                VirtIOBlk::<VirtioHal>::new(&mut *(VIRTIO0 as *mut VirtIOHeader)).unwrap(),
            ))
        }
    }
}

pub struct VirtioHal;

impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> usize {
        // warn!("dma_alloc");
        const PAGE_SIZE: usize = 1 << Sv39::PAGE_BITS;
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        let ptr: NonNull<u8> = unsafe { PAGE.allocate_layout(layout).unwrap().0 };
        ptr.as_ptr() as usize
    }

    fn dma_dealloc(paddr: usize, pages: usize) -> i32 {
        // warn!("dma_dealloc");
        const PAGE_SIZE: usize = 1 << Sv39::PAGE_BITS;
        let aligned = (paddr >> Sv39::PAGE_BITS) << Sv39::PAGE_BITS;
        let ptr = NonNull::new(aligned as *mut u8).unwrap();
        unsafe { PAGE.deallocate(ptr, pages * PAGE_SIZE) };
        0
    }

    fn phys_to_virt(paddr: usize) -> usize {
        // warn!("p2v");
        paddr
    }

    fn virt_to_phys(vaddr: usize) -> usize {
        // warn!("v2p");
        const VALID: VmFlags<Sv39> = VmFlags::build_from_str("__V");
        let ptr: NonNull<u8> = unsafe {
            KERNEL_SPACE
                .get()
                .unwrap()
                .translate(VAddr::new(vaddr), VALID)
                .unwrap()
        };
        ptr.as_ptr() as usize
    }
}

lazy_static! {
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(VirtIOBlock::new());
}
