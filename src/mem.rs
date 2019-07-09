use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::Permissions;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::ChiconError;
use crate::{DirEntry, File, FileSystem, FileType};

// Arbre avec chaque node = un element du path
#[derive(Default, Clone)]
struct MemFileSystem {
    children: HashMap<String, MemDirEntry>,
}
impl MemFileSystem {
    pub fn new() -> Self {
        MemFileSystem {
            children: HashMap::new(),
        }
    }

    fn get_from_relative_path(&self, path: PathBuf) -> Option<MemDirEntry> {
        if self.children.is_empty() {
            return None;
        }

        let mut path_iter = path.iter();
        let current_path = if let Some(cur_path) = path_iter.next() {
            cur_path
        } else {
            return None;
        };

        let child = if let Some(child_entry) = self
            .children
            .get(&current_path.to_string_lossy().into_owned())
        {
            child_entry
        } else {
            return None;
        };

        child.get_from_relative_path(path_iter.collect())
    }

    fn insert_file(&mut self, path: PathBuf) -> Result<MemFile, ChiconError> {
        let complete_path = path.clone();
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        // if something already exist
        if let Some(entry) = self
            .children
            .get_mut(&current_path.to_string_lossy().into_owned())
        {
            match entry {
                MemDirEntry::Directory(dir) => dir.insert_file(path_iter.collect(), complete_path),
                MemDirEntry::File(file) => {
                    file.truncate(path_iter.collect())?;
                    Ok(file.clone())
                }
            }
        } else {
            // create file
            if path_iter.clone().peekable().peek().is_some() {
                // insert directory and call insert_file on it
                let mut dir = MemDirectory {
                    name: current_path.to_string_lossy().into_owned(),
                    perm: Permissions::from_mode(0o755),
                    children: None,
                };
                let file_created = dir.insert_file(path_iter.collect(), complete_path)?;
                self.children
                    .insert(dir.name.clone(), MemDirEntry::Directory(dir.clone()));

                Ok(file_created)
            } else {
                let file = MemFile {
                    name: current_path.to_string_lossy().into_owned(),
                    content: Vec::new(),
                    perm: Permissions::from_mode(0o755),
                    complete_path,
                };
                self.children
                    .insert(file.name.clone(), MemDirEntry::File(file.clone()));
                Ok(file)
            }
        }
    }
}
impl FileSystem for MemFileSystem {
    type FSError = ChiconError;
    type File = MemFile;
    type DirEntry = MemDirEntry;

    fn chmod<P: AsRef<Path>>(&self, path: P, perm: Permissions) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        unimplemented!()
    }
    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();

        unimplemented!()
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

#[derive(Clone)]
struct MemFile {
    complete_path: PathBuf,
    name: String,
    content: Vec<u8>,
    perm: Permissions,
}
impl MemFile {
    fn truncate(&mut self, path: PathBuf) -> Result<(), ChiconError> {
        if !self.content.is_empty() {
            // check with std::fs library if it clears the content
            self.content.clear();
        }
        Ok(())
    }
}
impl File for MemFile {
    type FSError = ChiconError;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        // Get MemFile from filesystem and update him

        Ok(())
    }
}

impl Read for MemFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut content_slice = self.content.as_slice();
        let nb = content_slice.read(buf)?;
        self.content = content_slice.to_vec();
        Ok(nb)
    }
}
impl Write for MemFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.content.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.content.flush()
    }
}

#[derive(Clone)]
enum MemDirEntry {
    File(MemFile),
    Directory(MemDirectory),
}
impl DirEntry for MemDirEntry {
    type FSError = ChiconError;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        match self {
            MemDirEntry::Directory(dir) => Ok(PathBuf::from(dir.name.clone())),
            MemDirEntry::File(file) => Ok(PathBuf::from(file.complete_path.clone())),
        }
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        match self {
            MemDirEntry::Directory(_) => Ok(FileType::Directory),
            MemDirEntry::File(_) => Ok(FileType::File),
        }
    }
}
impl MemDirEntry {
    fn get_from_relative_path(&self, path: PathBuf) -> Option<MemDirEntry> {
        let mut path_iter = path.iter();
        let current_path = if let Some(cur_path) = path_iter.next() {
            cur_path
        } else {
            return Some(self.clone());
        };

        match self {
            MemDirEntry::File(file) => {
                if file.name == current_path.to_string_lossy().into_owned() {
                    Some(self.clone())
                } else {
                    None
                }
            }
            MemDirEntry::Directory(dir) => {
                if dir.name == current_path.to_string_lossy().into_owned() {
                    Some(self.clone())
                } else {
                    dir.get_from_relative_path(path)
                }
            }
        }
    }
}

