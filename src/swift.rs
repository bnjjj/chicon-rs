use std::fs::Permissions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use openstack;
use openstack::object_storage::Object;

use crate::error::ChiconError;
use crate::{DirEntry, File, FileSystem, FileType};

pub struct SwiftFileSystem {
    container: String,
    cloud: openstack::Cloud,
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
        auth_url: String,
        username: String,
        password: String,
        region: String,
        project_name: String,
        container: String,
    ) -> Result<Self, ChiconError> {
        let auth = openstack::auth::Password::new(&auth_url, &username, &password, "Default")?
            .with_region(region)
            .with_project_scope(project_name, "default");
        let os = openstack::Cloud::new(auth);
        os.create_container(container.clone())?;

        Ok(SwiftFileSystem {
            cloud: os,
            container,
        })
    }

    // /// Create a swift file system based on environment variable OS_*
    // pub fn new_from_env(account: String, container: String) -> Result<Self, ChiconError> {
    //     let mut runtime = Runtime::new().expect("cannot create a tokio runtime");
    //     let adapter = Adapter::from_env(OBJECT_STORAGE)?;

    //     runtime.block_on(adapter.put_empty(vec![account.clone(), container.clone()], None))?;

    //     Ok(SwiftFileSystem { account, container })
    // }
}

impl FileSystem for SwiftFileSystem {
    type FSError = ChiconError;
    type File = SwiftFile;
    type DirEntry = SwiftDirEntry;

    fn chmod<P: AsRef<Path>>(&self, _path: P, _perm: Permissions) -> Result<(), Self::FSError> {
        // let path = path.as_ref();
        unimplemented!()
    }

    // TODO: if all the path doesn't exist. Create dir before. Or check params in API
    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        self.cloud
            .create_object(
                self.container.as_ref(),
                path.to_str().unwrap(),
                "".as_bytes(),
            )
            .map(|_: Object| {
                SwiftFile::new(
                    self.cloud.clone(),
                    self.container.clone(),
                    PathBuf::from(path),
                )
            })
            .map_err(ChiconError::from)
    }
    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.cloud
            .create_object(
                self.container.as_ref(),
                path.to_str().unwrap(),
                "".as_bytes(),
            )
            .map(|_: Object| ())
            .map_err(ChiconError::from)
    }
    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        self.create_dir(path)
    }
    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        let object = self
            .cloud
            .get_object(self.container.as_ref(), path.to_str().unwrap())?;
        let mut file_content = object.download()?;
        let mut content = Vec::<u8>::new();
        file_content.read_to_end(&mut content)?;

        Ok(SwiftFile {
            cloud: self.cloud.clone(),
            container: self.container.clone(),
            filename: PathBuf::from(path),
            content,
            offset: 0,
            bytes_read: 0,
        })
    }
    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let mut path = path.as_ref().to_str().unwrap().to_string();
        if !path.ends_with('/') {
            path.push('/');
        }

        let object_query = self
            .cloud
            .find_objects(self.container.clone())
            .with_custom_query("prefix", &path)
            .with_custom_query("delimiter", "/");

        let dir_entries: Vec<SwiftDirEntry> = object_query
            .all()?
            .into_iter()
            .filter_map(|object: Object| {
                if let Some(subdir) = object.subdir() {
                    if &path == subdir {
                        None
                    } else {
                        Some(SwiftDirEntry {
                            name: PathBuf::from(subdir),
                            file_type: FileType::Directory,
                        })
                    }
                } else {
                    if &path == object.name() {
                        None
                    } else {
                        Some(SwiftDirEntry {
                            name: PathBuf::from(object.name()),
                            file_type: FileType::File,
                        })
                    }
                }
            })
            .collect();
        Ok(dir_entries)
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.cloud
            .get_object(self.container.clone(), path.to_str().unwrap())?
            .delete()
            .map_err(ChiconError::from)
            .map(|_| ())
    }

    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        let dir_entries = self.read_dir(path)?;
        if !dir_entries.is_empty() {
            Err(ChiconError::DirectoryNotEmpty)
        } else {
            self.remove_dir_all(path)
        }
    }

    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.cloud
            .get_object(self.container.clone(), path.to_str().unwrap())?
            .delete()
            .map_err(ChiconError::from)
            .map(|_| ())
    }
    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        let from = from.as_ref();
        let to = to.as_ref();
        let obj = self
            .cloud
            .get_object(self.container.clone(), from.to_str().unwrap())?;
        obj.copy(to.to_str().unwrap())?;

        obj.delete().map_err(ChiconError::from)
    }
}

/// Structure implementing File trait to represent a file on a swift filesystem
pub struct SwiftFile {
    cloud: openstack::Cloud,
    container: String,
    filename: PathBuf,
    content: Vec<u8>,
    offset: u64,
    bytes_read: u64,
}
impl SwiftFile {
    fn new(cloud: openstack::Cloud, container: String, filename: PathBuf) -> Self {
        SwiftFile {
            cloud,
            container,
            filename,
            content: Vec::new(),
            offset: 0,
            bytes_read: 0,
        }
    }
}
impl File for SwiftFile {
    type FSError = ChiconError;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        let buf = std::io::Cursor::new(self.content.clone());

