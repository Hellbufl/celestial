// use std::{error::Error, fmt};
use std::fmt;
use thiserror::Error;
// use std::ops::FromResidual;

#[derive(Debug, Error)]
pub enum Error {
    FileReadFailed(),
    FileDecodeFailed(),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let err_msg = match self {
            Error::FileReadFailed() => "Failed to read file!",
            Error::FileDecodeFailed() => "Failed to decode file!",
        };
        write!(f, "{err_msg}")
    }
}

// impl FromResidual<Result<Infallible, serde_binary::Error>> for Error {

// }

impl From<std::io::Error> for Error {
    fn from(_error: std::io::Error) -> Self {
        Error::FileReadFailed()
    }
}

impl From<serde_binary::Error> for Error {
    fn from(_error: serde_binary::Error) -> Self {
        Error::FileDecodeFailed()
    }
}