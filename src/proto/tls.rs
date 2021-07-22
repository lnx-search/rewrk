use crate::error::AnyError;

use std::sync::Arc;

use rustls::ClientConfig;
use tokio_rustls::TlsConnector;
use rustls_native_certs::load_native_certs;

pub fn connector_from_alpn(alpn: &[Vec<u8>]) -> Result<TlsConnector, AnyError> {
    let mut config = ClientConfig::new();

    config.root_store = load_native_certs().map_err(|_| "cant load native certificates")?;

    config.set_protocols(alpn);

    let connector = TlsConnector::from(Arc::new(config));

    Ok(connector)
}
