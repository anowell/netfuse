use fuse::FileType;
use libc::{self, ENOSYS};
use time::Timespec;
use std::path::Path;


pub type LibcError = libc::c_int;

pub struct Metadata {
    pub size: u64,
    pub atime: Timespec,
    pub mtime: Timespec,
    pub ctime: Timespec,
    pub crtime: Timespec,
    pub kind: FileType,
    pub perm: u16,
}

/// Represents a single entry of a directory listing
///   where the first element is the filename, and the second is the file metadata
pub type DirEntry = (String, Metadata);

pub trait NetworkFilesystem {

    /// Any arbitrary code to run when mounting
    ///
    /// May optionally return a Vec of Metadata that will be
    ///   pre-populated into the inode store
    fn init(&mut self) -> Result<Vec<Metadata>, LibcError> {
        Err(ENOSYS)
    }

    /// Returns the metadata for a file or directory associated with a given
    fn lookup(&mut self, _path: &Path) -> Result<Metadata, LibcError> {
        Err(ENOSYS)
    }

    /// Reads the contents of a file associated with a given path
    ///
    /// This is called on the first filesystem attempt to `read` a file,
    ///   since reading happens in chunks, the underlying `FileSystem` implemenation
    ///   will cache the result returned and read it in
    ///
    /// The cached data will be freed when there are no remaining open handles on this file.
    ///
    /// Note: the cache implementation will likely change to keep data around a bit longer
    ///   though I expect such changes will come with configurable parameters
    fn read(&mut self, _path: &Path, _buffer: &mut Vec<u8> ) -> Result<usize, LibcError> {
        Err(ENOSYS)
    }

    /// Write data back to the network backend
    ///
    /// This is not actually called when the filesystem calls `write`,
    ///   rather on `fsync` or during `release` when for a handle that modified the cached copy
    ///
    /// Note: the `release` implementation might change in the future for a configurable defferred commit
    ///
    /// Will only be called if a previous `lookup` has confirmed a file exists at this path
    /// This is also only called if the volume is mounted with the `rw` option
    fn write(&mut self, _path: &Path, _data: &[u8]) -> Result<(), LibcError> {
        Err(ENOSYS)
    }

    /// List contents of a directory
    ///
    /// The returned iterator should iterate of a tuple pairing of filename with file metadata
    ///
    /// TODO: return `impl Iterator<Item=Result<DirEntry, LibcError>>` once `impl Trait` lands in nightly
    fn readdir(&mut self, _path: &Path) -> Box<Iterator<Item=Result<DirEntry, LibcError>>> {
        Box::new(vec![Err(ENOSYS)].into_iter())
    }

    /// Creates an empty directory for the given path
    ///
    /// This is only called if the volume is mounted with the `rw` option
    fn mkdir(&mut self, _path: &Path) -> Result<(), LibcError> {
        Err(ENOSYS)
    }

    /// Removes the directory that corresponds to a given path
    ///
    /// This is called without any assumption to the directory being empty or not
    /// Generally, you'll want to return ENOTEMPTY if the directory is not empty
    ///
    /// This is only called if the volume is mounted with the `rw` option
    fn rmdir(&mut self, _path: &Path) -> Result<(), LibcError> {
        Err(ENOSYS)
    }

    /// Removes the file that corresponds to a given path
    ///
    /// This is only called if the volume is mounted with the `rw` option
    fn unlink(&mut self, _path: &Path) -> Result<(), LibcError>{
        Err(ENOSYS)
    }

}