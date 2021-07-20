use crate::error::AnyError;

use std::sync::Arc;

use rustls::ClientConfig;
use tokio_rustls::TlsConnector;
use rustls_native_certs::load_native_certs;

pub fn http1_alpn_connector() -> Result<TlsConnector, AnyError> {
    let mut config = ClientConfig::new();

    config.root_store = load_native_certs().map_err(|_| "cant load native certificates")?;

    config.set_protocols(&[b"http/1.1".to_vec()]);

    let connector = TlsConnector::from(Arc::new(config));

    Ok(connector)
}

pub fn http2_alpn_connector() -> Result<TlsConnector, AnyError> {
    let mut config = ClientConfig::new();

    config.root_store = load_native_certs().map_err(|_| "cant load native certificates")?;

    config.set_protocols(&[b"h2".to_vec(), b"http/1.1".to_vec()]);

    let connector = TlsConnector::from(Arc::new(config));

    Ok(connector)
}
