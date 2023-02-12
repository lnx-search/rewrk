use tokio_native_tls::TlsConnector;

mod conn;

pub use self::conn::{HttpStream, ReWrkConnection, ReWrkConnector};

/// The type of bench that is being ran.
#[derive(Clone, Copy, Debug)]
pub enum HttpProtocol {
    /// Sets the http protocol to be used as h1
    HTTP1,

    /// Sets the http protocol to be used as h2
    HTTP2,
}

impl HttpProtocol {
    pub fn is_http1(&self) -> bool {
        matches!(self, Self::HTTP1)
    }

    pub fn is_http2(&self) -> bool {
        matches!(self, Self::HTTP2)
    }
}

#[derive(Clone)]
/// The HTTP scheme used for the connection.
pub enum Scheme {
    Http,
    Https(TlsConnector),
}

impl Scheme {
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https(_) => 443,
        }
    }
}
