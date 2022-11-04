use easy_fs::{BlockDevice, EasyFileSystem};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

const BLOCK_SZ: usize = 512;

struct BlockFile(Mutex<File>);

impl BlockDevice for BlockFile {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.read(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.write(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }
}

pub fn easy_fs_pack(cases: &Vec<String>, target: &str) -> std::io::Result<()> {
    let block_file = Arc::new(BlockFile(Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}/{}", target, "fs.img"))?;
        f.set_len(64 * 2048 * 512).unwrap();
        f
    })));
    println!("Packing Testcases...");
    let efs = EasyFileSystem::create(block_file, 64 * 2048, 1);
    println!("Packing Testcases...");
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));
    println!("Packing Testcases...");
    for case in cases {
        println!("{}", format!("{}/{}", target, case));
        // load app data from host file system
        let mut host_file = File::open(format!("{}/{}", target, case)).unwrap();
        let mut all_data: Vec<u8> = Vec::new();
        host_file.read_to_end(&mut all_data).unwrap();
        // create a file in easy-fs
        let inode = root_inode.create(case.as_str()).unwrap();
        // write data to easy-fs
        inode.write_at(0, all_data.as_slice());
        // println!("{}", all_data.len());
    }
    println!("List Testcases in EFS: ");
    // list app
    for case in root_inode.readdir() {
        println!("{}", case);
    }
    Ok(())
}
