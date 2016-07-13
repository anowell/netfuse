extern crate fuse;
extern crate libc;
extern crate time;
extern crate sequence_trie;

mod inode;
mod cache;
mod nfs;

pub use nfs::{NetworkFilesystem, Metadata, LibcError};
use inode::{Inode, InodeStore};
use cache::CacheEntry;

use libc::{EIO, ENOENT};
use fuse::{FileType, FileAttr, Filesystem, Request, ReplyEntry, ReplyAttr, ReplyData, ReplyDirectory, ReplyOpen, ReplyEmpty, ReplyWrite};
use std::collections::HashMap;
use std::path::Path;
use time::Timespec;

const DEFAULT_TTL: Timespec = Timespec { sec: 1, nsec: 0 };

pub struct MountOptions<'a> {
    path: &'a Path,
    uid: u32,
    gid: u32,
    // read_only: bool,
}

impl <'a> MountOptions<'a> {
    pub fn new<P: AsRef<Path>>(path: &P) -> MountOptions {
        MountOptions {
            path: path.as_ref(),
            uid: unsafe { libc::getuid() } as u32,
            gid: unsafe { libc::getgid() } as u32,
            // read_only: false,
        }
    }
}

pub struct NetFuse<NFS: NetworkFilesystem> {
    inodes: InodeStore,
    nfs: NFS,
    /// map of inodes to to data buffers - indexed by inode (NOT inode-1)
    cache: HashMap<u64, CacheEntry>,
    uid: u32,
    gid: u32,
}

pub fn mount<NFS: NetworkFilesystem>(fs: NFS, options: MountOptions) {
    let netfuse = NetFuse {
        nfs: fs,
        inodes: InodeStore::new(0o550, options.uid, options.gid),
        cache: HashMap::new(),
        uid: options.uid,
        gid: options.gid,
    };
    fuse::mount(netfuse, &options.path, &[]);
}

impl <NFS: NetworkFilesystem> NetFuse<NFS> {
    fn insert_metadata(&mut self, path: &Path, metadata: &Metadata) -> &Inode {
        let ref mut inodes = self.inodes;
        let ino = inodes.len() as u64 + 1;
        println!("insert metadata: {} {}", ino, path.display());

        let attr = FileAttr {
            ino: ino,
            size: metadata.size,
            blocks: 0,
            atime: metadata.atime,
            mtime: metadata.mtime,
            ctime: metadata.ctime,
            crtime: metadata.crtime,
            kind: metadata.kind,
            perm: metadata.perm,
            nlink: 0,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            flags: 0,
        };
        // TODO: stop using to_string_lossy, and make the inode trie built from OsStr components
        inodes.insert(Inode::new(&path.to_string_lossy(), attr));
        inodes.get(ino).unwrap()
    }

    fn cache_readdir<'a>(&'a mut self, ino: u64) -> Box<Iterator<Item=Result<(String, FileAttr), LibcError>> + 'a> {
        let iter = self.inodes
                        .children(ino)
                        .into_iter()
                        .map( move |child| {
                            Ok((get_basename(&child.path).into(), child.attr.clone()))
                        });
        Box::new(iter)
    }

    // true if data was written, false if nothing needed written
    // error if writing failed
    fn flush_cache_if_needed(&mut self, ino: u64) -> Result<bool, LibcError> {
        let flushed = {
            let entry = self.cache.get(&ino).unwrap();

            match entry.warm && !entry.sync {
                true => {
                    let ref path = self.inodes[ino].path;
                    try!(self.nfs.write(&Path::new(&path), &entry.data));
                    true
                }
                false => false
            }
        };

        if flushed {
            // TODO: update attr mtime
            self.cache.get_mut(&ino).unwrap().sync = true;
        }

        Ok(flushed)
    }

    fn read_to_cache_if_needed(&mut self, ino: u64) -> Result<bool, LibcError> {
        // return if cache is already warm
        if self.cache.get(&ino).unwrap().warm {
            return Ok(false);
        }

        // make request to network backend for data to populate cache
        let path = Path::new(&self.inodes[ino].path);
        let mut buffer = Vec::new();
        let _ = try!(self.nfs.read(&path, &mut buffer));
        let mut entry = self.cache.get_mut(&ino).unwrap();
        entry.set(buffer);
        entry.sync = true;
        Ok(true)
    }

}

