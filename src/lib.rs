//! A file abstraction system for Rust
//!
//! Chicon is a library intends to provide a simple,
//! uniform and universal API interacting with any filesystem,
//! as an abstraction layer providing traits, types and methods.
//! otherwise link to an installed copy.
//!
//! The main `FileSystem` trait is based on the usage of `std::fs::*`
//! in order to be transparent when you want to switch from a physical filesystem
//! to a virtual filesystem like S3, SFTP, SSH, in-memory.
//!
//! Memory file system can be appropriate when you write your tests in order to have faster behavior than an IO filesystem.
//!
//! It is suitable for any situation when you need to store directories and files
//! on different filesystems.
//!
//! # Examples
//!
//! ## Use S3 as backend to create a file
//!
//! ```should_panic
//! use std::io::prelude::*;
//!
//! use chicon::{DirEntry, File, FileSystem, S3FileSystem};
//!
//! let s3_fs = S3FileSystem::new(
//!      String::from("my_access_key_id"),
//!      String::from("secret_access_key"),
//!      String::from("my_bucket"),
//!      String::from("my_region"),
//!      String::from("http://127.0.0.1"), // endpoint
//! );
//! let mut file = s3_fs.create_file("test.test").unwrap()
//!
//! file.write_all(String::from("here is a test").as_bytes()).unwrap();
//! file.sync_all().unwrap();
//!
//! let mut content: String = String::new();
//! file.read_to_string(&mut content).unwrap();
//! assert_eq!(content, String::from("here is a test"));
//!
//! s3_fs.remove_file("test.test").unwrap(); // If you want to delete the file
//! ```
//!
//! ## Use SFTP as backend to create a file
//!
//! You just need to change from `S3FileSystem::new` to `SFTPFileSystem::new`.
//!
//! ```should_panic
//! use std::io::prelude::*;
//!
//! use chicon::{DirEntry, File, FileSystem, SFTPFileSystem};
//!
//! let sftp_fs = SFTPFileSystem::new(
//!     String::from("127.0.0.1:2222"), // host:port
//!     String::from("foo"), // user
//!     None, // Some("passphrase") if you have a passphrase configured on your ssh key
//!     "/Users/foo/.ssh/my_private_key", // ABSOLUTE path to private key
//!     "/Users/foo/.ssh/my_public_key.pub" // ABSOLUTE path to public key
//! );
//! let mut file = sftp_fs.create_file("test.test").unwrap()
//!
//! file.write_all(String::from("here is a test").as_bytes()).unwrap();
//! file.sync_all().unwrap();
//!
//! let mut content: String = String::new();
//! file.read_to_string(&mut content).unwrap();
//! assert_eq!(content, String::from("here is a test"));
//!
//! ```
//!
//! ## Use SSH as backend to read a file
//!
//! ```should_panic
//! use std::io::prelude::*;
//!
//! use chicon::{DirEntry, File, FileSystem, SSHFileSystem};
//!
//! let ssh_fs = SSHFileSystem::new(
//!     String::from("127.0.0.1:2222"), // host:port
//!     String::from("foo"), // user
//!     None, // Some("passphrase") if you have a passphrase configured on your ssh key
//!     "/Users/foo/.ssh/my_private_key", // ABSOLUTE path to private key
//!     "/Users/foo/.ssh/my_public_key.pub" // ABSOLUTE path to public key
//! );
//! let mut file = ssh_fs.open_file("share/myfile.txt").unwrap();
//! let mut buffer = String::new();
//! file.read_to_string(&mut buffer).unwrap();
//!
//! println!("Here is the content of your file: {}", buffer);
//! ```
//!
//!
//! ## Use OS (local filesystem) as backend to create and read a directory
//!
//! ```should_panic
//! use std::io::prelude::*;
//!
//! use chicon::{DirEntry, File, FileType, FileSystem, OsFileSystem};
//!
//! let os_fs = OsFileSystem::new();
//! os_fs.create_dir_all("testreaddir/test").unwrap();
//! os_fs.create_file("testreaddir/mytest.test").unwrap();
//!
//! let dir_entries = os_fs.read_dir("testreaddir").unwrap();
//! assert!(!dir_entries.is_empty())
//! assert_eq!(dir_entries.len(), 2)
//! assert_eq!(
//!     dir_entries.get(0).unwrap().path().unwrap(),
//!     PathBuf::from("testreaddir/test")
//! );
//! assert_eq!(
//!     dir_entries.get(0).unwrap().file_type().unwrap(),
//!     FileType::Directory
//! );
//!
//! std::fs::remove_dir_all("testreaddir").unwrap(); // If you want to remove dir and all entries inside
//! ```
//!
//! If you need more examples, check-out all tests in the source code on Github
//!
#![doc(html_logo_url = "https://github.com/bnjjj/chicon-rs/blob/master/chicon_logo.png?raw=true")]
extern crate rusoto_core;
extern crate rusoto_s3;
extern crate ssh2;
#[macro_use]
extern crate url;
extern crate chrono;
extern crate env_logger;
extern crate osauth;
extern crate serde;
extern crate tokio;
#[macro_use]
extern crate failure;

mod error;
mod mem;
mod os;
mod s3;
mod sftp;
mod ssh;
// mod swift;

use std::fs::Permissions;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub use error::ChiconError;
pub use mem::*;
pub use os::*;
pub use s3::{S3DirEntry, S3File, S3FileSystem};
pub use sftp::*;
pub use ssh::*;

///
/// The FileSystem trait needs to be implemented if you want a fully available abstract filesystem.
/// For now we have few implementations as OSFileSystem, S3FileSystem, SFTPFileSystem, SSHFileSystem, MemFileSystem
///
pub trait FileSystem {
    type FSError;
    type File: File;
    type DirEntry: DirEntry;

    fn chmod<P: AsRef<Path>>(&self, path: P, perm: Permissions) -> Result<(), Self::FSError>;
    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError>;
    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError>;
    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError>;
    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError>;
    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError>;
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError>;
    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError>;
    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError>;
    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError>;
}

/// Trait that represent a file inside our FileSystem. Associated type `File` in our `FileSystem` trait must implement this trait.
pub trait File: Read + Write + Seek {
    type FSError;

    fn sync_all(&mut self) -> Result<(), Self::FSError>;
}

/// Trait that represent a directory entry inside our FileSystem. Associated type `DirEntry` in our `FileSystem` trait must implement this trait.
pub trait DirEntry {
    type FSError;

    fn path(&self) -> Result<PathBuf, Self::FSError>;
    fn file_type(&self) -> Result<FileType, Self::FSError>;
    fn name(&self) -> Result<String, Self::FSError> {
        let path = self.path()?;
        if let Some(filename) = path.as_path().file_name() {
            return Ok(filename.to_string_lossy().into_owned());
        }

        Ok(String::new())
    }
}

/// Possible file type when you fetch directory entries
#[derive(Clone, Debug, PartialEq)]
pub enum FileType {
    Directory,
    File,
    Symlink,
}
