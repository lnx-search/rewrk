use crate::error::AnyError;

use std::net::SocketAddr;
use std::str::FromStr;

use hyper::Uri;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy)]
pub enum Scheme {
    HTTP,
    HTTPS,
}

impl Scheme {
    pub fn default_port(&self) -> u16 {
        match self {
            Scheme::HTTP => 80,
            Scheme::HTTPS => 443,
        }
    }
}

impl From<&str> for Scheme {
    fn from(s: &str) -> Scheme {
        match s {
            "https" => Scheme::HTTPS,
            _ => Scheme::HTTP,
        }
    }
}

impl From<Option<&str>> for Scheme {
    fn from(s: Option<&str>) -> Scheme {
        match s {
            Some("https") => Scheme::HTTPS,
            _ => Scheme::HTTP,
        }
    }
}

pub struct ParsedUri {
    pub uri: Uri,
    pub scheme: Scheme,
    pub addr: SocketAddr,
}

impl ParsedUri {
    pub async fn parse_and_lookup(s: &str) -> Result<Self, AnyError> {
        let uri = Uri::from_str(s)?;

        let scheme = Scheme::from(uri.scheme_str());

        let host = uri.host().ok_or("cant find host")?;

        let port = match uri.port_u16() {
            Some(port) => port,
            None => scheme.default_port(),
        };

        let addr = get_preferred_ip(host, port).await?;

        Ok(ParsedUri { uri, scheme, addr })
    }
}

async fn get_preferred_ip(host: &str, port: u16) -> Result<SocketAddr, AnyError> {
    let addrs = tokio::net::lookup_host((host, port)).await?;

    let mut res = Err("host lookup failed".into());

    for addr in addrs {
        if addr.is_ipv4() {
            return Ok(addr);
        }

        if res.is_err() {
            res = Ok(addr);
        }
    }

    res
}
