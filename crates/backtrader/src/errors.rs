use thiserror::Error;

/// All errors generated in the barter::portfolio module.
#[derive(Error, Debug)]
pub enum ErrorRepr {
    #[error("Out of bounds, {}", .0)]
    OutOfBounds(&'static str),
}
