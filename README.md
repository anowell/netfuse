# algorithmia-fuse
Experimental: FUSE-based Algorithmia FileSystem

A user-mode virtual filesystem backed by the Algorithmia API. Basically, it handles filesystem requests by turning them into API calls and lazily building a local cache of remote resources. The end result is that you can mount Algorithmia data to a local directory, and use standard file operations to work with Algorithmia data.

Screenshots demonstrate basic traversal and read operations from CLI and file explorer:

![Screenshot](https://dl.dropboxusercontent.com/u/39033486/Algorithmia/algofs-walk-and-grep.png)

![Screenshot](https://dl.dropboxusercontent.com/u/39033486/Algorithmia/algofs-reading-files.png)

## Status

Currently, this is only an experimental filesystem. You should NOT rely on it for critical work.

In it's current state, it works as a basic read-write filesystem with several caveats:

- Connector support is too limited to be useful, and better support is blocked by the API - see [Issue #1](../../issues/1))
- Writes persist when closing the last open handle to a file. If the close fails, it's likely the data isn't persisted.
- Directory listing is permanently cached, so if you change a directory's contents outside of AlgoFS, you have to unmount and remount AlgoFS before those changes appear.
- The entire inode and file cache lives in RAM, so if you download a 4GB file, it will occupy 4GB of RAM until it is closed.
- Testing so far is very limited.
- General network filesystem caveats apply, e.g. some file operations may appear slow

See [issues](../../issues) for the full list of known issues. For any unexpected or surprising behavior,
please [file an issue](https://github.com/anowell/algorithmia-fuse/issues/new).


## Build, Test, Run, Debug

To build and test (tests are pretty barebones):
```
$ cargo build
$ cargo test
```

To mount the filesystem:
```
$ mkdir ~/algofs
$ target/debug/algofs ~/algofs
```

The `algofs` executable will print all the current debug output,
so currently it works best to browse the `~/algofs` from another terminal.

Note: some shell enhancements can cause a lot of extra listing operations.
And file explorers may trigger a lot of extra reads to preload or preview files.

To stop algofs, unmount it as root. (Note: killing `algofs` will stop request handling, but leaves `~/algofs` as a volume with no transport connected).
```
fusermount -u ~/algofs
# or `sudo umount ~/algofs`
```

