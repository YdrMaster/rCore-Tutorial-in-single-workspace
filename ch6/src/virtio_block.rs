use crate::KERNEL_SPACE;
use alloc::{
    alloc::{alloc_zeroed, dealloc},
    sync::Arc,
};
use core::{alloc::Layout, ptr::NonNull};
use easy_fs::BlockDevice;
use kernel_vm::page_table::{MmuMeta, Sv39, VAddr, VmFlags};
use spin::{Lazy, Mutex};
use virtio_drivers::{Hal, VirtIOBlk, VirtIOHeader};

const VIRTIO0: usize = 0x10001000;

pub static BLOCK_DEVICE: Lazy<Arc<dyn BlockDevice>> = Lazy::new(|| {
    Arc::new(unsafe {
        VirtIOBlock(Mutex::new(
            VirtIOBlk::new(&mut *(VIRTIO0 as *mut VirtIOHeader)).unwrap(),
        ))
    })
});

struct VirtIOBlock(Mutex<VirtIOBlk<'static, VirtioHal>>);

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

struct VirtioHal;

impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> usize {
        // warn!("dma_alloc");
        unsafe {
            alloc_zeroed(Layout::from_size_align_unchecked(
                pages << Sv39::PAGE_BITS,
                1 << Sv39::PAGE_BITS,
            )) as _
        }
    }

    fn dma_dealloc(paddr: usize, pages: usize) -> i32 {
        // warn!("dma_dealloc");
        unsafe {
            dealloc(
                paddr as _,
                Layout::from_size_align_unchecked(pages << Sv39::PAGE_BITS, 1 << Sv39::PAGE_BITS),
            )
        }
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
                .assume_init_ref()
                .translate(VAddr::new(vaddr), VALID)
                .unwrap()
        };
        ptr.as_ptr() as usize
    }
}
