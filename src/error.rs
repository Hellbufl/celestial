// use std::{error::Error, fmt};
use std::fmt::{self, Debug};
use thiserror::Error;
// use std::ops::FromResidual;

#[derive(Debug, Error)]
pub enum Error {
    IO {
        msg: String,
    },
    Parse {
        msg: String,
    },
    Binary {
        msg: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let err_msg = match self {
            Error::IO{ msg } => format!("Failed to read/write file!: {}", msg),
            Error::Parse{ msg } => format!("Failed to parse file!: {}", msg),
            Error::Binary{ msg } => format!("Failed to decode file!: {}", msg),
        };
        write!(f, "{err_msg}")
    }
}

// impl FromResidual<Result<Infallible, serde_binary::Error>> for Error {

// }

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IO{ msg: error.to_string() }
    }
}

impl From<serde_binary::Error> for Error {
    fn from(error: serde_binary::Error) -> Self {
        Error::Binary{ msg: error.to_string() }
    }
}

impl From<ini::Error> for Error {
    fn from(error: ini::Error) -> Self {
        match error {
            ini::Error::Io(e) => { Error::IO{ msg: e.to_string() } },
            ini::Error::Parse(e) => { Error::Parse { msg: e.to_string() } },
        }
    }
}