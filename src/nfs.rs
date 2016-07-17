use fuse::FileType;
use libc::{self, ENOSYS};
use time::Timespec;
use std::path::Path;
use std::ffi::{OsStr, OsString};

/// libc Error Code
pub type LibcError = libc::c_int;

/// Metadata representing a file
pub struct Metadata {
    pub size: u64,
    pub atime: Timespec,
    pub mtime: Timespec,
    pub ctime: Timespec,
    pub crtime: Timespec,
    pub kind: FileType,
    pub perm: u16,
}

/// Entry from a directory listing
pub struct DirEntry {
    pub filename: OsString,
    pub metadata: Metadata,
}

impl DirEntry {
    pub fn new<S: AsRef<OsStr>>(filename: S, metadata: Metadata) -> DirEntry {
        DirEntry { filename: filename.as_ref().to_owned(), metadata: metadata }
    }
}


/// Trait to implement to provide a backend store for a `NetFuse` filesystem
///
/// The methods in this trait are abstractions over the low level method provided
/// by the FUSE library's `Filesystem` trait. The `NetFuse` implementation of `FileSystem`
/// will manage many of the low-level details like inode numbers, offsets, sizes,
/// and lazy persistence.
///
/// In doing so, implementing a backend store for `NetFuse` (ie. implementing `NetworkFilesystem`)
/// is mostly a matter of making network network calls that map to very common filesystem operations.
///
/// The default implementation is just enough to mount a filesystem that supports no read or write operations
pub trait NetworkFilesystem {

    /// Any arbitrary code to run when mounting
    ///
    /// Returning an error will result in an error during mounting (TODO: verify)
    fn init(&mut self) -> Result<(), LibcError> {
        Ok(())
    }

    /// Returns the metadata for a file or directory associated with a given
    ///
    /// This typically corresponds to operations like `stat`.
    /// This method is called when the `NetFuse` inode store did not find
    /// cached inode data for this `path`. Any returned `Metadata` will be heavily cached.
    ///
    /// See `man 2 stat` for more information including appropriate errors to return.
    fn lookup(&mut self, _path: &Path) -> Result<Metadata, LibcError> {
        Err(ENOSYS)
    }

    /// Reads the contents of a file associated with a given path
    ///
    /// This is called on the first filesystem attempt to `read` a file,
    ///   since reading happens in chunks, the underlying `NetFuse` implemenation
    ///   will cache the result returned and read it in from the cache in chunks
    ///   without additional calls to this method.
    ///
    /// The cached data will be freed when there are no remaining open handles on this file.
    ///
    /// See `man 2 read` for more information including appropriate errors to return.
    fn read(&mut self, _path: &Path, _buffer: &mut Vec<u8> ) -> Result<usize, LibcError> {
        Err(ENOSYS)
    }

    /// Write data back to the network backend
    ///
    /// This is not actually called when the filesystem calls `write`.
    ///   Instead this is called when `NetFuse` handles an `fsync`
    ///   or during `release` for a file handle that modified the cached copy.
    ///
    /// Note: the `release` implementation might change in the future in favor of
    ///   a configurable defferred commit
    ///
    /// This method will only be called if:
    /// - a previous `lookup` has confirmed a file exists at this path
    /// - the volume was mounted with the `rw` option
    ///
    /// See `man 2 fsync` for more information including appropriate errors to return.
    fn write(&mut self, _path: &Path, _data: &[u8]) -> Result<(), LibcError> {
        Err(ENOSYS)
    }

    /// List contents of a directory
    ///
    /// This method should return an iterator over the contents of the directory
    ///   specified by `path`. By returning an iterator, `NetFuse` can begin listing
    ///   contents sooner in the cases where listing may require multiple paged network requests.
    ///
    /// These value will be cached to prevent additional listing. The current cache implementation
    ///   will likely change as it is not friendly to cases where the data changes outside the filesystem.
    ///   Until then, unmount and re-mount the volume to clear the directory listing cache.
    ///
    /// See `man 2 readdir` for more information including appropriate errors to return.
    ///
    /// Note: this method will likely return `impl Iterator<Item=Result<DirEntry, LibcError>>` once `impl Trait` lands in nightly
    fn readdir(&mut self, _path: &Path) -> Box<Iterator<Item=Result<DirEntry, LibcError>>> {
        Box::new(vec![Err(ENOSYS)].into_iter())
    }

    /// Creates an empty directory for the given path
    ///
    /// This method is only called if:
    /// - a previous `lookup` has confirmed the parent path was a directory
    /// - the volume is mounted with the `rw` option
    ///
    /// See `man 2 mkdir` for more information including appropriate errors to return.
    fn mkdir(&mut self, _path: &Path) -> Result<(), LibcError> {
        Err(ENOSYS)
    }

    /// Removes the directory that corresponds to a given path
    ///
    /// This method is only called if:
    /// - a previous `lookup` has confirmed a directory exists at this path
    /// - the volume is mounted with the `rw` option
    ///
    /// See `man 2 rmdir` for more information including appropriate errors to return.
    ///   Namely: you'll generally want to return ENOTEMPTY if the directory is not empty
    fn rmdir(&mut self, _path: &Path) -> Result<(), LibcError> {
        Err(ENOSYS)
    }

    /// Removes the file that corresponds to a given path
    ///
    /// This method is only called if:
    /// - a previous `lookup` has confirmed a file exists at this path
    /// - the volume is mounted with the `rw` option
    ///
    /// See `man 2 unlink` for more information including appropriate errors to return.
    fn unlink(&mut self, _path: &Path) -> Result<(), LibcError>{
        Err(ENOSYS)
    }

}