        self.cloud
            .create_object(self.container.clone(), self.filename.to_str().unwrap(), buf)?;

        Ok(())
    }
}

impl Read for SwiftFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut content_slice = if self.bytes_read == 0 {
            if self.offset >= self.content.len() as u64 {
                return Ok(0);
            }
            &self.content[(self.offset as usize)..]
        } else {
            self.content.as_slice()
        };
        let nb = content_slice.read(buf)?;

        self.bytes_read += nb as u64;
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
impl Seek for SwiftFile {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, std::io::Error> {
        let err = std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid argument: bad cursor value",
        );
        match pos {
            SeekFrom::Current(nb) if self.offset as i64 + nb < self.content.len() as i64 => {
                let cursor: i64 = self.offset as i64 + nb;
                if cursor < 0 {
                    return Err(err);
                }
                self.offset = cursor as u64;
                Ok(cursor as u64)
            }
            SeekFrom::End(nb) if nb >= 0 => {
                self.offset = (self.content.len() as u64) + nb as u64;
                Ok(self.offset)
            }
            SeekFrom::End(nb) if (self.content.len() as i64) + nb >= 0 => {
                let cursor: i64 = (self.content.len() as i64) + nb;
                self.offset = cursor as u64;
                Ok(cursor as u64)
            }
            SeekFrom::Start(nb) if nb < self.content.len() as u64 => {
                self.offset = nb;
                Ok(nb)
            }
            _ => Err(err),
        }
    }
}

