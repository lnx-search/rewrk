use crate::error::AnyError;

use std::convert::TryFrom;
use std::str::FromStr;

use hyper::Uri;

#[derive(Clone, Copy)]
pub enum Scheme {
    HTTP,
    HTTPS
}

impl Scheme {
    pub fn default_port(&self) -> u16 {
        match self {
            Scheme::HTTP => 80,
            Scheme::HTTPS => 443
        }
    }
}

impl From<&str> for Scheme {
    fn from(s: &str) -> Scheme {
        match s {
            "https" => Scheme::HTTPS,
            _ => Scheme::HTTP
        }
    }
}

impl From<Option<&str>> for Scheme {
    fn from(s: Option<&str>) -> Scheme {
        match s {
            Some("https") => Scheme::HTTPS,
            _ => Scheme::HTTP
        }
    }
}

pub struct ParsedUri {
    pub uri: Uri,
    pub scheme: Scheme,
    pub host: String,
    pub port: u16
}

impl TryFrom<&str> for ParsedUri {
    type Error = AnyError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let uri = Uri::from_str(&s)?;

        let scheme = Scheme::from(uri.scheme_str());

        let host = uri.host().ok_or("cant find host")?.to_owned();

        let port = match uri.port_u16() {
            Some(port) => port,
            None => scheme.default_port()
        };

        Ok(ParsedUri {
            uri,
            scheme,
            host,
            port
        })
    }
}

impl TryFrom<String> for ParsedUri {
    type Error = AnyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}

impl TryFrom<&String> for ParsedUri {
    type Error = AnyError;

    fn try_from(s: &String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
