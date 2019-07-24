use std::env;
use std::fs::Permissions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use url::percent_encoding::{utf8_percent_encode, SIMPLE_ENCODE_SET};

use rusoto_core::{
    credential::EnvironmentProvider, region::Region, request::HttpClient, ByteStream,
};
use rusoto_s3::{
    CopyObjectRequest, DeleteObjectRequest, GetObjectRequest, ListObjectsV2Request,
    PutObjectRequest, S3Client, S3,
};

use crate::{error::ChiconError, DirEntry, File, FileSystem, FileType};

define_encode_set! {
    pub QUERY_ENCODE_SET = [SIMPLE_ENCODE_SET] | {' ', '"', '#', '<', '>'}
}

/// Structure implementing `FileSystem` trait to store on an Amazon S3 API compliant
pub struct S3FileSystem {
    bucket: String,
    s3_client: S3Client,
}
impl S3FileSystem {
    pub fn new(
        access_key_id: String,
        secret_access_key: String,
        bucket: String,
        region: String,
        endpoint: String,
    ) -> Self {
        env::set_var("CHICON_ACCESS_KEY_ID", access_key_id);
        env::set_var("CHICON_SECRET_ACCESS_KEY", secret_access_key);
        let http_client = HttpClient::new().expect("cannot create http client with tls enabled");
        let s3_client = S3Client::new_with(
            http_client,
            EnvironmentProvider::with_prefix("CHICON"),
            Region::Custom {
                name: region,
                endpoint,
            },
        );
        S3FileSystem { bucket, s3_client }
    }
}
impl FileSystem for S3FileSystem {
    type FSError = ChiconError;
    type File = S3File;
    type DirEntry = S3DirEntry;

