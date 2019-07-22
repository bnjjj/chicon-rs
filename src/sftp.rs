use std::fs::Permissions;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use ssh2::{FileStat, OpenFlags, Session};

use crate::error::ChiconError;
use crate::{DirEntry, File as FsFile, FileSystem, FileType};

struct SSHSession {
    // Only usefull to not drop connection
    _tcp_stream: TcpStream,
    session: Session,
}
impl SSHSession {
    fn new<P: AsRef<Path>>(
        addr: String,
        username: &str,
        passphrase: Option<&str>,
        private_key: P,
        public_key: P,
    ) -> Result<Self, ChiconError> {
        let private_key = private_key.as_ref();
        let public_key = public_key.as_ref();

        let tcp_stream = TcpStream::connect(addr)?;
        let mut session = Session::new().ok_or(ChiconError::SFTPError)?;
        session.handshake(&tcp_stream)?;
        session.userauth_pubkey_file(username, Some(public_key), private_key, passphrase)?;

        Ok(SSHSession {
            _tcp_stream: tcp_stream,
            session,
        })
    }

    fn session(&self) -> &Session {
        &self.session
    }
}

/// Structure implementing `FileSystem` trait to store on a SFTP server
pub struct SFTPFileSystem<'a> {
    username: String,
    addr: String,
    passphrase: Option<&'a str>,
    private_key: PathBuf,
    public_key: PathBuf,
}
impl<'a> SFTPFileSystem<'a> {
    pub fn new<P: AsRef<Path>>(
        addr: String,
        username: String,
        passphrase: Option<&'a str>,
        private_key: P,
        public_key: P,
    ) -> Self {
        let private_key = private_key.as_ref();
        let public_key = public_key.as_ref();

        SFTPFileSystem {
            username,
            passphrase,
            private_key: PathBuf::from(private_key),
            public_key: PathBuf::from(public_key),
            addr,
        }
    }
}
impl<'a> FileSystem for SFTPFileSystem<'a> {
    type FSError = ChiconError;
    type File = SFTPFile<'a>;
    type DirEntry = SFTPDirEntry;

    fn chmod<P: AsRef<Path>>(&self, path: P, perm: Permissions) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp()?;
        sftp.create(path)?;

        let file_stat = sftp.stat(path)?;
        let stat = FileStat {
            perm: Some(perm.mode()),
            ..file_stat
        };

        sftp.setstat(path, stat).map_err(ChiconError::from)
    }

    fn create_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp()?;
        sftp.create(path)?;

        Ok(SFTPFile::new(
            PathBuf::from(path),
            Vec::<u8>::new(),
            self.addr.clone(),
            self.username.clone(),
            self.passphrase,
            &self.private_key,
            &self.public_key,
        ))
    }

    fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp()?;
        sftp.mkdir(path.as_ref(), 0o755)
            .map(|_| ())
            .map_err(ChiconError::from)
    }

    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        self.create_dir(path).map(|_| ()).map_err(ChiconError::from)
    }

    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<Self::File, Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp()?;
        let mut content: Vec<u8> = Vec::new();
        {
            let mut file = sftp.open(path)?;
            file.read_to_end(&mut content)?;
        }
        Ok(SFTPFile::new(
            PathBuf::from(path),
            content,
            self.addr.clone(),
            self.username.clone(),
            self.passphrase,
            &self.private_key,
            &self.public_key,
        ))
    }

    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp().map_err(ChiconError::from)?;
        let dir_entries = sftp.readdir(path.as_ref()).map_err(ChiconError::from)?;

        Ok(dir_entries.into_iter().map(SFTPDirEntry::from).collect())
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp().map_err(ChiconError::from)?;
        sftp.unlink(path.as_ref()).map_err(ChiconError::from)
    }

    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();
        let sftp = session.sftp().map_err(ChiconError::from)?;

        sftp.rmdir(path.as_ref()).map_err(ChiconError::from)
    }

    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();

        let dir_entries = self.read_dir(path)?;
        for dir in dir_entries {
            match dir.file_type()? {
                FileType::Directory => self.remove_dir_all(dir.path()?.as_path())?,
                FileType::File | FileType::Symlink => self.remove_file(dir.path()?.as_path())?,
            }
        }

        self.remove_dir(path)
    }

    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let sftp = session.sftp().map_err(ChiconError::from)?;
        sftp.rename(from.as_ref(), to.as_ref(), None)
            .map_err(ChiconError::from)
    }
}

