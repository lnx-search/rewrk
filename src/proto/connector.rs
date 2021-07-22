use crate::error::AnyError;
use crate::utils::BoxedFuture;
use crate::proto::tls;
use crate::proto::protocol::HttpProtocol;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::JoinHandle;

use tokio_rustls::TlsConnector;
use tokio_rustls::webpki::{DNSName, DNSNameRef};

use hyper::Body;
use hyper::client::conn;

pub struct Connection {
    pub send_request: conn::SendRequest<Body>,
    pub handle: JoinHandle<()>
}

pub trait Connect {
    fn handshake<S, P>(&self, stream: S, protocol: P) -> BoxedFuture<Result<Connection, AnyError>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        P: HttpProtocol + Send + Sync + 'static;
}

pub struct HttpConnector;

impl HttpConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Connect for HttpConnector {
    fn handshake<S, P>(&self, stream: S, protocol: P) -> BoxedFuture<Result<Connection, AnyError>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        P: HttpProtocol + Send + Sync + 'static
    {
        Box::pin(handshake(stream, protocol))
    }
}

pub struct HttpsConnector {
    domain: DNSName,
    tls_connector: TlsConnector
}

impl HttpsConnector {
    pub fn new(domain: &str, alpn: &[Vec<u8>]) -> Result<Self, AnyError> {
        let domain = DNSNameRef::try_from_ascii_str(domain)?.to_owned();
        let tls_connector = tls::connector_from_alpn(alpn)?;

        Ok(Self {
            domain,
            tls_connector
        })
    }
}

impl Connect for HttpsConnector {
    fn handshake<S, P>(&self, stream: S, protocol: P) -> BoxedFuture<Result<Connection, AnyError>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        P: HttpProtocol + Send + Sync + 'static
    {
        Box::pin(async move {
            let stream = self.tls_connector.connect(self.domain.as_ref(), stream).await?;

            Ok(handshake(stream, protocol).await?)
        })
    }
}

async fn handshake<S, P>(stream: S, protocol: P) -> Result<Connection, AnyError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    P: HttpProtocol + Send + Sync + 'static
{
    let (send_request, connection) = conn::Builder::new()
        .http2_only(protocol.is_http2())
        .handshake(stream).await?;

    let handle = tokio::spawn(async move {
        if let Err(_) = connection.await {}

        // Connection died
        // Should reconnect and log
    });

    Ok(Connection {
        send_request,
        handle
    })
}
