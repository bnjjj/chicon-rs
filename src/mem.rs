use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::Permissions;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::error::ChiconError;
use crate::{DirEntry, File, FileSystem, FileType};

/// Structure implementing `FileSystem` trait to store on an in memory filesystem, for now please only for testing use cases ! Need to be benchmarked before production use
#[derive(Default, Clone)]
pub struct MemFileSystem {
    children: RefCell<HashMap<String, MemDirEntry>>,
}
impl FileSystem for MemFileSystem {
    type FSError = ChiconError;
    type File = MemFile;
    type DirEntry = MemDirEntry;

    fn chmod<P: AsRef<Path>>(&self, _path: P, _perm: Permissions) -> Result<(), Self::FSError> {
        Ok(())
    }
    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        self.insert_file(PathBuf::from(path))
    }
    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.insert_dir(PathBuf::from(path), false)?;

        Ok(())
    }
    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
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
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.remove(PathBuf::from(path), FileType::File, false)
    }
    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.remove(PathBuf::from(path), FileType::Directory, false)
    }
    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        self.remove(PathBuf::from(path), FileType::Directory, true)
    }
    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        let from = from.as_ref();
        let to = to.as_ref();
        self.rename_internal(PathBuf::from(from), PathBuf::from(to))
    }
}

impl MemFileSystem {
    pub fn new() -> Self {
        MemFileSystem {
            children: RefCell::new(HashMap::new()),
        }
    }

    fn get_from_relative_path(&self, path: PathBuf) -> Option<MemDirEntry> {
        let children = self.children.try_borrow().ok()?;
        if children.is_empty() {
            return None;
        }

        let mut path_iter = path.iter();
        let current_path = path_iter.next()?;
        let child =
            if let Some(child_entry) = children.get(&current_path.to_string_lossy().into_owned()) {
                child_entry
            } else {
                return None;
            };

        child.get_from_relative_path(path_iter.collect())
    }

