use super::BenchType;
use http::{header::HeaderValue, uri::Uri};
use std::{
    convert::TryFrom,
    net::{SocketAddr, ToSocketAddrs},
};
use tokio::task::spawn_blocking;
use tokio_native_tls::TlsConnector;

type AnyError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug)]
pub(crate) enum Scheme {
    Http,
    Https(TlsConnector),
}

impl Scheme {
    fn default_port(&self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https(_) => 443,
        }
    }
}

#[derive(Clone)]
pub(crate) struct UserInput {
    pub(crate) addr: SocketAddr,
    pub(crate) scheme: Scheme,
    pub(crate) host: String,
    pub(crate) host_header: HeaderValue,
    pub(crate) uri: Uri,
}

impl UserInput {
    pub(crate) async fn new(protocol: BenchType, string: String) -> Result<Self, AnyError> {
        spawn_blocking(move || Self::blocking_new(protocol, string))
            .await
            .unwrap()
    }

    fn blocking_new(protocol: BenchType, string: String) -> Result<Self, AnyError> {
        let uri = Uri::try_from(string)?;
        let scheme = match uri.scheme().ok_or("scheme is not present on uri")?.as_str() {
            "http" => Scheme::Http,
            "https" => {
                let mut builder = native_tls::TlsConnector::builder();

                builder
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true);

                match protocol {
                    BenchType::HTTP1 => builder.request_alpns(&["http/1.1"]),
                    BenchType::HTTP2 => builder.request_alpns(&["h2"]),
                };

                let connector = TlsConnector::from(builder.build()?);
                Scheme::Https(connector)
            }
            _ => return Err("invalid scheme".into()),
        };
        let authority = uri.authority().ok_or("host not present on uri")?;
        let host = authority.host().to_owned();
        let port = authority
            .port_u16()
            .unwrap_or_else(|| scheme.default_port());
        let host_header = HeaderValue::from_str(&host)?;

        // Prefer ipv4.
        let addr_iter = (host.as_str(), port).to_socket_addrs()?;
        let mut last_addr = None;
        for addr in addr_iter {
            last_addr = Some(addr);
            if addr.is_ipv4() {
                break;
            }
        }
        let addr = last_addr.ok_or("hostname lookup failed")?;

        Ok(Self {
            addr,
            scheme,
            host,
            host_header,
            uri,
        })
    }
}
