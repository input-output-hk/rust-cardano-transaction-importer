use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Transaction {
    pub txid: String,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Input {
    pub id: String,
    pub index: i32
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Output {
    pub address: String,
    //FIXME: This should probably by a u64
    pub value: i64,
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Error {
    DatabaseError(rusqlite::Error),
    ConnectionError(reqwest::Error),
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::DatabaseError(error)
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::ConnectionError(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::DatabaseError(ref err) => fmt::Display::fmt(err, f),
            Error::ConnectionError(ref err) => fmt::Display::fmt(err, f),
        }
    }
}