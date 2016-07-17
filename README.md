# netfuse
Experimental: FUSE-based abstraction for networked filesystems

This library provides a wrapper around the pure [rust rewrite of libfuse](https://github.com/zargony/rust-fuse).
It provides an internally managed inode cache that allows abstracting FS operations into operations on paths.
It is designed with the assumption that the backing store is over a network,
so the implementation relies heavily on caching and lazy writing to improve perceived performance.

[Documentation](https://anowell.github.io/netfuse/netfuse/)

## Implementations

This was originally ripped out of the implementation of algorithmia-fuse mentioned below.

- [algorithmia-fuse](https://github.com/anowell/algorithmia-fuse) - filesystem for managing data through the Algorithmia platform

If you build something with it, open a PR or file an issue to get it added here. :-)

## Current caveats

I wouldn't recommend this for any production-quality filesystem today. These are some known caveats:

- Writes persist when closing the last open handle to a file. If the close fails, it's likely the data isn't persisted.
- The entire inode and file cache lives in RAM, so if you download a 4GB file, it will occupy 4GB of RAM until it is closed.
- Directory listing is permanently cached, so if you change a directory's contents outside of the FS, you have to unmount and remount before those changes appear.
- Testing while mounted has been limited to a handful of common I/O scenarios
- General network filesystem caveats apply, e.g. some file operations may appear slow
- Implementing `readdir` will hopefully be much nicer after [impl Trait](https://github.com/rust-lang/rust/issues/34511) lands

Please [file an issue](https://github.com/anowell/netfuse/issues/new) or create a pull request
if you run into any issue or limitation using this library.


## Build, Test

To build and test:
```
$ cargo build
$ cargo test
```
