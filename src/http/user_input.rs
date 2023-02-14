use std::convert::TryFrom;
use std::net::{SocketAddr, ToSocketAddrs};

use anyhow::{anyhow, Result};
use http::header::HeaderValue;
use http::uri::Uri;
use http::{HeaderMap, Method};
use hyper::body::Bytes;
use tokio::task::spawn_blocking;
use tokio_native_tls::TlsConnector;

use super::BenchType;

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
    pub(crate) method: Method,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Bytes,
}

impl UserInput {
    pub(crate) async fn new(
        protocol: BenchType,
        string: String,
        method: Method,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Self> {
        spawn_blocking(move || {
            Self::blocking_new(protocol, string, method, headers, body)
        })
        .await
        .unwrap()
    }

    fn blocking_new(
        protocol: BenchType,
        string: String,
        method: Method,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Self> {
        let uri = Uri::try_from(string)?;
        let scheme = uri
            .scheme()
            .ok_or_else(|| anyhow!("scheme is not present on uri"))?
            .as_str();
        let scheme = match scheme {
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
            },
            _ => return Err(anyhow::Error::msg("invalid scheme")),
        };
        let authority = uri
            .authority()
            .ok_or_else(|| anyhow!("host not present on uri"))?;
        let host = authority.host().to_owned();
        let port = authority
            .port_u16()
            .unwrap_or_else(|| scheme.default_port());
        let host_header = if port == 80 {
            HeaderValue::from_str(&host)?
        } else {
            HeaderValue::from_str(&format!("{}:{}", host, port))?
        };

        let uri = match protocol {
            BenchType::HTTP1 => match uri.path_and_query() {
                Some(pq) => Uri::try_from(pq.as_str())?,
                None => Uri::try_from(uri.path())?,
            },
            BenchType::HTTP2 => uri,
        };

        // Prefer ipv4.
        let addr_iter = (host.as_str(), port).to_socket_addrs()?;
        let mut last_addr = None;
        for addr in addr_iter {
            last_addr = Some(addr);
            if addr.is_ipv4() {
                break;
            }
        }
        let addr = last_addr.ok_or_else(|| anyhow!("hostname lookup failed"))?;

        Ok(Self {
            addr,
            scheme,
            host,
            host_header,
            uri,
            method,
            headers,
            body,
        })
    }
}