fn get_basename(path: &str) -> &str {
    path.rsplitn(2, "/").next().unwrap() //.to_string()
}

impl <NFS: NetworkFilesystem> Filesystem for NetFuse<NFS> {

    // If parent is marked visited, then only perform lookup in the cache
    // otherwise, if the cache lookup is a miss, perform the network lookup
    fn lookup(&mut self, _req: &Request, parent: u64, name: &Path, reply: ReplyEntry) {
        let name = name.to_string_lossy();
        println!("lookup(parent={}, name=\"{}\")", parent, name);

        // Clone until MIR NLL lands
        match self.inodes.child(parent, &name).cloned() {
            Some(child_inode) => reply.entry(&DEFAULT_TTL, &child_inode.attr, 0),
            None => {
                // Clone until MIR NLL lands
                let parent_inode = self.inodes[parent].clone();
                if parent_inode.visited {
                    println!("lookup - short-circuiting cache miss");
                    reply.error(ENOENT);
                } else {
                    let child_path_str = format!("{}/{}", parent_inode.path, name);
                    let child_path = Path::new(&child_path_str);
                    match self.nfs.lookup(&child_path) {
                        Ok(child_metadata) => {
                            let inode = self.insert_metadata(&child_path, &child_metadata);
                            reply.entry(&DEFAULT_TTL, &inode.attr, 0)
                        }
                        Err(err) => {
                            println!("lookup error - {}", err);
                            reply.error(ENOENT);
                        }
                    }
                }
            }
        }
    }

