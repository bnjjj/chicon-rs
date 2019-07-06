use std::env;
use std::fs::Permissions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use url::percent_encoding::{utf8_percent_encode, SIMPLE_ENCODE_SET};

use serde::{Serialize, Deserialize};
use futures::future::Future;
use osauth::{
    from_env, services::ObjectStorageService, services::OBJECT_STORAGE, Adapter, AuthType,
};

use crate::error::ChiconError;
use crate::{DirEntry, File, FileSystem, FileType};

define_encode_set! {
    pub QUERY_ENCODE_SET = [SIMPLE_ENCODE_SET] | {' ', '"', '#', '<', '>'}
}

pub struct SwiftFileSystem {
    adapter: Adapter<ObjectStorageService>,
    account: String,
    container: String,
}
// For v3 authentication
// OS_AUTH_URL - Auth URL
// OS_USERNAME - UserName for api
// OS_USER_ID - User Id
// OS_PASSWORD - Key for api access
// OS_USER_DOMAIN_NAME - User's domain name
// OS_USER_DOMAIN_ID - User's domain Id
// OS_PROJECT_NAME - Name of the project
// OS_PROJECT_DOMAIN_NAME - Name of the tenant's domain, only needed if it differs from the user domain
// OS_PROJECT_DOMAIN_ID - Id of the tenant's domain, only needed if it differs the from user domain
// OS_TRUST_ID - If of the trust
// OS_REGION_NAME - Region to use - default is use first region
impl SwiftFileSystem {
    /// Create a swift file system passing the right credentials informations in parameters
    pub fn new(
        account: String,
        container: String,
        auth_url: String,
        username: String,
        password: String,
        project_name: String,
    ) -> Result<Self, ChiconError> {
        env::set_var("OS_AUTH_URL", auth_url);
        env::set_var("OS_USERNAME", username);
        env::set_var("OS_PASSWORD", password);
        env::set_var("OS_PROJECT_NAME", project_name);

        let adapter = from_env()?.into_adapter(OBJECT_STORAGE);
        let container_query = ContainerQuery{};
        adapter.post_json(&[&account, &container], container_query, None).wait()?;

        Ok(SwiftFileSystem { account, container, adapter })
    }

    /// Create a swift file system based on environment variable OS_*
    pub fn new_from_env(account: String, container: String) -> Result<Self, ChiconError> {
        let adapter = from_env()?.into_adapter(OBJECT_STORAGE);
        let container_query = ContainerQuery{};
        adapter.post_json(&[&account, &container], container_query, None).wait()?;

        Ok(SwiftFileSystem { account, container, adapter })
    }
}

impl FileSystem for SwiftFileSystem {
    type FSError = ChiconError;
    type File = SwiftFile;
    type DirEntry = SwiftDirEntry;

    fn chmod<P: AsRef<Path>>(&self, path: P, perm: Permissions) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        
        unimplemented!()
    }
    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        let object_path = utf8_percent_encode(
            path.to_str().ok_or(ChiconError::BadPath)?,
            QUERY_ENCODE_SET,
        ).to_string();
        let object_req = ObjectQuery {};
        let object_res: Object = self.adapter.post_json(&[&self.account, &self.container, &object_path], object_req, None).wait()?;
    }
    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        let from = from.as_ref();
        let to = to.as_ref();
        unimplemented!()
    }
}

/// Structure implementing File trait to represent a file on a swift filesystem
pub struct SwiftFile {
    account: String,
    container: String,
    filename: PathBuf,
    // Maybe add url to upload
    content: Vec<u8>
}
impl SwiftFile {
    fn new(account: String, container: String, filename: PathBuf) -> Self {
        SwiftFile {
            account,
            container,
            filename,
            content: Vec::new()
        }
    }
}
impl File for SwiftFile {
    type FSError = ChiconError;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        unimplemented!()
    }
}

impl Read for SwiftFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut content_slice = self.content.as_slice();
        let nb = content_slice.read(buf)?;
        self.content = content_slice.to_vec();
        Ok(nb)
    }
}
impl Write for SwiftFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.content.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.content.flush()
    }
}

/// Structure implementing `DirEntry` trait to represent an entry in a directory on a swift filesystem
pub struct SwiftDirEntry(std::fs::DirEntry);
impl DirEntry for SwiftDirEntry {
    type FSError = ChiconError;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        unimplemented!()
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        unimplemented!()
    }
}

// type Object struct {
//     Name               string     `json:"name"`          // object name
//     ContentType        string     `json:"content_type"`  // eg application/directory
//     Bytes              int64      `json:"bytes"`         // size in bytes
//     ServerLastModified string     `json:"last_modified"` // Last modified time, eg '2011-06-30T08:20:47.736680' as a string supplied by the server
//     LastModified       time.Time  // Last modified time converted to a time.Time
//     Hash               string     `json:"hash"` // MD5 hash, eg "d41d8cd98f00b204e9800998ecf8427e"
//     PseudoDirectory    bool       // Set when using delimiter to show that this directory object does not really exist
//     SubDir             string     `json:"subdir"` // returned only when using delimiter to mark "pseudo directories"
//     ObjectType         ObjectType // type of this object
// }
#[derive(Serialize, Deserialize)]
struct Object {
    name: String,
    content_type: String,
    bytes: i64,
    last_modified: chrono::DateTime<chrono::Utc>,
    hash: String,
    sub_dir: String
}

#[derive(Serialize, Deserialize)]
struct ObjectQuery {
    name: String,
    count: i64,
    bytes: i64
}

#[derive(Serialize, Deserialize)]
struct ContainerQuery {
    limit: i32,
    prefix: String,
    marker: String,
    end_marker: String,
    headers: HashMap<String, String>,
}