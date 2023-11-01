use alloc::alloc::dealloc;
use alloc::fmt::format;
use alloc::format;
use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// Make hardlink, returns a new inode representing the link node
    pub fn link_at(&self, src: &str, dst: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();

        let src_inode_id = self.read_disk_inode(|disk_inode| self.find_inode_id(src, disk_inode));
        let dst_inode_id = self.read_disk_inode(|disk_inode| self.find_inode_id(dst, disk_inode));
        // source inode is not found or dst inode is found
        if src_inode_id.is_none() || dst_inode_id.is_some() { return None }

        let src_inode_id = src_inode_id.unwrap();
        // append hardlink to current disk_inode of directory
        self.modify_disk_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            self.increase_size(new_size as u32, root_inode, &mut fs);
            let dirent = DirEntry::new(dst, src_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(src_inode_id);
        block_cache_sync_all();
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
    }
    /// unlink hardlink, returns true if succeed.
    pub fn unlink_at(&self, name: &str) -> bool {
        let src_inode_id = self.read_disk_inode(|disk_inode| self.find_inode_id(name, disk_inode));
        if src_inode_id.is_none() { return false }
        let src_inode_id = src_inode_id.unwrap();

        let all_links = self.read_disk_inode(|root_node: &DiskInode| {
            assert!(root_node.is_dir());
            let file_count = (root_node.size as usize) / DIRENT_SZ;
            let mut links = Vec::new();
            let mut dirent = DirEntry::empty();
            for i in 0..file_count {
                assert_eq!(
                    root_node.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                if dirent.inode_id() == src_inode_id {
                    links.push((i, dirent.clone()));
                }
            }
            links
        });

        // only 1 hardlink, the file name must be the provided
        // and the data block and file index block should be cleared.
        if all_links.len() == 1 {
            let single = all_links[0];
            assert_eq!(single.1.name(), name);
            let (block_id, block_offset) = {
                let fs = self.fs.lock();
                fs.get_disk_inode_pos(single.1.inode_id())
            };

            // clear the file inode
            Self::new(
                block_id,
                block_offset,
                self.fs.clone(),
                self.block_device.clone()
            ).clear();
        }

        // remove file index from inode
        self.modify_disk_inode(|root_node| {
            for (idx, dirent) in &all_links {
                if dirent.name() == name {
                    root_node.write_at(idx * DIRENT_SZ, DirEntry::empty().as_bytes(), &self.block_device);
                    return true
                }
            }
            false
        })
    }
    /// get inode and hardlink count
    /// 因为目前目录就一层结构，所以直接用 ROOT_INODE 查找硬链接数量
    pub fn stat(&self) -> (u32, usize, u8) {

        // TODO: 如果是多级目录需要改成 parent
        let parent = EasyFileSystem::root_inode(&self.fs.clone());

        let fs = self.fs.lock();
        parent.read_disk_inode(|root_node: &DiskInode| {
            assert!(root_node.is_dir());

            let mut inode_id: u32 = 0;
            let mut inode_type: u8 = 0;
            let mut links: usize = 0;

            let file_count = (root_node.size as usize) / DIRENT_SZ;
            let mut dirent = DirEntry::empty();

            // 根据 block_id 和 block_offset 确定 DirEnt
            for i in 0..file_count {
                assert_eq!(
                    root_node.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                let (block_id, block_offset) = fs.get_disk_inode_pos(dirent.inode_id());
                if block_id as usize == self.block_id && block_offset == self.block_offset {
                    inode_id = dirent.inode_id();

                    inode_type = Self::new(
                        block_id,
                        block_offset,
                        self.fs.clone(),
                        self.block_device.clone()
                    ).read_disk_inode(|node| if node.is_file() { 0 } else { 1 });

                    break
                }
            }

            for i in 0..file_count {
                assert_eq!(
                    root_node.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                if dirent.inode_id() == inode_id {
                    links += 1;
                }
            }

            (inode_id, links, inode_type)
        })
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
}
