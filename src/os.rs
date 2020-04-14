use std::fs::{File, OpenOptions, Permissions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::{ChiconError, DirEntry, File as FsFile, FileSystem, FileType};

/// Structure implementing `FileSystem` trait to store on a local filesystem
#[derive(Default)]
pub struct OsFileSystem;

impl OsFileSystem {
    pub fn new() -> Self {
        OsFileSystem {}
    }
}

impl FileSystem for OsFileSystem {
    type FSError = ChiconError;
    type File = OsFile;
    type DirEntry = OsDirEntry;

    fn chmod<P: AsRef<Path>>(&self, path: P, perm: Permissions) -> Result<(), Self::FSError> {
        std::fs::set_permissions(path, perm).map_err(|e| e.into())
    }

    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        Ok(OsFile::from(File::create(path)?))
    }

    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        std::fs::create_dir(path).map_err(|e| e.into())
    }

    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        std::fs::create_dir_all(path).map_err(|e| e.into())
    }

    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        Ok(OsFile::from(
            OpenOptions::new()
                .read(true)
                .write(true)
                .append(true)
                .open(path)?,
        ))
    }

    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let read_dir = std::fs::read_dir(path)?.filter_map(Result::ok);
        Ok(read_dir.map(OsDirEntry::from).collect())
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        std::fs::remove_file(path).map_err(|e| e.into())
    }

    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        std::fs::remove_dir(path).map_err(|e| e.into())
    }

    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        std::fs::remove_dir_all(path).map_err(|e| e.into())
    }

    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        std::fs::rename(from, to).map_err(|e| e.into())
    }
}

/// Structure implementing File trait to represent a file on a local filesystem
pub struct OsFile(File);

impl FsFile for OsFile {
    type FSError = std::io::Error;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        self.0.sync_all()
    }
}

impl Read for OsFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.0.read(buf)
    }
}

impl Write for OsFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.0.flush()
    }
}

impl Seek for OsFile {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, std::io::Error> {
        self.0.seek(pos)
    }
}

impl From<File> for OsFile {
    fn from(file: File) -> Self {
        OsFile(file)
    }
}

/// Structure implementing `DirEntry` trait to represent an entry in a directory on a local filesystem
pub struct OsDirEntry(std::fs::DirEntry);

impl DirEntry for OsDirEntry {
    type FSError = ChiconError;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        Ok(self.0.path())
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        let file_type = self.0.file_type()?;
        if file_type.is_dir() {
            Ok(FileType::Directory)
        } else if file_type.is_file() {
            Ok(FileType::File)
        } else {
            Ok(FileType::Symlink)
        }
    }
}

impl From<std::fs::DirEntry> for OsDirEntry {
    fn from(dir_entry: std::fs::DirEntry) -> Self {
        OsDirEntry(dir_entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_file() {
        let os_fs = OsFileSystem::new();
        {
            let mut file = os_fs.create_file("test.test").unwrap();

            assert!(std::fs::read("test.test").is_ok());

            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
        }

        let mut content = String::new();
        {
            let mut new_file = os_fs.open_file("test.test").unwrap();
            new_file.read_to_string(&mut content).unwrap();
        }
        assert_eq!(String::from("coucoutoi"), content);

        std::fs::remove_file("test.test").unwrap();
    }

    #[test]
    fn test_seek_file() {
        let os_fs = OsFileSystem::new();
        {
            let mut file = os_fs.create_file("testseek.test").unwrap();

            assert!(std::fs::read("testseek.test").is_ok());

            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
        }

        let mut content = String::new();
        {
            let mut new_file = os_fs.open_file("testseek.test").unwrap();
            new_file.seek(SeekFrom::Start(2)).unwrap();
            new_file.read_to_string(&mut content).unwrap();
        }
        assert_eq!(String::from("ucoutoi"), content);

        std::fs::remove_file("testseek.test").unwrap();
    }

    #[test]
    fn test_seek_end_file() {
        let os_fs = OsFileSystem::new();
        {
            let mut file = os_fs.create_file("testseekend.test").unwrap();
            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
        }

        let mut content = String::new();
        {
            let mut new_file = os_fs.open_file("testseekend.test").unwrap();
            assert_eq!(new_file.seek(SeekFrom::End(2)).unwrap(), 11);
            assert_eq!(new_file.seek(SeekFrom::End(-2)).unwrap(), 7);
            new_file.read_to_string(&mut content).unwrap();
        }
        assert_eq!(String::from("oi"), content);

        std::fs::remove_file("testseekend.test").unwrap();
    }

    #[test]
    fn test_create_dir() {
        let os_fs = OsFileSystem::new();
        os_fs.create_dir("testdir").unwrap();

        assert!(std::fs::read_dir("testdir").is_ok());

        std::fs::remove_dir("testdir").unwrap();
    }

    #[test]
    fn test_create_dir_all() {
        let os_fs = OsFileSystem::new();
        os_fs.create_dir_all("testdirall/test").unwrap();
        os_fs.create_file("testdirall/test/test.test").unwrap();
        os_fs.create_dir_all("testdirall/test").unwrap();

        assert!(std::fs::read_dir("testdirall/test").is_ok());

        std::fs::remove_dir_all("testdirall").unwrap();
    }

    #[test]
    fn test_read_dir() {
        let os_fs = OsFileSystem::new();
        os_fs.create_dir_all("testreaddir/test").unwrap();
        os_fs.create_file("testreaddir/mytest.test").unwrap();

        let dir_entries = os_fs.read_dir("testreaddir").unwrap();

        assert!(!dir_entries.is_empty());
        assert_eq!(dir_entries.len(), 2);
        assert_eq!(
            dir_entries.get(0).unwrap().path().unwrap(),
            PathBuf::from("testreaddir/test")
        );

        std::fs::remove_dir_all("testreaddir").unwrap();
    }

    #[test]
    fn test_read_dir_bis() {
        let os_fs = OsFileSystem::new();
        os_fs.create_dir_all("testreaddirbis/test").unwrap();
        os_fs
            .create_file("testreaddirbis/test/mytest.test")
            .unwrap();
        os_fs
            .create_file("testreaddirbis/test/myother.test")
            .unwrap();

        let dir_entries = os_fs.read_dir("testreaddirbis/test").unwrap();

        assert!(!dir_entries.is_empty());
        assert_eq!(dir_entries.len(), 2);
        assert_eq!(
            dir_entries.get(0).unwrap().path().unwrap(),
            PathBuf::from("testreaddirbis/test/mytest.test")
        );

        std::fs::remove_dir_all("testreaddirbis").unwrap();
    }

    #[test]
    fn test_create_file_expects_error() {
        let os_fs = OsFileSystem::new();
        let err: Option<ChiconError> = os_fs.remove_dir("++non_existant+++").err();
        assert_eq!(err.is_some(), true)
    }
}