    fn chmod<P: AsRef<Path>>(&self, _path: P, _perm: Permissions) -> Result<(), Self::FSError> {
        unimplemented!()
    }

    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path: &Path = path.as_ref();
        let filename: String = path.to_string_lossy().into_owned();
        if filename.contains("../") {
            return Err(ChiconError::RelativePath);
        }
        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: filename.clone(),
            ..Default::default()
        };

        let _put_obj_res = self.s3_client.put_object(req).sync()?;
        let get_req = GetObjectRequest {
            bucket: self.bucket.clone(),
            key: filename.clone(),
            ..Default::default()
        };

        let _object = self.s3_client.get_object(get_req).sync()?;
        Ok(S3File::new(
            self.bucket.clone(),
            filename,
            self.s3_client.clone(),
        ))
    }

    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path: &Path = path.as_ref();
        let mut dir: String = path.to_string_lossy().into_owned();
        if dir.contains("../") {
            return Err(ChiconError::RelativePath);
        }
        if !dir.ends_with('/') {
            dir.push('/');
        }

        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: dir,
            body: Some(ByteStream::from(vec![])),
            ..Default::default()
        };

        self.s3_client
            .put_object(req)
            .sync()
            .map(|_| ())
            .map_err(ChiconError::from)
    }

    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        self.create_dir(path)
    }

    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path: &Path = path.as_ref();
        let filename: String = path.to_string_lossy().into_owned();
        if filename.contains("../") {
            return Err(ChiconError::RelativePath);
        }
        let get_req = GetObjectRequest {
            bucket: self.bucket.clone(),
            key: filename.clone(),
            ..Default::default()
        };

        let object_res = self.s3_client.get_object(get_req).sync()?;
        let mut file = S3File::new(self.bucket.clone(), filename, self.s3_client.clone());
        if let Some(body) = object_res.body {
            std::io::copy(&mut body.into_async_read(), &mut file)?;
        }

        Ok(file)
    }

    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let path: &Path = path.as_ref();
        let mut dir_name: String = path
            .to_string_lossy()
            .into_owned()
            .trim_start_matches("./")
            .to_string();
        if dir_name.contains("../") {
            return Err(ChiconError::RelativePath);
        }
        let prefix: Option<String> = if dir_name != "." {
            if !dir_name.ends_with('/') {
                dir_name.push('/');
            }
            Some(dir_name.clone())
        } else {
            None
        };

        let list_req = ListObjectsV2Request {
            bucket: self.bucket.clone(),
            prefix,
            ..Default::default()
        };
        let list = self.s3_client.list_objects_v2(list_req).sync()?;
        let mut dir_entries: Vec<S3DirEntry> = Vec::new();
        if let Some(objects) = list.contents {
            for object in objects {
                if let Some(key) = object.key {
                    dir_entries.push(S3DirEntry { key });
                }
            }
        }
        Ok(dir_entries)
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path: &Path = path.as_ref();
        let filename = path.to_string_lossy().into_owned();
        if filename.contains("../") {
            return Err(ChiconError::RelativePath);
        }
        let req = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            key: filename,
            ..Default::default()
        };

        self.s3_client
            .delete_object(req)
            .sync()
            .map(|_| ())
            .map_err(ChiconError::from)
    }

    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path: &Path = path.as_ref();
        let dir_name = path.to_string_lossy().into_owned();
        if dir_name.contains("../") {
            return Err(ChiconError::RelativePath);
        }

        let dir_entries = self.read_dir(path)?;
        if !dir_entries.is_empty() {
            return Err(ChiconError::DirectoryNotEmpty);
        }

        self.remove_dir_all(path)
    }

    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path: &Path = path.as_ref();
        let dir_name = path.to_string_lossy().into_owned();
        if dir_name.contains("../") {
            return Err(ChiconError::RelativePath);
        }

        let req = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            key: path.to_string_lossy().into_owned(),
            ..Default::default()
        };

        self.s3_client
            .delete_object(req)
            .sync()
            .map(|_| ())
            .map_err(ChiconError::from)
    }

    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        let from: &Path = from.as_ref();
        let from_filename: String = from.to_string_lossy().into_owned();
        if from_filename.contains("../") {
            return Err(ChiconError::RelativePath);
        }
        let to: &Path = to.as_ref();
        let to_filename: String = to.to_string_lossy().into_owned();
        if to_filename.contains("../") {
            return Err(ChiconError::RelativePath);
        }

        let copy_req = CopyObjectRequest {
            bucket: self.bucket.clone(),
            key: to_filename,
            copy_source: utf8_percent_encode(
                format!("{}/{}", self.bucket, from_filename).as_ref(),
                QUERY_ENCODE_SET,
            )
            .collect::<String>(),
            ..Default::default()
        };

        self.s3_client.copy_object(copy_req).sync()?;
        self.remove_file(from_filename)
    }
}

/// Structure implementing `File` trait to represent a file on an Amazon S3 API compliant
pub struct S3File {
    key: String,
    bucket: String,
    content: Vec<u8>,
    s3_client: S3Client,
}
impl File for S3File {
    type FSError = ChiconError;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: self.key.clone(),
            body: Some(self.content.clone().into()),
            ..Default::default()
        };
        let _res = self.s3_client.put_object(req).sync()?;
        Ok(())
    }
}

impl Read for S3File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut content_slice = self.content.as_slice();
        let nb = content_slice.read(buf)?;
        self.content = content_slice.to_vec();
        Ok(nb)
    }
}
impl Write for S3File {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.content.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.content.flush()
    }
}

impl S3File {
    fn new(bucket: String, key: String, s3_client: S3Client) -> Self {
        S3File {
            bucket,
            key,
            content: Vec::new(),
            s3_client,
        }
    }
}

/// Structure implementing `DirEntry` trait to represent an entry in a directory on an Amazon S3 API compliant
pub struct S3DirEntry {
    key: String,
}
impl DirEntry for S3DirEntry {
    type FSError = ChiconError;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        Ok(PathBuf::from(self.key.clone()))
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        if self.key.ends_with('/') {
            Ok(FileType::Directory)
        } else {
            Ok(FileType::File)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_file() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        let mut file = s3_fs.create_file("test.test").unwrap();

        file.write_all(String::from("coucou").as_bytes()).unwrap();
        file.sync_all().unwrap();

        let mut content: String = String::new();
        file.read_to_string(&mut content).unwrap();
        assert_eq!(content, String::from("coucou"));

        s3_fs.remove_file("test.test").unwrap();
    }

