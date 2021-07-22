use hyper::{Request, Body, Uri};
use http::request;

pub trait HttpProtocol {
    fn is_http2(&self) -> bool;

    fn request_builder(&self, uri: &Uri) -> request::Builder;

    fn get_request(&self, uri: &Uri) -> Request<Body> {
        self
            .request_builder(uri)
            .body(Body::empty())
            .expect("bad uri")
    }

    fn alpn_protocols(&self) -> Vec<Vec<u8>>;
}

#[derive(Clone, Copy)]
pub struct Http;

impl HttpProtocol for Http {
    fn is_http2(&self) -> bool {
        false
    }

    fn request_builder(&self, uri: &Uri) -> request::Builder {
        let host = host_header(uri);

        Request::builder()
            .uri(uri.path())
            .header("Host", host)
    }

    fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        vec![b"http/1.1".to_vec()]
    }
}

#[derive(Clone, Copy)]
pub struct Http2;

impl HttpProtocol for Http2 {
    fn is_http2(&self) -> bool {
        true
    }

    fn request_builder(&self, uri: &Uri) -> request::Builder {
        Request::builder()
            .uri(uri)
    }

    fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        vec![b"h2".to_vec()]
    }
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
