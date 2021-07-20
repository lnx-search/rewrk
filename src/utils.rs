use hyper::{Body, Request, Uri};

const GIGABYTE: f64 = (1024 * 1024 * 1024) as f64;
const MEGABYTE: f64 = (1024 * 1024) as f64;
const KILOBYTE: f64 = 1024 as f64;

/// Constructs a new Request of a given host.
pub fn get_http1_request(uri: &Uri) -> Request<Body> {
    let host = host_header(uri);

    Request::builder()
        .uri(uri.path())
        .header("Host", host)
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request")
}

pub fn get_http2_request(uri: &Uri) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request")
}

fn host_header(uri: &Uri) -> String {
    let invalid_uri = "Invalid URI";

    match uri.port_u16() {
        Some(port) => {
            format!("{}:{}", uri.host().expect(invalid_uri), port)
        }
        None => uri.host().expect(invalid_uri).to_owned(),
    }
}

/// Dirt simple div mod function.
pub fn div_mod(main: u64, divider: u64) -> (u64, u64) {
    let whole = main / divider;
    let rem = main % divider;

    (whole, rem)
}

pub fn format_data(data_size: f64) -> String {
    if data_size > GIGABYTE as f64 {
        format!("{:.2} GB", data_size / GIGABYTE)
    } else if data_size > MEGABYTE as f64 {
        format!("{:.2} MB", data_size / MEGABYTE)
    } else if data_size > KILOBYTE as f64 {
        format!("{:.2} KB", data_size / KILOBYTE)
    } else {
        format!("{:.2} B", data_size)
    }
}

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
