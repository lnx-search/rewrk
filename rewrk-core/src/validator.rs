use std::borrow::Cow;
use std::fmt::Debug;

use http::response::Parts;
use hyper::body::Bytes;

#[derive(Debug, thiserror::Error, Clone)]
/// The provided request is invalid and should not be counted.
pub enum ValidationError {
    #[error("The returned status code is not valid: {0}")]
    InvalidStatus(u16),
    #[error("The body of the request does not match the expected structure: {0}")]
    InvalidBody(Cow<'static, str>),
    #[error("The request is missing a required header: {0}")]
    MissingHeader(Cow<'static, str>),
    #[error("The request contained a header, but it was invalid: {0}")]
    InvalidHeader(Cow<'static, str>),
    #[error("The connection was aborted by the remote server.")]
    ConnectionAborted,
    #[error("The connection took to long to respond.")]
    Timeout,
    #[error("A validation error rejected the request: {0}")]
    Other(Cow<'static, str>),
}

/// A validating utility for checking responses returned by the webserver are correct.
///
/// It's important that these operations are light weight as they are called on the same
/// runtime as the request runtime which may block operations.
pub trait ResponseValidator: Send + Sync + 'static {
    fn validate(&self, head: Parts, body: Bytes) -> Result<(), ValidationError>;
}

#[derive(Debug)]
/// The default validator handler.
pub struct DefaultValidator;

impl ResponseValidator for DefaultValidator {
    fn validate(&self, head: Parts, _body: Bytes) -> Result<(), ValidationError> {
        if head.status.is_success() {
            Ok(())
        } else {
            Err(ValidationError::InvalidStatus(head.status.as_u16()))
        }
    }
}