/// Structure implementing `File` trait to represent a file on a SFTP server
pub struct SFTPFile<'a> {
    filename: PathBuf,
    content: Vec<u8>,
    addr: String,
    username: String,
    passphrase: Option<&'a str>,
    private_key: PathBuf,
    public_key: PathBuf,
}
impl<'a> SFTPFile<'a> {
    fn new<P>(
        filename: PathBuf,
        content: Vec<u8>,
        addr: String,
        username: String,
        passphrase: Option<&'a str>,
        private_key: P,
        public_key: P,
    ) -> Self
    where
        P: AsRef<Path>,
    {
        let private_key = private_key.as_ref();
        let public_key = public_key.as_ref();

        SFTPFile {
            filename,
            content,
            username,
            passphrase,
            private_key: PathBuf::from(private_key),
            public_key: PathBuf::from(public_key),
            addr,
        }
    }
}
impl<'a> FsFile for SFTPFile<'a> {
    type FSError = ChiconError;

    fn sync_all(&mut self) -> Result<(), Self::FSError> {
        let tcp_stream = TcpStream::connect(self.addr.clone())?;
        let mut session = Session::new().ok_or(ChiconError::SFTPError)?;
        session.handshake(&tcp_stream)?;
        session.userauth_pubkey_file(
            &self.username,
            Some(self.public_key.as_path()),
            self.private_key.as_path(),
            self.passphrase,
        )?;
        let sftp = session.sftp()?;

        // Bits comming from https://docs.rs/libssh2-sys/0.1.33/libssh2_sys/
        let mut file = sftp.open_mode(
            &self.filename,
            OpenFlags::from_bits(2 | 16).unwrap(),
            0o755,
            ssh2::OpenType::File,
        )?;
        file.write_all(self.content.as_slice())?;
        file.fsync().map_err(ChiconError::from)
    }
}

impl<'a> Read for SFTPFile<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut content_slice = self.content.as_slice();
        let nb = content_slice.read(buf)?;
        self.content = content_slice.to_vec();
        Ok(nb)
    }
}
impl<'a> Write for SFTPFile<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.content.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.content.flush()
    }
}

/// Structure implementing `DirEntry` trait to represent an entry in a directory on a SFTP server
pub struct SFTPDirEntry {
    path: PathBuf,
    stat: FileStat,
}
impl DirEntry for SFTPDirEntry {
    type FSError = ssh2::Error;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        Ok(self.path.clone())
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        if self.stat.is_dir() {
            Ok(FileType::Directory)
        } else if self.stat.is_file() {
            Ok(FileType::File)
        } else {
            Ok(FileType::Symlink)
        }
    }
}

impl From<(PathBuf, FileStat)> for SFTPDirEntry {
    fn from(dir_entry: (PathBuf, FileStat)) -> Self {
        SFTPDirEntry {
            path: dir_entry.0,
            stat: dir_entry.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_create_dir() {
        let sftp_fs = SFTPFileSystem::new(
            String::from("127.0.0.1:2222"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        sftp_fs.create_dir("share/testcreatetest").unwrap();
        sftp_fs.remove_dir("share/testcreatetest").unwrap();
    }

    #[test]
    fn test_read_dir() {
        let sftp_fs = SFTPFileSystem::new(
            String::from("127.0.0.1:2222"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        sftp_fs.create_dir("share/testreaddirtest").unwrap();
        sftp_fs.create_file("share/testreaddirtest/myfile").unwrap();

        let res = sftp_fs.read_dir("share/testreaddirtest").unwrap();
        assert_eq!(1, res.len());
        assert_eq!(
            PathBuf::from(String::from("share/testreaddirtest/myfile")),
            res.get(0).unwrap().path().unwrap()
        );

        sftp_fs.remove_file("share/testreaddirtest/myfile").unwrap();
        sftp_fs.remove_dir("share/testreaddirtest").unwrap();
    }

    #[test]
    fn test_full_flow() {
        let sftp_fs = SFTPFileSystem::new(
            String::from("127.0.0.1:2222"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        let _res = sftp_fs.read_dir(".").unwrap();
        sftp_fs.create_dir("share/testfulltest").unwrap();
        sftp_fs.remove_dir("share/testfulltest").unwrap();

        let mut file_created = sftp_fs.create_file("share/testfull.test").unwrap();
        file_created.write_all(b"Coucou c'est moi").unwrap();
        file_created.sync_all().unwrap();

        let mut file = sftp_fs.open_file("share/testfull.test").unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        sftp_fs.remove_file("share/testfull.test").unwrap();
    }

    #[test]
    fn test_remove_dir_all() {
        let sftp_fs = SFTPFileSystem::new(
            String::from("127.0.0.1:2222"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        sftp_fs.create_dir("share/testremovedirtest").unwrap();
        sftp_fs
            .create_file("share/testremovedirtest/myfile")
            .unwrap();

        sftp_fs.remove_dir_all("share/testremovedirtest").unwrap();
    }
}