#[derive(Clone)]
struct MemDirectory {
    // complete_path: PathBuf,
    name: String,
    perm: Permissions,
    children: Option<HashMap<String, MemDirEntry>>,
}
impl MemDirectory {
    fn get_from_relative_path(&self, path: PathBuf) -> Option<MemDirEntry> {
        let mut path_iter = path.iter();
        let current_path = if let Some(cur_path) = path_iter.next() {
            cur_path
        } else {
            return None;
        };
        let children = if let Some(children_entry) = &self.children {
            children_entry
        } else {
            return None;
        };

        children
            .get(&current_path.to_string_lossy().into_owned())
            .and_then(|entry| entry.get_from_relative_path(path_iter.collect()))
    }

    fn insert_file(
        &mut self,
        path: PathBuf,
        complete_path: PathBuf,
    ) -> Result<MemFile, ChiconError> {
        if self.children.is_none() {
            self.children = Some(HashMap::new());
        }
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        if let Some(children) = &mut self.children {
            // if something already exist
            if let Some(entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
                match entry {
                    MemDirEntry::Directory(dir) => {
                        dir.insert_file(path_iter.collect(), complete_path)
                    }
                    MemDirEntry::File(file) => {
                        file.truncate(path_iter.collect())?;
                        Ok(file.clone())
                    }
                }
            } else {
                // create file
                if path_iter.clone().peekable().peek().is_some() {
                    // insert directory and call insert_file on it
                    let mut dir = MemDirectory {
                        name: current_path.to_string_lossy().into_owned(),
                        perm: Permissions::from_mode(0o755),
                        children: None,
                    };
                    let file_created = dir.insert_file(path_iter.collect(), complete_path)?;
                    children.insert(dir.name.clone(), MemDirEntry::Directory(dir.clone()));

                    Ok(file_created)
                } else {
                    let file = MemFile {
                        name: current_path.to_string_lossy().into_owned(),
                        content: Vec::new(),
                        perm: Permissions::from_mode(0o755),
                        complete_path,
                    };
                    children.insert(file.name.clone(), MemDirEntry::File(file.clone()));

                    Ok(file)
                }
            }
        } else {
            Err(ChiconError::BadPath)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fs_internals() {
        let mut mem_fs = MemFileSystem::new();
        let mut children_other: HashMap<String, MemDirEntry> = HashMap::new();
        let file = MemFile {
            name: String::from("test.test"),
            content: Vec::new(),
            perm: Permissions::from_mode(0o644),
            complete_path: PathBuf::from("test.test"),
        };
        children_other.insert("test.test".to_string(), MemDirEntry::File(file));
        let other_dir = MemDirectory {
            name: String::from("other"),
            perm: Permissions::from_mode(0o644),
            children: Some(children_other),
        };

        let mut children: HashMap<String, MemDirEntry> = HashMap::new();
        children.insert("other".to_string(), MemDirEntry::Directory(other_dir));
        let dir = MemDirectory {
            name: String::from("test"),
            perm: Permissions::from_mode(0o644),
            children: Some(children),
        };

        mem_fs
            .children
            .insert(String::from("test"), MemDirEntry::Directory(dir));

        let found = mem_fs.get_from_relative_path(PathBuf::from("test/other/test.test"));
        assert!(found.is_some());
        let mem_file = found.unwrap();

        assert_eq!(mem_file.file_type().unwrap(), FileType::File);
        assert_eq!(mem_file.path().unwrap(), PathBuf::from("test.test"));

        let not_found = mem_fs.get_from_relative_path(PathBuf::from("test/other/bla.test"));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_fs_internals_insert_file() {
        let mut mem_fs = MemFileSystem::new();
        mem_fs
            .insert_file(PathBuf::from("test/other/test.test"))
            .unwrap();

        let found = mem_fs.get_from_relative_path(PathBuf::from("test/other/test.test"));
        assert!(found.is_some());
        let mem_file = found.unwrap();

        assert_eq!(mem_file.file_type().unwrap(), FileType::File);
        assert_eq!(
            mem_file.path().unwrap(),
            PathBuf::from("test/other/test.test")
        );

        let not_found = mem_fs.get_from_relative_path(PathBuf::from("test/other/bla.test"));
        assert!(not_found.is_none());
    }

}
