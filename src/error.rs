use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced in this library.
#[derive(Debug, Error)]
pub enum Error {
    #[error("saucer error: {0}")]
    Saucer(i32),
}
