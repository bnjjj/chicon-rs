use std::fmt;

use rusoto_core::RusotoError;
use rusoto_s3::{
    CopyObjectError, DeleteObjectError, GetObjectError, ListObjectsV2Error, PutObjectError,
};
use ssh2;

/// Possible errors which can occured during execution
pub enum ChiconError {
    IOError(std::io::Error),
    DirectoryNotEmpty,
    RelativePath,
    BadPath,
    RusotoGetObjectError(RusotoError<GetObjectError>),
    RusotoPutObjectError(RusotoError<PutObjectError>),
    RusotoDeleteObjectError(RusotoError<DeleteObjectError>),
    RusotoCopyObjectError(RusotoError<CopyObjectError>),
    RusotoListObjectsV2Error(RusotoError<ListObjectsV2Error>),
    SSHError(ssh2::Error),
    SSHExecutionError(String),
    SFTPError,
    OpenstackError(osauth::Error),
}

impl fmt::Debug for ChiconError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChiconError::IOError(err) => write!(f, "IO error: {:?}", err),
            ChiconError::DirectoryNotEmpty => write!(
                f,
                "Directory is not empty. Try remove_dir_all method to force delete"
            ),
            ChiconError::RelativePath => write!(f, "path must be absolute and not relative"),
            ChiconError::BadPath => write!(f, "path is incorrect"),
            ChiconError::RusotoGetObjectError(err) => {
                write!(f, "Rusoto GetObjectError error: {:?}", err)
            }
            ChiconError::RusotoPutObjectError(err) => {
                write!(f, "Rusoto RusotoPutObjectError error: {:?}", err)
            }
            ChiconError::RusotoDeleteObjectError(err) => {
                write!(f, "Rusoto RusotoDeleteObjectError error: {:?}", err)
            }
            ChiconError::RusotoCopyObjectError(err) => {
                write!(f, "Rusoto RusotoCopyObjectError error: {:?}", err)
            }
            ChiconError::RusotoListObjectsV2Error(err) => {
                write!(f, "Rusoto RusotoListObjectsV2Error error: {:?}", err)
            }
            ChiconError::SSHError(err) => write!(f, "SSH error: {:?}", err),
            ChiconError::SSHExecutionError(output) => {
                write!(f, "SSH execution error: {:?}", output)
            }
            ChiconError::SFTPError => write!(f, "SFTP error"),
            ChiconError::OpenstackError(err) => write!(f, "Openstack error : {:?}", err),
        }
    }
}

impl From<std::io::Error> for ChiconError {
    fn from(err: std::io::Error) -> Self {
        ChiconError::IOError(err)
    }
}
impl From<RusotoError<GetObjectError>> for ChiconError {
    fn from(err: RusotoError<GetObjectError>) -> Self {
        ChiconError::RusotoGetObjectError(err)
    }
}
impl From<RusotoError<PutObjectError>> for ChiconError {
    fn from(err: RusotoError<PutObjectError>) -> Self {
        ChiconError::RusotoPutObjectError(err)
    }
}
impl From<RusotoError<DeleteObjectError>> for ChiconError {
    fn from(err: RusotoError<DeleteObjectError>) -> Self {
        ChiconError::RusotoDeleteObjectError(err)
    }
}
impl From<RusotoError<CopyObjectError>> for ChiconError {
    fn from(err: RusotoError<CopyObjectError>) -> Self {
        ChiconError::RusotoCopyObjectError(err)
    }
}
impl From<RusotoError<ListObjectsV2Error>> for ChiconError {
    fn from(err: RusotoError<ListObjectsV2Error>) -> Self {
        ChiconError::RusotoListObjectsV2Error(err)
    }
}
impl From<ssh2::Error> for ChiconError {
    fn from(err: ssh2::Error) -> Self {
        ChiconError::SSHError(err)
    }
}
impl From<osauth::Error> for ChiconError {
    fn from(err: osauth::Error) -> Self {
        ChiconError::OpenstackError(err)
    }
}