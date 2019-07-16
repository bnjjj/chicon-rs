use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::Permissions;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::error::ChiconError;
use crate::{DirEntry, File, FileSystem, FileType};

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
                MemDirEntry::File(file) => Ok(file.clone()),
            }
        } else {
            // create file
            if path_iter.clone().peekable().peek().is_some() {
                Err(ChiconError::MemDirNotFound(PathBuf::from(current_path)))
            } else {
                let file_internal = MemFileInternal {
                    name: current_path.to_string_lossy().into_owned(),
                    content: Vec::new(),
                    perm: Permissions::from_mode(0o755),
                    complete_path,
                };
                let file = MemFile(Rc::new(RefCell::new(file_internal)));

                self.children.insert(
                    current_path.to_string_lossy().into_owned(),
                    MemDirEntry::File(file.clone()),
                );
                Ok(file)
            }
        }
    }

    fn insert_dir(&mut self, path: PathBuf, force: bool) -> Result<MemDirectory, ChiconError> {
        let complete_path = path.clone();
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        // if something already exist
        if let Some(entry) = self
            .children
            .get_mut(&current_path.to_string_lossy().into_owned())
        {
            match entry {
                MemDirEntry::Directory(dir) => {
                    if path_iter.clone().peekable().peek().is_some() {
                        dir.insert_dir(path_iter.collect(), complete_path, force)
                    } else {
                        Ok(dir.clone())
                    }
                }
                MemDirEntry::File(_) => Err(ChiconError::BadPath),
            }
        } else {
            if path_iter.clone().peekable().peek().is_some() {
                if force {
                    // insert directory and call insert_file on it
                    let mut dir_internal = MemDirectoryInternal {
                        name: current_path.to_string_lossy().into_owned(),
                        perm: Permissions::from_mode(0o755),
                        children: None,
                        complete_path: complete_path.clone(),
                    };
                    let mut dir = MemDirectory(Rc::new(RefCell::new(dir_internal)));
                    let new_dir = dir.insert_dir(path_iter.collect(), complete_path, force)?;
                    self.children.insert(
                        current_path.to_string_lossy().into_owned(),
                        MemDirEntry::Directory(dir.clone()),
                    );

                    Ok(new_dir)
                } else {
                    Err(ChiconError::MemDirNotFound(PathBuf::from(current_path)))
                }
            } else {
                let dir_internal = MemDirectoryInternal {
                    name: current_path.to_string_lossy().into_owned(),
                    perm: Permissions::from_mode(0o755),
                    complete_path,
                    children: None,
                };
                let dir = MemDirectory(Rc::new(RefCell::new(dir_internal)));

                self.children.insert(
                    current_path.to_string_lossy().into_owned(),
                    MemDirEntry::Directory(dir.clone()),
                );

                Ok(dir)
            }
        }
    }

    // TODO: add force boolean for remove_dir_all
    fn remove(&mut self, path: PathBuf, entry_type: FileType) -> Option<MemDirEntry> {
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
            .get_mut(&current_path.to_string_lossy().into_owned())
        {
            if path_iter.clone().peekable().peek().is_some() {
                child_entry
            } else if child_entry.file_type().unwrap() == entry_type {
                return self
                    .children
                    .remove(&current_path.to_string_lossy().into_owned());
            } else {
                return None;
            }
        } else {
            return None;
        };

        child.remove(path_iter.collect(), entry_type)
    }
}
impl FileSystem for MemFileSystem {
    type FSError = ChiconError;
    type File = MemFile;
    type DirEntry = MemDirEntry;

    fn chmod<P: AsRef<Path>>(&self, _path: P, _perm: Permissions) -> Result<(), Self::FSError> {
        Ok(())
    }
    fn create_file<P: AsRef<Path>>(&mut self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        self.insert_file(PathBuf::from(path))
    }
    fn create_dir<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.insert_dir(PathBuf::from(path), false)?;

