# Library to read unencrypted .3ds file
This library allow you

```rust
let file = File::open("rom.3ds").unwrap(); // get an access to an unencrypted romfs file
let _romfs_vfs = get_romfs_vfs(file).unwrap(); // get a vfs::VFS object to access the rom read only
```

For more information on how to use the returned vfs object, read it's documentation: https://docs.rs/vfs/0.2.1/vfs/trait.VFS.html.
