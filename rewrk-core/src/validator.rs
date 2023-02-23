use std::borrow::Cow;
use std::fmt::Debug;

use http::response::Parts;
use http::HeaderMap;
use hyper::body::Bytes;

use crate::RequestKey;

#[derive(Debug, thiserror::Error, Clone)]
/// The provided request is invalid and should not be counted.
pub enum ValidationError {
    #[error("The returned status code is not valid: {0}")]
    /// The returned status code is not valid
    InvalidStatus(u16, HeaderMap),
    #[error("The body of the request does not match the expected structure: {0}")]
    /// The body of the request does not match the expected structure
    InvalidBody(Cow<'static, str>),
    #[error("The request is missing a required header: {0}")]
    /// The request is missing a required header
    MissingHeader(Cow<'static, str>),
    #[error("The request contained a header, but it was invalid: {0}")]
    /// The request contained a header, but it was invalid
    InvalidHeader(Cow<'static, str>),
    #[error("The connection was aborted by the remote serve.")]
    /// The connection was aborted by the remote server
    ConnectionAborted,
    #[error("The connection took to long to respond")]
    /// The connection took to long to respond
    Timeout,
    #[error("A validation error rejected the request: {0}")]
    /// A validation error rejected the request
    Other(Cow<'static, str>),
}

/// A validating utility for checking responses returned by the webserver are correct.
///
/// It's important that these operations are light weight as they are called on the same
/// runtime as the request runtime which may block operations.
///
/// # Example
///
/// This example is just the [DefaultValidator] implementation, it can do as much or
/// as little as you'd like but it's important that it does not block heavily as it
/// will reduce the overall throughput of the worker thread.
///
/// ```
/// use http::response::Parts;
/// use hyper::body::Bytes;
/// use rewrk_core::{RequestKey, ResponseValidator, ValidationError};
///
/// #[derive(Debug)]
/// pub struct DefaultValidator;
///
/// impl ResponseValidator for DefaultValidator {
///     fn validate(&self, request_id: RequestKey, head: Parts, _body: Bytes) -> Result<(), ValidationError> {
///         if head.status.is_success() {
///             Ok(())
///         } else {
///             Err(ValidationError::InvalidStatus(head.status.as_u16()))
///         }
///     }
/// }
/// ```
pub trait ResponseValidator: Send + Sync + 'static {
    fn validate(
        &self,
        request_key: RequestKey,
        head: Parts,
        body: Bytes,
    ) -> Result<(), ValidationError>;
}

#[derive(Debug)]
/// The default validator handler.
pub struct DefaultValidator;

impl ResponseValidator for DefaultValidator {
    fn validate(
        &self,
        _request_key: RequestKey,
        head: Parts,
        _body: Bytes,
    ) -> Result<(), ValidationError> {
        if head.status.is_success() {
            Ok(())
        } else {
            Err(ValidationError::InvalidStatus(
                head.status.as_u16(),
                head.headers,
            ))
        }
    }
}
