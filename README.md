
# Chicon

[![Version](https://img.shields.io/crates/v/chicon.svg)](https://crates.io/crates/chicon)
[![Documentation](https://docs.rs/chicon/badge.svg)](https://docs.rs/chicon)

A file abstraction system for Rust.  Chicon is a library meant to provide a simple, uniform and universal API interacting with any filesystem. Chicon behave as an abstraction layer providing traits, types and methods. The main `FileSystem` trait is based on the usage of [`std::fs::*`](https://doc.rust-lang.org/stable/std/fs/) in order to be transparent when switching from a physical filesystem to a virtual one like S3, SFTP, SSH or in-memory. Chicon is suitable for any situation where storing directories and files on different filesystems is needed. Memory file system can be appropriate to write tests in order to have a faster execution than with an IO filesystem.

## Examples

### Use S3 as backend to create a file

```rust
use std::io::prelude::*;
use chicon::{DirEntry, File, FileSystem, S3FileSystem};
let s3_fs = S3FileSystem::new(
     String::from("my_access_key_id"),
     String::from("secret_access_key"),
     String::from("my_bucket"),
     String::from("my_region"),
     String::from("http://127.0.0.1"), // endpoint
);
let mut file = s3_fs.create_file("test.test").unwrap()
file.write_all(String::from("here is a test").as_bytes()).unwrap();
file.sync_all().unwrap();
let mut content: String = String::new();
file.read_to_string(&mut content).unwrap();
assert_eq!(content, String::from("here is a test"));
s3_fs.remove_file("test.test").unwrap(); // To delete the file
```

### Use SFTP as backend to create a file

> You just need to change from `S3FileSystem::new` to `SFTPFileSystem::new`.

```rust
use std::io::prelude::*;
use chicon::{DirEntry, File, FileSystem, SFTPFileSystem};
let sftp_fs = SFTPFileSystem::new(
    String::from("127.0.0.1:2222"), // host:port
    String::from("foo"), // user
    None, // Some("passphrase") if you have a passphrase configured on your ssh key
    "/Users/foo/.ssh/my_private_key", // ABSOLUTE path to private key
    "/Users/foo/.ssh/my_public_key.pub" // ABSOLUTE path to public key
);
let mut file = sftp_fs.create_file("test.test").unwrap()
file.write_all(String::from("here is a test").as_bytes()).unwrap();
file.sync_all().unwrap();
let mut content: String = String::new();
file.read_to_string(&mut content).unwrap();
assert_eq!(content, String::from("here is a test"));
```

### Use SSH as backend to read a file

```rust
use std::io::prelude::*;
use chicon::{DirEntry, File, FileSystem, SSHFileSystem};
let ssh_fs = SSHFileSystem::new(
    String::from("127.0.0.1:2222"), // host:port
    String::from("foo"), // user
    None, // Some("passphrase") if you have a passphrase configured on your ssh key
    "/Users/foo/.ssh/my_private_key", // ABSOLUTE path to private key
    "/Users/foo/.ssh/my_public_key.pub" // ABSOLUTE path to public key
);
let mut file = ssh_fs.open_file("share/myfile.txt").unwrap();
let mut buffer = String::new();
file.read_to_string(&mut buffer).unwrap();
println!("Here is the content of your file: {}", buffer);
```

### Use OS (local filesystem) as backend to create and read a directory

```rust
use std::io::prelude::*;
use chicon::{DirEntry, File, FileType, FileSystem, OsFileSystem};
let os_fs = OsFileSystem::new();
os_fs.create_dir_all("testreaddir/test").unwrap();
os_fs.create_file("testreaddir/mytest.test").unwrap();
let dir_entries = os_fs.read_dir("testreaddir").unwrap();
assert!(!dir_entries.is_empty())
assert_eq!(dir_entries.len(), 2)
assert_eq!(
    dir_entries.get(0).unwrap().path().unwrap(),
    PathBuf::from("testreaddir/test")
);
assert_eq!(
    dir_entries.get(0).unwrap().file_type().unwrap(),
    FileType::Directory
);
std::fs::remove_dir_all("testreaddir").unwrap(); // To remove dir and all entries inside
```

> If you need more examples, checkout the tests in the source code on the GitHub repository.

## Roadmap

+ implement swift as a new backend
+ refactor with more idiomatic Rust stuff