        Ok(())
    }
    fn create_dir_all<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.insert_dir(PathBuf::from(path), true)?;

        Ok(())
    }
    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        if let Some(entry) = self.get_from_relative_path(PathBuf::from(path)) {
            match entry {
                MemDirEntry::File(file) => Ok(file),
                _ => Err(ChiconError::MemFileNotFound(PathBuf::from(path))),
            }
        } else {
            Err(ChiconError::MemFileNotFound(PathBuf::from(path)))
        }
    }
    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let path = path.as_ref();
        if let Some(entry) = self.get_from_relative_path(PathBuf::from(path)) {
            match entry {
                MemDirEntry::Directory(dir) => {
                    if let Some(children) = &dir.0.try_borrow()?.children {
                        Ok(children.iter().map(|(_, child)| child.clone()).collect())
                    } else {
                        Ok(Vec::new())
                    }
                }
                _ => Err(ChiconError::MemFileNotFound(PathBuf::from(path))),
            }
        } else {
            Err(ChiconError::MemFileNotFound(PathBuf::from(path)))
        }
    }
    fn remove_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.remove(PathBuf::from(path), FileType::File)
            .map(|_| ())
            .ok_or(ChiconError::BadPath)
    }
    fn remove_dir<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.remove(PathBuf::from(path), FileType::Directory)
            .map(|_| ())
            .ok_or(ChiconError::BadPath)
    }
    fn remove_dir_all<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        // add force boolean
        // self.remove(PathBuf::from(path), FileType::Directory).map(|_| ()).ok_or(ChiconError::BadPath)
        unimplemented!()
    }
    fn rename<P: AsRef<Path>>(&mut self, from: P, to: P) -> Result<(), Self::FSError> {
        let from = from.as_ref();
        let to = to.as_ref();
        unimplemented!()
    }
}

#[derive(Clone)]
struct MemFileInternal {
    complete_path: PathBuf,
    name: String,
    content: Vec<u8>,
    perm: Permissions,
}

#[derive(Clone)]
pub struct MemFile(Rc<RefCell<MemFileInternal>>);
impl File for MemFile {
    type FSError = ChiconError;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        Ok(())
    }
}

impl Read for MemFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut cloned_content: Vec<u8> = Vec::new();
        {
            let content = &self
                .0
                .try_borrow()
                .map_err(|err| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("cannot borrow the file to fill the content : {:?}", err),
                    )
                })?
                .content;
            cloned_content = content.clone();
        }
        let mut content_slice = cloned_content.as_slice();
        let nb = content_slice.read(buf)?;

        self.0
            .try_borrow_mut()
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("cannot borrow mut the file to fill the content : {:?}", err),
                )
            })?
            .content = content_slice.to_vec();
        Ok(nb)
    }
}
impl Write for MemFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.0
            .try_borrow_mut()
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "cannot borrow mut the file to write",
                )
            })?
            .content
            .write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.0
            .try_borrow_mut()
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "cannot borrow mut the file to flush",
                )
            })?
            .content
            .flush()
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
            MemDirEntry::Directory(dir) => Ok(PathBuf::from(dir.0.try_borrow()?.name.clone())),
            MemDirEntry::File(file) => Ok(file.0.try_borrow()?.complete_path.clone()),
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
                if file.0.try_borrow().ok()?.name == current_path.to_string_lossy().into_owned() {
                    Some(self.clone())
                } else {
                    None
                }
            }
            MemDirEntry::Directory(dir) => {
                if dir.0.try_borrow().ok()?.name == current_path.to_string_lossy().into_owned() {
                    Some(self.clone())
                } else {
                    dir.get_from_relative_path(path)
                }
            }
        }
    }

    fn remove(&mut self, path: PathBuf, entry_type: FileType) -> Option<MemDirEntry> {
        let mut path_iter = path.iter();
        let current_path = if let Some(cur_path) = path_iter.next() {
            cur_path
        } else {
            return None;
        };

        match self {
            MemDirEntry::File(file) => None,
            MemDirEntry::Directory(dir) => dir.remove(path, entry_type),
        }
    }
}