    fn insert_file(&self, path: PathBuf) -> Result<MemFile, ChiconError> {
        let complete_path = path.clone();
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        let mut children = self.children.try_borrow_mut()?;

        // if something already exist
        if let Some(entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
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

                children.insert(
                    current_path.to_string_lossy().into_owned(),
                    MemDirEntry::File(file.clone()),
                );
                Ok(file)
            }
        }
    }

    fn insert_dir(&self, path: PathBuf, force: bool) -> Result<MemDirectory, ChiconError> {
        let complete_path = path.clone();
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;
        let mut children = self.children.try_borrow_mut()?;

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
        } else if path_iter.clone().peekable().peek().is_some() {
            if force {
                // insert directory and call insert_file on it
                let dir_internal = MemDirectoryInternal {
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

    fn remove(&self, path: PathBuf, entry_type: FileType, force: bool) -> Result<(), ChiconError> {
        let mut children = self.children.try_borrow_mut()?;
        if children.is_empty() {
            return match entry_type {
                FileType::File => Err(ChiconError::MemFileNotFound(path)),
                FileType::Directory => Err(ChiconError::MemDirNotFound(path)),
                _ => Err(ChiconError::BadPath),
            };
        }
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        let child = if let Some(child_entry) =
            children.get_mut(&current_path.to_string_lossy().into_owned())
        {
            if path_iter.clone().peekable().peek().is_some() {
                child_entry
            } else if child_entry.file_type().unwrap() == entry_type {
                // check force
                if let MemDirEntry::Directory(dir_entry) = child_entry {
                    let dir_internal = dir_entry.0.try_borrow()?;
                    if let Some(dir_children) = &dir_internal.children {
                        if !dir_children.is_empty() && !force {
                            // Return error
                            return Err(ChiconError::MemDirNotEmpty(path.clone()));
                        }
                    }
                }

                return children
                    .remove(&current_path.to_string_lossy().into_owned())
                    .map(|_| ())
                    .ok_or(ChiconError::BadPath);
            } else {
                return Err(ChiconError::BadPath);
            }
        } else {
            return Err(ChiconError::BadPath);
        };

        child.remove(path_iter.collect(), entry_type, force)
    }

    fn rename_internal(&self, path: PathBuf, new_path: PathBuf) -> Result<(), ChiconError> {
        let complete_path = new_path.clone();
        if self.children.try_borrow()?.is_empty() {
            return Err(ChiconError::BadPath);
        }

        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;
        let mut new_path_iter = new_path.iter();
        let current_new_path = new_path_iter.next().ok_or(ChiconError::BadPath)?;

        let mut children = self.children.try_borrow_mut()?;
        let child = if let Some(child_entry) =
            children.get_mut(&current_path.to_string_lossy().into_owned())
        {
            if path_iter.clone().peekable().peek().is_some() {
                child_entry
            } else {
                // replace
                match child_entry {
                    MemDirEntry::Directory(dir_entry) => {
                        {
                            let mut dir_internal = dir_entry.0.try_borrow_mut()?;
                            dir_internal.complete_path = complete_path;
                        }
                        let dir_entry_cloned = dir_entry.clone();
                        children.remove(&current_path.to_string_lossy().into_owned());
                        children.insert(
                            current_new_path.to_string_lossy().into_owned(),
                            MemDirEntry::Directory(dir_entry_cloned),
                        );
                        return Ok(());
                    }
                    MemDirEntry::File(file_entry) => {
                        {
                            let mut file_internal = file_entry.0.try_borrow_mut()?;
                            file_internal.complete_path = complete_path;
                        }
                        let file_entry_cloned = file_entry.clone();
                        children.remove(&current_path.to_string_lossy().into_owned());
                        children.insert(
                            current_new_path.to_string_lossy().into_owned(),
                            MemDirEntry::File(file_entry_cloned),
                        );
                        return Ok(());
                    }
                }
            }
        } else {
            return Err(ChiconError::BadPath);
        };

        child.rename(path_iter.collect(), new_path_iter.collect(), complete_path)
    }
}

#[derive(Clone)]
struct MemFileInternal {
    complete_path: PathBuf,
    name: String,
    content: Vec<u8>,
    perm: Permissions,
}

/// Structure implementing File trait to represent a file on an in memory filesystem
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
        let mut cloned_content: Vec<u8>;
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

/// Structure implementing `DirEntry` trait to represent an entry in a directory on an in memory filesystem
#[derive(Clone)]
pub enum MemDirEntry {
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

    fn remove(
        &mut self,
        path: PathBuf,
        entry_type: FileType,
        force: bool,
    ) -> Result<(), ChiconError> {
        match self {
            MemDirEntry::File(_) => Err(ChiconError::BadPath),
            MemDirEntry::Directory(dir) => dir.remove(path, entry_type, force),
        }
    }

    fn rename(
        &mut self,
        path: PathBuf,
        new_path: PathBuf,
        complete_path: PathBuf,
    ) -> Result<(), ChiconError> {
        // replace
        match self {
            MemDirEntry::Directory(dir_entry) => dir_entry.rename(path, new_path, complete_path),
            MemDirEntry::File(_) => Err(ChiconError::BadPath),
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
/// Structure representing a directory on an in memory filesystem
#[derive(Clone)]
pub struct MemDirectory(Rc<RefCell<MemDirectoryInternal>>);
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
                        let dir_internal = MemDirectoryInternal {
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

    fn remove(
        &mut self,
        path: PathBuf,
        entry_type: FileType,
        force: bool,
    ) -> Result<(), ChiconError> {
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;

        let mut mem_dir = self.0.try_borrow_mut()?;
        let children = if let Some(children_entry) = &mut mem_dir.children {
            children_entry
        } else {
            return Err(ChiconError::BadPath);
        };
        if let Some(child_entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
            if path_iter.clone().peekable().peek().is_some() {
                child_entry.remove(path_iter.collect(), entry_type, force)
            } else {
                // check force
                if let MemDirEntry::Directory(dir_entry) = child_entry {
                    let dir_internal = dir_entry.0.try_borrow()?;
                    if let Some(dir_children) = &dir_internal.children {
                        if !dir_children.is_empty() && !force {
                            return Err(ChiconError::MemDirNotFound(
                                dir_internal.complete_path.clone(),
                            ));
                        }
                    }
                }
                children
                    .remove(&current_path.to_string_lossy().into_owned())
                    .map(|_| ())
                    .ok_or(ChiconError::BadPath)
            }
        } else {
            Err(ChiconError::BadPath)
        }
    }

    fn rename(
        &mut self,
        path: PathBuf,
        new_path: PathBuf,
        complete_path: PathBuf,
    ) -> Result<(), ChiconError> {
        let mut path_iter = path.iter();
        let current_path = path_iter.next().ok_or(ChiconError::BadPath)?;
        let mut new_path_iter = new_path.iter();
        let current_new_path = new_path_iter.next().ok_or(ChiconError::BadPath)?;

        let mut mem_dir = self.0.try_borrow_mut()?;
        let children = if let Some(children_entry) = &mut mem_dir.children {
            children_entry
        } else {
            return Err(ChiconError::BadPath);
        };
        if let Some(child_entry) = children.get_mut(&current_path.to_string_lossy().into_owned()) {
            if path_iter.clone().peekable().peek().is_some() {
                child_entry.rename(path_iter.collect(), new_path_iter.collect(), complete_path)
            } else {
                // check force
                match child_entry {
                    MemDirEntry::Directory(dir_entry) => {
                        {
                            let mut dir_internal = dir_entry.0.try_borrow_mut()?;
                            dir_internal.complete_path = complete_path;
                        }
                        let dir_entry_cloned = dir_entry.clone();
                        children.remove(&current_path.to_string_lossy().into_owned());
                        children.insert(
                            current_new_path.to_string_lossy().into_owned(),
                            MemDirEntry::Directory(dir_entry_cloned),
                        );
                        Ok(())
                    }
                    MemDirEntry::File(file_entry) => {
                        {
                            let mut file_internal = file_entry.0.try_borrow_mut()?;
                            file_internal.complete_path = complete_path;
                        }
                        let file_entry_cloned = file_entry.clone();
                        children.remove(&current_path.to_string_lossy().into_owned());
                        children.insert(
                            current_new_path.to_string_lossy().into_owned(),
                            MemDirEntry::File(file_entry_cloned),
                        );
                        Ok(())
                    }
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
    fn test_fs_internals_insert_file_in_dir() {
        let mem_fs = MemFileSystem::new();
        mem_fs.insert_file(PathBuf::from("test.test")).unwrap();

        let found = mem_fs.get_from_relative_path(PathBuf::from("test.test"));
        assert!(found.is_some());
        let mem_file = found.unwrap();

        assert_eq!(mem_file.file_type().unwrap(), FileType::File);
        assert_eq!(mem_file.path().unwrap(), PathBuf::from("test.test"));

        mem_fs.create_dir_all("test/other/chicon").unwrap();
        assert_eq!(mem_fs.read_dir("test/other").unwrap().len(), 1);
        mem_fs.create_file("test/other/chicon/rs.txt").unwrap();

        mem_fs.open_file("test/other/chicon/rs.txt").unwrap();
    }

    #[test]
    fn test_fs_internals_insert_file() {
        let mem_fs = MemFileSystem::new();
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
        let mem_fs = MemFileSystem::new();
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
        let mem_fs = MemFileSystem::new();
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
        let mem_fs = MemFileSystem::new();
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

        file.write_all(b"Blabla").unwrap();
        file.sync_all().unwrap();
        file.read_to_string(&mut buffer).unwrap();

        assert_eq!(buffer, String::from("coucoutoiBlabla"));
    }

    #[test]
    fn test_remove_file() {
        let mem_fs = MemFileSystem::new();
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
        let mem_fs = MemFileSystem::new();
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        assert!(mem_fs.remove_dir("share").is_err());
        assert!(mem_fs.remove_dir("share/testmemreaddir").is_err());
        mem_fs.remove_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .remove_file("share/testmemreaddir/myotherfile")
            .unwrap();
        mem_fs.remove_dir("share/testmemreaddir").unwrap();

        assert!(mem_fs.read_dir("share/testmemreaddir").is_err());
    }

    #[test]
    fn test_remove_dir_all() {
        let mem_fs = MemFileSystem::new();
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        assert!(mem_fs.remove_dir("share").is_err());
        mem_fs.remove_dir_all("share/testmemreaddir").unwrap();
    }

    #[test]
    fn test_rename() {
        let mem_fs = MemFileSystem::new();
        assert!(mem_fs.create_dir("share/testmemreaddir").is_err());
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddir").unwrap();
        mem_fs.create_dir_all("share/testmemreaddirother").unwrap();
        mem_fs.create_file("share/testmemreaddir/myfile").unwrap();
        mem_fs
            .create_file("share/testmemreaddir/myotherfile")
            .unwrap();

        assert!(mem_fs.remove_dir("share").is_err());
        mem_fs
            .rename(
                "share/testmemreaddir/myotherfile",
                "share/testmemreaddir/myotherfilebis",
            )
            .unwrap();
        mem_fs
            .remove_file("share/testmemreaddir/myotherfilebis")
            .unwrap();
        assert!(mem_fs
            .remove_file("share/testmemreaddir/myotherfile")
            .is_err());
        mem_fs
            .rename(
                "share/testmemreaddirother",
                "share/testmemreaddirotherrenamed",
            )
            .unwrap();
        mem_fs
            .remove_dir_all("share/testmemreaddirotherrenamed")
            .unwrap();
        assert!(mem_fs.remove_dir_all("share/testmemreaddirother").is_err());
    }

}
