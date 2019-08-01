use std::cell::{BorrowError, BorrowMutError};
use std::path::PathBuf;

use rusoto_core::RusotoError;
use rusoto_s3::{
    CopyObjectError, DeleteObjectError, DeleteObjectsError, GetObjectError, ListObjectsV2Error, PutObjectError,
};
use ssh2;

macro_rules! from_error {
    ($type:ty, $target:ident, $targetvar:expr) => {
        impl From<$type> for $target {
            fn from(s: $type) -> Self {
                $targetvar(s.into())
            }
        }
    };
}

/// Possible errors which can occured during execution
#[derive(Fail, Debug)]
pub enum ChiconError {
    #[fail(display = "IO error: {:?}", _0)]
    IOError(std::io::Error),
    #[fail(display = "Directory is not empty. Try remove_dir_all method to force delete")]
    DirectoryNotEmpty,
    #[fail(display = "path must be absolute and not relative")]
    RelativePath,
    #[fail(display = "path is incorrect or do not exist")]
    BadPath,
    #[fail(display = "Rusoto GetObjectError error: {:?}", _0)]
    RusotoGetObjectError(RusotoError<GetObjectError>),
    #[fail(display = "Rusoto PutObjectError error: {:?}", _0)]
    RusotoPutObjectError(RusotoError<PutObjectError>),
    #[fail(display = "Rusoto DeleteObjectError error: {:?}", _0)]
    RusotoDeleteObjectError(RusotoError<DeleteObjectError>),
    #[fail(display = "Rusoto DeleteObjectsError error: {:?}", _0)]
    RusotoDeleteObjectsError(RusotoError<DeleteObjectsError>),
    #[fail(display = "Rusoto CopyObjectError error: {:?}", _0)]
    RusotoCopyObjectError(RusotoError<CopyObjectError>),
    #[fail(display = "Rusoto ListObjectsV2Error error: {:?}", _0)]
    RusotoListObjectsV2Error(RusotoError<ListObjectsV2Error>),
    #[fail(display = "SSH error: {:?}", _0)]
    SSHError(ssh2::Error),
    #[fail(display = "SSH execution error: {:?}", _0)]
    SSHExecutionError(String),
    #[fail(display = "SFTP error")]
    SFTPError,
    #[fail(display = "Openstack error: {:?}", _0)]
    OpenstackError(osauth::Error),
    #[fail(display = "Borrow error {:?}", _0)]
    BorrowError(BorrowError),
    #[fail(display = "Borrow mut error {:?}", _0)]
    BorrowMutError(BorrowMutError),
    #[fail(display = "Error memory file not found: {:?}", _0)]
    MemFileNotFound(PathBuf),
    #[fail(display = "Error memory directory not found: {:?}", _0)]
    MemDirNotFound(PathBuf),
    #[fail(display = "Error memory directory is not empty: {:?}", _0)]
    MemDirNotEmpty(PathBuf),
}

from_error!(std::io::Error, ChiconError, ChiconError::IOError);
from_error!(ssh2::Error, ChiconError, ChiconError::SSHError);
from_error!(
    RusotoError<GetObjectError>,
    ChiconError,
    ChiconError::RusotoGetObjectError
);
from_error!(
    RusotoError<PutObjectError>,
    ChiconError,
    ChiconError::RusotoPutObjectError
);
from_error!(
    RusotoError<DeleteObjectError>,
    ChiconError,
    ChiconError::RusotoDeleteObjectError
);
from_error!(
    RusotoError<DeleteObjectsError>,
    ChiconError,
    ChiconError::RusotoDeleteObjectsError
);
from_error!(
    RusotoError<CopyObjectError>,
    ChiconError,
    ChiconError::RusotoCopyObjectError
);
from_error!(
    RusotoError<ListObjectsV2Error>,
    ChiconError,
    ChiconError::RusotoListObjectsV2Error
);
from_error!(osauth::Error, ChiconError, ChiconError::OpenstackError);
from_error!(BorrowError, ChiconError, ChiconError::BorrowError);
from_error!(BorrowMutError, ChiconError, ChiconError::BorrowMutError);