#[derive(Clone)]
struct MemDirectoryInternal {
    complete_path: PathBuf,
    name: String,
    perm: Permissions,
    children: Option<HashMap<String, MemDirEntry>>,
}
#[derive(Clone)]
struct MemDirectory(Rc<RefCell<MemDirectoryInternal>>);
impl MemDirectory {
    fn get_from_relative_path(&self, path: PathBuf) -> Option<MemDirEntry> {
        let mut path_iter = path.iter();
        let current_path = if let Some(cur_path) = path_iter.next() {
            cur_path
        } else {
            return None;
        };
        let mem_dir = self.0.try_borrow().ok()?;
        let children = if let Some(children_entry) = &mem_dir.children {
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
        if self.0.try_borrow()?.children.is_none() {
            self.0.try_borrow_mut()?.children = Some(HashMap::new());
        }
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        if let Some(children) = &mut self.0.try_borrow_mut()?.children {
            // if something already exist
            if let Some(entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
                match entry {
                    MemDirEntry::Directory(dir) => {
                        dir.insert_file(path_iter.collect(), complete_path)
                    }
                    MemDirEntry::File(file) => Ok(file.clone()),
                }
            } else {
                // create file
                if path_iter.clone().peekable().peek().is_some() {
                    Err(ChiconError::MemDirNotFound(PathBuf::from(current_path)))
                } else {
                    let file_internal = MemFileInternal {
                        name: current_path.to_string_lossy().into_owned(),
                        content: Vec::new(),
                        perm: Permissions::from_mode(0o755),
                        complete_path,
                    };
                    let file = MemFile(Rc::new(RefCell::new(file_internal)));

                    children.insert(
                        current_path.to_string_lossy().into_owned(),
                        MemDirEntry::File(file.clone()),
                    );

                    Ok(file)
                }
            }
        } else {
            Err(ChiconError::BadPath)
        }
    }

    fn insert_dir(
        &mut self,
        path: PathBuf,
        complete_path: PathBuf,
        force: bool,
    ) -> Result<MemDirectory, ChiconError> {
        if self.0.try_borrow()?.children.is_none() {
            self.0.try_borrow_mut()?.children = Some(HashMap::new());
        }
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        if let Some(children) = &mut self.0.try_borrow_mut()?.children {
            // if something already exist
            if let Some(entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
                match entry {
                    MemDirEntry::Directory(dir) => {
                        if path_iter.clone().peekable().peek().is_some() {
                            dir.insert_dir(path_iter.collect(), complete_path, force)
                        } else {
                            Ok(dir.clone())
                        }
                    }
                    MemDirEntry::File(_) => Err(ChiconError::BadPath),
                }
            } else {
                // create file
                if path_iter.clone().peekable().peek().is_some() {
                    if force {
                        let mut dir_internal = MemDirectoryInternal {
                            name: current_path.to_string_lossy().into_owned(),
                            perm: Permissions::from_mode(0o755),
                            children: None,
                            complete_path: complete_path.clone(),
                        };
                        let mut dir = MemDirectory(Rc::new(RefCell::new(dir_internal)));
                        let new_dir = dir.insert_dir(path_iter.collect(), complete_path, force)?;
                        children.insert(
                            current_path.to_string_lossy().into_owned(),
                            MemDirEntry::Directory(dir.clone()),
                        );

                        Ok(new_dir)
                    } else {
                        Err(ChiconError::MemDirNotFound(PathBuf::from(current_path)))
                    }
                } else {
                    let dir_internal = MemDirectoryInternal {
                        name: current_path.to_string_lossy().into_owned(),
                        perm: Permissions::from_mode(0o755),
                        complete_path,
                        children: None,
                    };
                    let dir = MemDirectory(Rc::new(RefCell::new(dir_internal)));

                    children.insert(
                        current_path.to_string_lossy().into_owned(),
                        MemDirEntry::Directory(dir.clone()),
                    );

                    Ok(dir)
                }
            }
        } else {
            Err(ChiconError::BadPath)
        }
    }

    fn remove(&mut self, path: PathBuf, entry_type: FileType) -> Option<MemDirEntry> {
        let mut path_iter = path.iter();
        let current_path = if let Some(cur_path) = path_iter.next() {
            cur_path
        } else {
            return None;
        };

        let mut mem_dir = self.0.try_borrow_mut().ok()?;
        let children = if let Some(children_entry) = &mut mem_dir.children {
            children_entry
        } else {
            return None;
        };
        if let Some(child_entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
            if path_iter.clone().peekable().peek().is_some() {
                child_entry.remove(path_iter.collect(), entry_type)
            } else {
                children.remove(&current_path.to_string_lossy().into_owned())
            }
        } else {
            None
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
        let file_internal = MemFileInternal {
            name: String::from("test.test"),
            content: Vec::new(),
            perm: Permissions::from_mode(0o644),
            complete_path: PathBuf::from("test.test"),
        };
        let file = MemFile(Rc::new(RefCell::new(file_internal)));
        children_other.insert("test.test".to_string(), MemDirEntry::File(file));
        let other_dir_internal = MemDirectoryInternal {
            name: String::from("other"),
            perm: Permissions::from_mode(0o644),
            children: Some(children_other),
            complete_path: PathBuf::from("test/other"),
        };
        let other_dir = MemDirectory(Rc::new(RefCell::new(other_dir_internal)));
        let mut children: HashMap<String, MemDirEntry> = HashMap::new();
        children.insert("other".to_string(), MemDirEntry::Directory(other_dir));
        let dir_internal = MemDirectoryInternal {
            name: String::from("test"),
            perm: Permissions::from_mode(0o644),
            children: Some(children),
            complete_path: PathBuf::from("test"),
        };
        let dir = MemDirectory(Rc::new(RefCell::new(dir_internal)));

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
        mem_fs.insert_file(PathBuf::from("test.test")).unwrap();

        let found = mem_fs.get_from_relative_path(PathBuf::from("test.test"));
        assert!(found.is_some());
        let mem_file = found.unwrap();

        assert_eq!(mem_file.file_type().unwrap(), FileType::File);
        assert_eq!(mem_file.path().unwrap(), PathBuf::from("test.test"));

        let not_found = mem_fs.get_from_relative_path(PathBuf::from("bla.test"));
        assert!(not_found.is_none());

        assert!(mem_fs
            .insert_file(PathBuf::from("test/other/test.test"))
            .is_err());
    }

    #[test]
    fn test_read_dir() {
        let mut mem_fs = MemFileSystem::new();
        // use create_dir_all before
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        let res = mem_fs.read_dir("share/testmemreaddir").unwrap();
        assert_eq!(2, res.len());
        assert!(
            PathBuf::from(String::from("share/testmemreaddir/myfile"))
                == res.get(0).unwrap().path().unwrap()
                || PathBuf::from(String::from("share/testmemreaddir/myotherfile"))
                    == res.get(0).unwrap().path().unwrap()
        );
    }

    #[test]
    fn test_create_dir() {
        let mut mem_fs = MemFileSystem::new();
        // use create_dir_all before
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        let res = mem_fs.read_dir("share/testmemreaddir").unwrap();
        assert_eq!(2, res.len());
        assert!(
            PathBuf::from(String::from("share/testmemreaddir/myfile"))
                == res.get(0).unwrap().path().unwrap()
                || PathBuf::from(String::from("share/testmemreaddir/myotherfile"))
                    == res.get(0).unwrap().path().unwrap()
        );
    }

    #[test]
    fn test_create_file() {
        let mut mem_fs = MemFileSystem::new();
        // use create_dir_all before
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        let res = mem_fs.read_dir("share/testmemreaddir").unwrap();
        assert_eq!(2, res.len());
        assert!(
            PathBuf::from(String::from("share/testmemreaddir/myfile"))
                == res.get(0).unwrap().path().unwrap()
                || PathBuf::from(String::from("share/testmemreaddir/myotherfile"))
                    == res.get(0).unwrap().path().unwrap()
        );

        let mut file = mem_fs
            .open_file("share/testmemreaddir/myotherfile")
            .unwrap();
        let mut buffer = String::new();
        {
            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
            file.read_to_string(&mut buffer).unwrap();
        }

        assert_eq!(buffer, String::from("coucoutoi"));
    }

    #[test]
    fn test_remove_file() {
        let mut mem_fs = MemFileSystem::new();
        // use create_dir_all before
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        let res = mem_fs.read_dir("share/testmemreaddir").unwrap();
        assert_eq!(2, res.len());
        assert!(
            PathBuf::from(String::from("share/testmemreaddir/myfile"))
                == res.get(0).unwrap().path().unwrap()
                || PathBuf::from(String::from("share/testmemreaddir/myotherfile"))
                    == res.get(0).unwrap().path().unwrap()
        );

        let mut file = mem_fs
            .open_file("share/testmemreaddir/myotherfile")
            .unwrap();
        let mut buffer = String::new();
        {
            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
            file.read_to_string(&mut buffer).unwrap();
        }

        assert_eq!(buffer, String::from("coucoutoi"));

        mem_fs
            .remove_file("share/testmemreaddir/myotherfile")
            .unwrap();

        assert!(mem_fs
            .open_file("share/testmemreaddir/myotherfile")
            .is_err());
    }

    #[test]
    fn test_remove_dir() {
        let mut mem_fs = MemFileSystem::new();
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        mem_fs.remove_dir("share/testmemreaddir").unwrap();

        assert!(mem_fs.read_dir("share/testmemreaddir").is_err());
    }

}
