use thiserror::Error;

/// All errors generated in the barter::portfolio module.
#[derive(Error, Debug, Clone)]
pub enum ErrorRepr {
    #[error("out of bounds, {}", .0)]
    OutOfBounds(&'static str),
    #[error("not exists, {}", .0)]
    NotExists(&'static str),
}
