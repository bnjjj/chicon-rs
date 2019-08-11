use std::convert::TryInto;
use std::fs::Permissions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use ssh2::Session;

use crate::error::ChiconError;
use crate::{DirEntry, File, FileSystem, FileType};

struct SSHSession {
    // Only useful to not drop connection
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

/// Structure implementing `FileSystem` trait to store on a SSH server (via scp)
pub struct SSHFileSystem<'a> {
    username: String,
    addr: String,
    passphrase: Option<&'a str>,
    private_key: PathBuf,
    public_key: PathBuf,
}
impl<'a> SSHFileSystem<'a> {
    pub fn new<P: AsRef<Path>>(
        addr: String,
        username: String,
        passphrase: Option<&'a str>,
        private_key: P,
        public_key: P,
    ) -> Self {
        let private_key = private_key.as_ref();
        let public_key = public_key.as_ref();

        SSHFileSystem {
            username,
            passphrase,
            private_key: PathBuf::from(private_key),
            public_key: PathBuf::from(public_key),
            addr,
        }
    }
}
impl<'a> FileSystem for SSHFileSystem<'a> {
    type FSError = ChiconError;
    type File = SSHFile<'a>;
    type DirEntry = SSHDirEntry;

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

        let mut chan = session.channel_session()?;
        chan.exec(
            format!(
                "chmod {} {}",
                perm.mode(),
                path.to_str().ok_or(ChiconError::BadPath)?
            )
            .as_str(),
        )?;
        let mut output = String::new();
        chan.read_to_string(&mut output)?;
        chan.wait_eof()?;
        chan.close()?;
        chan.wait_close()?;