/// Structure implementing `DirEntry` trait to represent an entry in a directory on a swift filesystem
#[derive(Debug)]
pub struct SwiftDirEntry {
    name: PathBuf,
    file_type: FileType,
}
impl DirEntry for SwiftDirEntry {
    type FSError = ChiconError;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        Ok(self.name.clone())
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        Ok(self.file_type.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;
    use std::env;

    // #[test]
    // fn test_create_file() {
    //     env_logger::init();
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     let mut file = swift_fs.create_file("blabla/test.test").unwrap();

    //     file.write_all(String::from("pookie").as_bytes()).unwrap();
    //     file.sync_all().unwrap();

    //     // let mut content: String = String::new();
    //     // file.read_to_string(&mut content).unwrap();
    //     // assert_eq!(content, String::from("coucou"));

    //     // swift_fs.remove_file("test.test").unwrap();
    // }

    // #[test]
    // fn test_create_file() {
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     let mut file = swift_fs.create_file("test.test").unwrap();

    //     file.write_all(String::from("coucou").as_bytes()).unwrap();
    //     file.sync_all().unwrap();

    //     let mut content: String = String::new();
    //     file.read_to_string(&mut content).unwrap();
    //     assert_eq!(content, String::from("coucou"));

    //     swift_fs.remove_file("test.test").unwrap();
    // }

    // #[test]
    // fn test_open_file() {
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     let mut file = swift_fs.create_file("testopen.test").unwrap();

    //     file.write_all(String::from("coucoutoi").as_bytes())
    //         .unwrap();
    //     file.flush().unwrap();
    //     file.sync_all().unwrap();

    //     let mut file_opened = swift_fs.open_file("testopen.test").unwrap();
    //     let mut content: String = String::new();
    //     file_opened.read_to_string(&mut content).unwrap();
    //     assert_eq!(content, String::from("coucoutoi"));

    //     swift_fs.remove_file("testopen.test").unwrap();
    // }

    // #[test]
    // fn test_rename_file() {
    //     env_logger::init();
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     swift_fs.create_dir("test/").unwrap();
    //     let mut file = swift_fs.create_file("test/testrename.test").unwrap();

    //     file.write_all(String::from("coucoutoi").as_bytes())
    //         .unwrap();
    //     file.flush().unwrap();
    //     file.sync_all().unwrap();

    //     // swift_fs
    //     //     .rename("test/testrename.test", "test/testrenamebis.test")
    //     //     .unwrap();

    //     let files = swift_fs.read_dir("test/").unwrap();
    //     println!("---- {:?}", files);

    //     // assert!(swift_fs.open_file("test/testrename.test").is_err());

    //     // swift_fs.remove_dir_all("test/testrenamebis.test").unwrap();
    // }

    #[test]
    fn test_read_dir() {
        env_logger::init();
        let swift_fs = SwiftFileSystem::new(
            env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
            env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
            env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
            env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
            env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
            String::from("chicon-test"),
        )
        .expect("cannot create swift filesystem");
        swift_fs.create_dir("testdir").unwrap();
        let mut file = swift_fs.create_file("testdir/test.test").unwrap();

        file.write_all(String::from("coucoutoi").as_bytes())
            .unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();

        let dir_entries = swift_fs.read_dir("testdir").unwrap();
        assert!(!dir_entries.is_empty());
        assert_eq!(
            dir_entries.get(0).unwrap().file_type().unwrap(),
            FileType::File
        );
        assert_eq!(
            dir_entries.get(0).unwrap().path().unwrap(),
            PathBuf::from("testdir/test.test")
        );

        swift_fs.remove_dir_all("testdir").unwrap();
    }

    #[test]
    fn test_read_dir_empty() {
        let swift_fs = SwiftFileSystem::new(
            env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
            env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
            env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
            env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
            env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
            String::from("chicon-test"),
        )
        .expect("cannot create swift filesystem");
        let mut file = swift_fs.create_file("testdirempty.test").unwrap();

        file.write_all(String::from("coucoutoi").as_bytes())
            .unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();

        let dir_entries = swift_fs.read_dir("testdirempty").unwrap();
        assert!(dir_entries.is_empty());
        swift_fs.remove_file("testdirempty.test").unwrap();
    }

    #[test]
    fn test_read_dir_empty_bis() {
        let swift_fs = SwiftFileSystem::new(
            env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
            env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
            env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
            env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
            env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
            String::from("chicon-test"),
        )
        .expect("cannot create swift filesystem");
        swift_fs.create_dir("testdiremptybis").unwrap();

        let dir_entries = swift_fs.read_dir("testdiremptybis").unwrap();
        assert!(dir_entries.is_empty());
        swift_fs.remove_dir_all("testdiremptybis").unwrap();
    }

    #[test]
    fn test_remove_dir_not_empty() {
        let swift_fs = SwiftFileSystem::new(
            env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
            env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
            env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
            env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
            env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
            String::from("chicon-test"),
        )
        .expect("cannot create swift filesystem");
        swift_fs.create_file("testremovedirnot/empty.test").unwrap();

        assert!(swift_fs.remove_dir("testremovedirnot").is_err());
        swift_fs.remove_dir_all("testremovedirnot").unwrap();
    }

    // #[test]
    // fn test_read_dir_with_dot() {
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     swift_fs.create_file("empty.test").unwrap();

    //     let dir_entries = swift_fs.read_dir(".").unwrap();
    //     assert!(!dir_entries.is_empty());
    //     assert_eq!(
    //         dir_entries.get(0).unwrap().path().unwrap(),
    //         PathBuf::from("empty.test")
    //     );
    //     swift_fs.remove_file("empty.test").unwrap();
    // }

    // #[test]
    // fn test_read_dir_recurse() {
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     swift_fs.create_dir_all("testreaddirbis/test").unwrap();
    //     swift_fs
    //         .create_file("testreaddirbis/test/mytest.test")
    //         .unwrap();
    //     swift_fs
    //         .create_file("testreaddirbis/test/myother.test")
    //         .unwrap();

    //     let dir_entries = swift_fs.read_dir("testreaddirbis/test").unwrap();

    //     assert!(!dir_entries.is_empty());
    //     assert_eq!(dir_entries.len(), 2);
    //     assert_eq!(
    //         dir_entries.get(0).unwrap().path().unwrap(),
    //         PathBuf::from("testreaddirbis/test/myother.test")
    //     );

    //     swift_fs.remove_dir_all("testreaddirbis").unwrap();
    // }

    // #[test]
    // fn test_seek_file() {
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     {
    //         let mut file = swift_fs.create_file("testseek.test").unwrap();
    //         file.write_all(String::from("coucoutoi").as_bytes())
    //             .unwrap();
    //         file.sync_all().unwrap();
    //     }

    //     let mut content = String::new();
    //     {
    //         let mut new_file = swift_fs.open_file("testseek.test").unwrap();
    //         new_file.seek(SeekFrom::Start(2)).unwrap();
    //         new_file.read_to_string(&mut content).unwrap();
    //     }
    //     assert_eq!(String::from("ucoutoi"), content);

    //     swift_fs.remove_file("testseek.test").unwrap();
    // }

    // #[test]
    // fn test_seek_end_file() {
    //     let swift_fs = SwiftFileSystem::new(
    //         env::var("OS_AUTH_URL").expect("Missing environment variable OS_AUTH_URL"),
    //         env::var("OS_USERNAME").expect("Missing environment variable OS_USERNAME"),
    //         env::var("OS_PASSWORD").expect("Missing environment variable OS_PASSWORD"),
    //         env::var("OS_REGION_NAME").expect("Missing environment variable OS_REGION_NAME"),
    //         env::var("OS_PROJECT_NAME").expect("Missing environment variable OS_PROJECT_NAME"),
    //         String::from("chicon-test"),
    //     )
    //     .expect("cannot create swift filesystem");
    //     {
    //         let mut file = swift_fs.create_file("testseekend.test").unwrap();
    //         file.write_all(String::from("coucoutoi").as_bytes())
    //             .unwrap();
    //         file.sync_all().unwrap();
    //     }

    //     let mut content = String::new();
    //     {
    //         let mut new_file = swift_fs.open_file("testseekend.test").unwrap();
    //         assert_eq!(new_file.seek(SeekFrom::End(2)).unwrap(), 11);
    //         assert_eq!(new_file.seek(SeekFrom::End(-2)).unwrap(), 7);
    //         new_file.read_to_string(&mut content).unwrap();
    //     }
    //     assert_eq!(String::from("oi"), content);
    //     swift_fs.remove_file("testseekend.test").unwrap();
    // }
}