    #[test]
    fn test_open_file() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        let mut file = s3_fs.create_file("testopen.test").unwrap();

        file.write_all(String::from("coucoutoi").as_bytes())
            .unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();

        let mut file_opened = s3_fs.open_file("testopen.test").unwrap();
        let mut content: String = String::new();
        file_opened.read_to_string(&mut content).unwrap();
        assert_eq!(content, String::from("coucoutoi"));

        s3_fs.remove_file("testopen.test").unwrap();
    }

    #[test]
    fn test_rename_file() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        let mut file = s3_fs.create_file("test/testrename.test").unwrap();

        file.write_all(String::from("coucoutoi").as_bytes())
            .unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();

        s3_fs
            .rename("test/testrename.test", "test/testrenamebis.test")
            .unwrap();

        assert!(s3_fs.open_file("test/testrename.test").is_err());

        s3_fs.remove_dir_all("test/testrenamebis.test").unwrap();
    }

    #[test]
    fn test_read_dir() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        let mut file = s3_fs.create_file("testdir/test.test").unwrap();

        file.write_all(String::from("coucoutoi").as_bytes())
            .unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();

        let dir_entries = s3_fs.read_dir("testdir").unwrap();
        assert!(!dir_entries.is_empty());
        assert_eq!(
            dir_entries.get(0).unwrap().file_type().unwrap(),
            FileType::File
        );
        assert_eq!(
            dir_entries.get(0).unwrap().path().unwrap(),
            PathBuf::from("testdir/test.test")
        );

        s3_fs.remove_dir_all("testdir").unwrap();
    }

    #[test]
    fn test_read_dir_empty() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        let mut file = s3_fs.create_file("testdirempty.test").unwrap();

        file.write_all(String::from("coucoutoi").as_bytes())
            .unwrap();
        file.flush().unwrap();
        file.sync_all().unwrap();

        let dir_entries = s3_fs.read_dir("testdirempty").unwrap();
        assert!(dir_entries.is_empty());
        s3_fs.remove_file("testdirempty.test").unwrap();
    }

    #[test]
    fn test_read_dir_empty_bis() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        s3_fs.create_dir("testdiremptybis").unwrap();

        let dir_entries = s3_fs.read_dir("testdiremptybis").unwrap();
        assert!(dir_entries.is_empty());
        s3_fs.remove_dir_all("testdiremptybis").unwrap();
    }

    #[test]
    fn test_remove_dir_not_empty() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        s3_fs.create_file("testremovedirnot/empty.test").unwrap();

        assert!(s3_fs.remove_dir("testremovedirnot").is_err());
        s3_fs.remove_dir_all("testremovedirnot").unwrap();
    }

    #[test]
    fn test_read_dir_with_dot() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        s3_fs.create_file("empty.test").unwrap();

        let dir_entries = s3_fs.read_dir(".").unwrap();
        assert!(!dir_entries.is_empty());
        assert_eq!(
            dir_entries.get(0).unwrap().path().unwrap(),
            PathBuf::from("empty.test")
        );
        s3_fs.remove_file("empty.test").unwrap();
    }

    #[test]
    fn test_read_dir_recurse() {
        let s3_fs = S3FileSystem::new(
            String::from("testest"),
            String::from("testtest"),
            String::from("test"),
            String::from("local"),
            String::from("http://127.0.0.1"),
        );
        s3_fs.create_dir_all("testreaddirbis/test").unwrap();
        s3_fs
            .create_file("testreaddirbis/test/mytest.test")
            .unwrap();
        s3_fs
            .create_file("testreaddirbis/test/myother.test")
            .unwrap();

        let dir_entries = s3_fs.read_dir("testreaddirbis/test").unwrap();

        assert!(!dir_entries.is_empty());
        assert_eq!(dir_entries.len(), 2);
        assert_eq!(
            dir_entries.get(0).unwrap().path().unwrap(),
            PathBuf::from("testreaddirbis/test/myother.test")
        );

        s3_fs.remove_dir_all("testreaddirbis").unwrap();
    }
}
