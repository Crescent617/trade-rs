use thiserror::Error;

/// All errors generated in the barter::portfolio module.
#[derive(Error, Debug, Clone)]
pub enum ErrorRepr {
    #[error("out of bounds, {}", .0)]
    OutOfBounds(String),
    #[error("not exists, {}", .0)]
    NotExists(&'static str),
    #[error("not satisfied: {}", .0)]
    NotSatisfied(&'static str),
    #[error("expired: {}", .0)]
    OrderExpired(String),
}