    // Return the cached inode
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match self.inodes.get(ino) {
            Some(inode) => reply.attr(&DEFAULT_TTL, &inode.attr),
            None => {
                println!("getattr ENOENT: {}", ino);
                reply.error(ENOENT);
            }
        };
    }

    // If the data cache for this ino not warm, call the network read to populated the cache
    // then use the offset and size to return the right part of the cached data
    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, size: u32, reply: ReplyData) {
        println!("read(ino={}, fh={}, offset={}, size={})", ino, _fh, offset, size);

        // Determine if we should hit the API
        if let Err(err) = self.read_to_cache_if_needed(ino) {
            return reply.error(err);
        }

        // Return the cached data
        let ref buffer = self.cache.get(&ino).unwrap().data;
        let end_offset = offset + size as u64;
        match buffer.len() {
            len if len as u64 > offset + size as u64 => {
                reply.data(&buffer[(offset as usize)..(end_offset as usize)]);
            }
            len if len as u64 > offset => {
                reply.data(&buffer[(offset as usize)..]);
            }
            len => {
                println!("attempted read beyond buffer for ino {} len={} offset={} size={}", ino, len, offset, size);
                reply.error(ENOENT);
            }
        }
    }

    // TODO: properly support offset
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        if offset > 0 {
            reply.ok();
            return;
        }

        let parent_ino = match ino {
            1 => 1,
            _ => self.inodes.parent(ino).expect("inode has no parent").attr.ino,
        };

        reply.add(ino, 0, FileType::Directory, ".");
        reply.add(parent_ino, 1, FileType::Directory, "..");

        let dir_visited  = self.inodes.get(ino).map(|n| n.visited).unwrap_or(false);
        match dir_visited {
            // read directory from cache
            true =>  {
                for (i, next) in self.cache_readdir(ino).enumerate().skip(offset as usize) {
                    match next {
                        Ok((filename, attr)) => {
                            reply.add(attr.ino, i as u64 + offset + 2, attr.kind, &filename);
                        }
                        Err(err) => { return reply.error(err); }
                    }
                }
            },
            // read directory from cache
            false => {
                // FIXME: sometimes cloning is just easier than fixing borrows
                let parent_path = self.inodes[ino].path.clone();
                for (i, next) in self.nfs.readdir(&Path::new(&parent_path)).enumerate().skip(offset as usize) {
                    match next {
                        Ok((filename, meta)) => {
                            let child_path = format!("{}/{}", parent_path, filename);
                            let inode = self.insert_metadata(&Path::new(&child_path), &meta);
                            reply.add(inode.attr.ino, i as u64 + offset + 2, inode.attr.kind, &filename);
                        }
                        Err(err) => { return reply.error(err); }
                    }
                }
            }
        };
        reply.ok();
    }

    fn mknod(&mut self, _req: &Request, parent: u64, name: &Path, _mode: u32, _rdev: u32, reply: ReplyEntry) {
        let name = name.to_string_lossy();
        println!("mknod(parent={}, name={}, mode=0o{:o})", parent, name, _mode);

        // TODO: check if we have write access to this parent (or does the FS do that for us)
        // or maybe some `self.nfs.allow_mknod(&path)

        let path = format!("{}/{}", self.inodes[parent].path, name);
        let now = time::now_utc().to_timespec();

        let meta = Metadata {
            size: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::RegularFile,
            perm: _mode as u16,  // TODO: should this be based on _mode or parent -x bits (e.g. & 0o666)
        };

        // FIXME: cloning because it's quick-and-dirty
        let attr = self.insert_metadata(&Path::new(&path), &meta).attr.clone();

        // Need to add an entry and declare it warm, so that empty files can be created on release/fsync
        //   but don't increment opened handles until `open` is called
        let mut entry = self.cache.entry(attr.ino).or_insert_with(|| CacheEntry::new());
        entry.warm = true;

        // TODO: figure out when/if I should be using a generation number:
        //       https://github.com/libfuse/libfuse/blob/842b59b996e3db5f92011c269649ca29f144d35e/include/fuse_lowlevel.h#L78-L91
        reply.entry(&DEFAULT_TTL, &attr, 0);
    }

    fn mkdir(&mut self, _req: &Request, parent: u64, name: &Path, _mode: u32, reply: ReplyEntry) {
        let name = name.to_string_lossy();
        println!("mkdir(parent={}, name={}, mode=0o{:o})", parent, name, _mode);

        let path_str = format!("{}/{}", self.inodes[parent].path, name);
        let path = Path::new(&path_str);
        match self.nfs.mkdir(&path) {
            Ok(_) => {
                let now = time::now_utc().to_timespec();
                let meta = Metadata {
                    size: 0,
                    atime: now,
                    mtime: now,
                    ctime: now,
                    crtime: now,
                    kind: FileType::Directory,
                    perm: _mode as u16,  // TODO: should this be based on _mode or parent
                };

                let attr = self.insert_metadata(&path, &meta).attr;

                // TODO: figure out when/if I should be using a generation number:
                //       https://github.com/libfuse/libfuse/blob/842b59b996e3db5f92011c269649ca29f144d35e/include/fuse_lowlevel.h#L78-L91
                reply.entry(&DEFAULT_TTL, &attr, 0);
            }
            Err(err) => {
                println!("mkdir error - {}", err);
                reply.error(err);
            }
        }
    }

    fn open (&mut self, _req: &Request, ino: u64, flags: u32, reply: ReplyOpen) {
        println!("open(ino={}, flags=0x{:x})", ino, flags);
        // match flags & O_ACCMODE => O_RDONLY, O_WRONLY, O_RDWR

        let mut entry = self.cache.entry(ino).or_insert_with(|| CacheEntry::new());
        entry.opened();
        reply.opened(0, flags);
    }

    fn release (&mut self, _req: &Request, ino: u64, fh: u64, flags: u32, _lock_owner: u64, flush: bool, reply: ReplyEmpty) {
        println!("release(ino={}, fh={}, flags=0x{:x}, flush={})", ino, fh, flags, flush);

        let handles = self.cache.get_mut(&ino).unwrap().released();

        // Until a delayed commit is working, also write-on-close
        if handles == 0 {
            if let Err(err) = self.flush_cache_if_needed(ino) {
                println!("release flush error - {}", err);
            }
        }

        let &CacheEntry {sync, warm, ..} = self.cache.get(&ino).unwrap();
        if handles == 0 && (sync || !warm) {
            println!("release is purging {} from cache", ino);
            let _ = self.cache.remove(&ino);
        }

        reply.ok();
    }

    fn fsync (&mut self, _req: &Request, ino: u64, fh: u64, datasync: bool, reply: ReplyEmpty) {
        println!("fsync(ino={}, fh={}, datasync={})", ino, fh, datasync);

        match self.flush_cache_if_needed(ino) {
            Ok(_) => reply.ok(),
            Err(err) => {
                println!("fsync error - {}", err);
                reply.error(EIO);
            }
        }
    }

    fn write (&mut self, _req: &Request, ino: u64, fh: u64, offset: u64, data: &[u8], flags: u32, reply: ReplyWrite) {
        // TODO: check if in read-only mode: EROFS
        println!("write(ino={}, fh={}, offset={}, len={}, flags=0x{:x})", ino, fh, offset, data.len(), flags);

        let is_replace = (offset == 0) && (self.inodes.get(ino).unwrap().attr.size < data.len() as u64);

        // Skip data lookup if write entirely replaces file or if we already cached the API response.
        if !is_replace {
            // Determine if we should hit the API
            if let Err(err) = self.read_to_cache_if_needed(ino) {
                return reply.error(err);
            }
        }

        let new_size = match self.cache.get_mut(&ino) {
            Some(ref mut entry) => {
                let end = data.len() + offset as usize;
                if end > self.inodes[ino].attr.size as usize {
                    entry.data.resize(end, 0);
                }
                entry.write(offset, &data);
                reply.written(data.len() as u32);
                entry.data.len() as u64
            }
            None => {
                println!("write failed to read file");
                reply.error(ENOENT);
                return;
            }
        };

        let ref mut inode = self.inodes[ino];
        inode.attr.size = new_size;
    }

    fn setattr (&mut self, _req: &Request, ino: u64, _mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, _atime: Option<Timespec>, _mtime: Option<Timespec>, _fh: Option<u64>, _crtime: Option<Timespec>, _chgtime: Option<Timespec>, _bkuptime: Option<Timespec>, flags:               Option<u32>, reply: ReplyAttr) {
        println!("setattr(ino={}, mode={:?}, size={:?}, fh={:?}, flags={:?})", ino, _mode, size, _fh, flags);
        match self.inodes.get_mut(ino) {
            Some(mut inode) => {
                if let Some(new_size) = size {
                    inode.attr.size = new_size;
                }
                if let Some(new_uid) = uid {
                    inode.attr.uid = new_uid;
                }
                if let Some(new_gid) = gid {
                    inode.attr.gid = new_gid;
                }
                // TODO: is mode (u32) equivalent to attr.perm (u16)?
                reply.attr(&DEFAULT_TTL, &inode.attr);
            }
            None => reply.error(ENOENT)
        }
    }

    fn rmdir(&mut self, _req: &Request, parent: u64, name: &Path, reply: ReplyEmpty) {
        let name = name.to_string_lossy();
        println!("rmdir(parent={}, name={})", parent, name);

        let ino_opt = self.inodes.child(parent, &name).map(|inode| inode.attr.ino);
        let path = format!("{}/{}", self.inodes[parent].path, name);
        match self.nfs.rmdir(&Path::new(&path)) {
            Ok(_) => {
                ino_opt.map(|ino| {
                    self.inodes.remove(ino);
                    self.cache.remove(&ino);
                });
                reply.ok()
            },
            Err(err) => {
                println!("rmdir failed: {}", err);
                reply.error(EIO);
            }
        }
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &Path, reply: ReplyEmpty) {
        let name = name.to_string_lossy();
        println!("unlink(parent={}, name={})", parent, name);

        let ino_opt = self.inodes.child(parent, &name).map(|inode| inode.attr.ino);
        let path = format!("{}/{}", self.inodes[parent].path, name);
        match self.nfs.unlink(&Path::new(&path)) {
            Ok(_) => {
                ino_opt.map(|ino| {
                    self.inodes.remove(ino);
                    self.cache.remove(&ino);
                });
                reply.ok()
            },
            Err(err) => {
                println!("Delete failed: {}", err);
                reply.error(EIO);
            }
        }
    }

}