        if chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }
        Ok(())
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

        let mut my_chan = session.channel_session()?;
        my_chan.exec(format!("touch {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(SSHFile::new(
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
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();

        let mut my_chan = session.channel_session()?;
        my_chan.exec(format!("mkdir {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(())
    }

    fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();
        let mut my_chan = session.channel_session()?;

        my_chan
            .exec(format!("mkdir -p {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(())
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
        let mut my_chan = session.channel_session()?;

        my_chan.exec(format!("cat {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(SSHFile::new(
            PathBuf::from(path),
            output.into_bytes(),
            self.addr.clone(),
            self.username.clone(),
            self.passphrase,
            &self.private_key,
            &self.public_key,
        ))
    }

    fn read_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<Self::DirEntry>, Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();
        let mut my_chan = session.channel_session()?;

        my_chan.exec(format!("ls -Ap {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        let mut entries: Vec<Self::DirEntry> = Vec::new();
        for entry in output.split_whitespace() {
            entries.push(SSHDirEntry::new(path, entry))
        }

        Ok(entries)
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();
        let mut my_chan = session.channel_session()?;

        my_chan.exec(format!("rm -f {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(())
    }

    fn remove_dir<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        self.remove_dir_all(path)
    }

    fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<(), Self::FSError> {
        let path = path.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();
        let mut my_chan = session.channel_session()?;

        my_chan.exec(format!("rm -rf {}", path.to_str().ok_or(ChiconError::BadPath)?).as_str())?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(())
    }

    fn rename<P: AsRef<Path>>(&self, from: P, to: P) -> Result<(), Self::FSError> {
        let from = from.as_ref();
        let to = to.as_ref();
        let ssh_session = SSHSession::new(
            self.addr.clone(),
            &self.username,
            self.passphrase,
            self.private_key.as_path(),
            self.public_key.as_path(),
        )?;
        let session = ssh_session.session();
        let mut my_chan = session.channel_session()?;

        my_chan.exec(
            format!(
                "mv -f {} {}",
                from.to_str().ok_or(ChiconError::BadPath)?,
                to.to_str().ok_or(ChiconError::BadPath)?
            )
            .as_str(),
        )?;
        let mut output = String::new();
        my_chan.read_to_string(&mut output)?;
        my_chan.wait_eof()?;
        my_chan.close()?;
        my_chan.wait_close()?;

        if my_chan.exit_status()? != 0 {
            return Err(ChiconError::SSHExecutionError(output));
        }

        Ok(())
    }
}

/// Structure implementing `File` trait to represent a file on a SSH server (via scp)
pub struct SSHFile<'a> {
    filename: PathBuf,
    content: Vec<u8>,
    addr: String,
    username: String,
    passphrase: Option<&'a str>,
    private_key: PathBuf,
    public_key: PathBuf,
    offset: u64,
    bytes_read: u64,
}
impl<'a> SSHFile<'a> {
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

        SSHFile {
            filename,
            content,
            username,
            passphrase,
            private_key: PathBuf::from(private_key),
            public_key: PathBuf::from(public_key),
            addr,
            offset: 0,
            bytes_read: 0,
        }
    }
}
impl<'a> File for SSHFile<'a> {
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

        let mut chan = session.scp_send(
            self.filename.as_path(),
            0o755,
            self.content.len().try_into().unwrap(),
            None,
        )?;

        chan.write_all(self.content.as_slice())?;
        chan.send_eof()?;
        chan.wait_eof()?;
        chan.close()?;
        chan.wait_close().map_err(ChiconError::from)
    }
}

impl<'a> Read for SSHFile<'a> {
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
impl<'a> Write for SSHFile<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.content.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.content.flush()
    }
}
impl<'a> Seek for SSHFile<'a> {
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

/// Structure implementing `DirEntry` trait to represent an entry in a directory on a SSH server
pub struct SSHDirEntry {
    path: PathBuf,
    file_type: FileType,
}
impl SSHDirEntry {
    pub fn new(root_path: &Path, raw_path: &str) -> Self {
        let file_type = if raw_path.ends_with('/') {
            FileType::Directory
        } else {
            FileType::File
        };

        SSHDirEntry {
            file_type,
            path: root_path.join(raw_path.trim_end_matches('/')),
        }
    }
}
impl DirEntry for SSHDirEntry {
    type FSError = ssh2::Error;

    fn path(&self) -> Result<PathBuf, Self::FSError> {
        Ok(self.path.clone())
    }

    fn file_type(&self) -> Result<FileType, Self::FSError> {
        Ok(self.file_type.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_create_dir() {
        let ssh_fs = SSHFileSystem::new(
            String::from("127.0.0.1:22"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        ssh_fs.create_dir("share/testsshcreatetest").unwrap();
        ssh_fs.remove_dir("share/testsshcreatetest").unwrap();
    }

    #[test]
    fn test_read_dir() {
        let ssh_fs = SSHFileSystem::new(
            String::from("127.0.0.1:22"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        ssh_fs.create_dir("share/testsshreaddirtest").unwrap();
        ssh_fs
            .create_file("share/testsshreaddirtest/myfile")
            .unwrap();

        let res = ssh_fs.read_dir("share/testsshreaddirtest").unwrap();
        assert_eq!(1, res.len());
        assert_eq!(
            PathBuf::from(String::from("share/testsshreaddirtest/myfile")),
            res.get(0).unwrap().path().unwrap()
        );

        ssh_fs
            .remove_file("share/testsshreaddirtest/myfile")
            .unwrap();
        ssh_fs.remove_dir("share/testsshreaddirtest").unwrap();
    }

    #[test]
    fn test_full_flow() {
        let ssh_fs = SSHFileSystem::new(
            String::from("127.0.0.1:22"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
        );

        let _res = ssh_fs.read_dir(".").unwrap();
        ssh_fs.create_dir("share/testsshfulltest").unwrap();
        ssh_fs.remove_dir("share/testsshfulltest").unwrap();

        let mut file_created = ssh_fs.create_file("share/testsshfull.test").unwrap();
        file_created.write_all(b"Coucou c'est moi").unwrap();
        file_created.sync_all().unwrap();

        let mut file = ssh_fs.open_file("share/testsshfull.test").unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        ssh_fs.remove_file("share/testsshfull.test").unwrap();
    }

    #[test]
    fn test_remove_dir_all() {
        let ssh_fs = SSHFileSystem::new(
            String::from("127.0.0.1:22"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PUBLIC_KEY environment variable must be set"),
        );

        ssh_fs.create_dir("share/testsshremovedirtest").unwrap();
        ssh_fs
            .create_file("share/testsshremovedirtest/myfile")
            .unwrap();

        ssh_fs.remove_dir_all("share/testsshremovedirtest").unwrap();
    }

    #[test]
    fn test_seek_file() {
        let ssh_fs = SSHFileSystem::new(
            String::from("127.0.0.1:22"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PUBLIC_KEY environment variable must be set"),
        );
        {
            let mut file = ssh_fs.create_file("testseek.test").unwrap();
            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
        }

        let mut content = String::new();
        {
            let mut new_file = ssh_fs.open_file("testseek.test").unwrap();
            new_file.seek(SeekFrom::Start(2)).unwrap();
            new_file.read_to_string(&mut content).unwrap();
        }
        assert_eq!(String::from("ucoutoi"), content);

        ssh_fs.remove_file("testseek.test").unwrap();
    }

    #[test]
    fn test_seek_end_file() {
        let ssh_fs = SSHFileSystem::new(
            String::from("127.0.0.1:22"),
            env::var("SSH_USER").expect("SSH_USER environment variable must be set"),
            None,
            env::var("SSH_PRIVATE_KEY").expect("SSH_PRIVATE_KEY environment variable must be set"),
            env::var("SSH_PUBLIC_KEY").expect("SSH_PUBLIC_KEY environment variable must be set"),
        );
        {
            let mut file = ssh_fs.create_file("testseekend.test").unwrap();
            file.write_all(String::from("coucoutoi").as_bytes())
                .unwrap();
            file.sync_all().unwrap();
        }

        let mut content = String::new();
        {
            let mut new_file = ssh_fs.open_file("testseekend.test").unwrap();
            assert_eq!(new_file.seek(SeekFrom::End(2)).unwrap(), 11);
            assert_eq!(new_file.seek(SeekFrom::End(-2)).unwrap(), 7);
            new_file.read_to_string(&mut content).unwrap();
        }
        assert_eq!(String::from("oi"), content);
        ssh_fs.remove_file("testseekend.test").unwrap();
    }
}
