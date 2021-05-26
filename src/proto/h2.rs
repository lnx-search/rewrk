use crate::error::AnyError;
use crate::proto::tcp_stream;

use std::str::FromStr;
use std::time::Instant;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use hyper::{Body, Uri, StatusCode};
use hyper::client::conn;

use tower::{Service, ServiceExt};

use crate::results::WorkerResult;
use crate::utils::get_request;


/// A single http/1 connection worker
///
/// Builds a new http client with the http2_only option set either to false.
///
/// It then waits for the signaller to start sending pings to queue requests,
/// a client can take a request from the queue and then send the request,
/// these times are then measured and compared against previous latencies
/// to work out the min, max, total time and total requests of the given
/// worker which can then be sent back to the controller when the handle
/// is awaited.
///
/// todo Make concurrent handling for h2 tests
pub async fn client(
    time_for: Duration,
    uri_string: String,
    predicted_size: usize,
) -> Result<WorkerResult, AnyError> {
    let uri = Uri::from_str(&uri_string)?;

    let host = uri.host().ok_or("cant find host")?;
    let port = uri.port_u16().unwrap_or(80);

    let host_port = format!("{}:{}", host, port);

    let counter = Arc::new(AtomicUsize::new(0));

    let (disconnect_tx, mut disconnect_rx) = mpsc::channel(1);

    let start = Instant::now();

    let mut session = connect_with_retry(
        start,
        time_for,
        &host_port,
        counter.clone(),
        disconnect_tx.clone(),
    ).await?;

    let mut times: Vec<Duration> = Vec::with_capacity(predicted_size);

    while time_for > start.elapsed() {
        tokio::select!{
            val = send_request(&uri, &mut session, &mut times) => {
                if let Err(_e) = val {
                    // Errors are ignored currently.
                }
            },
            _ = disconnect_rx.recv() => {
                session = connect_with_retry(
                    start,
                    time_for,
                    &host_port,
                    counter.clone(),
                    disconnect_tx.clone(),
                ).await?;
            },
        };
    }

    let time_taken = start.elapsed();

    let result = WorkerResult{
        total_times: vec![time_taken],
        request_times: times,
        buffer_sizes: vec![counter.load(Ordering::Acquire)]
    };

    Ok(result)
}

// NOTE: Currently ignoring errors.
async fn send_request(
    uri: &Uri,
    session: &mut conn::SendRequest<Body>,
    times: &mut Vec<Duration>,
) -> Result<(), AnyError> {
    let req = get_request(&uri);

    let ts = Instant::now();

    match session.ready().await {
        Ok(_) => (),
        Err(_) => return Ok(()),
    }

    let resp = match session.call(req).await {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let took = ts.elapsed();

    let status = resp.status();
    assert_eq!(status, StatusCode::OK);

    let _buff = match hyper::body::to_bytes(resp).await {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    times.push(took);

    Ok(())
}

async fn connect_with_retry(
    start: Instant,
    time_for: Duration,
    host_port: &str,
    counter: Arc<AtomicUsize>,
    disconnect_tx: mpsc::Sender<()>,
) -> Result<conn::SendRequest<Body>, AnyError> {
    while start.elapsed() < time_for {
        let res = connect(
            host_port,
            counter.clone(),
            disconnect_tx.clone(),
        ).await;

        match res {
            Ok(session) => return Ok(session),
            Err(_) => (),
        }
    }

    Err("connection closed".into())
}

async fn connect(
    host_port: &str,
    counter: Arc<AtomicUsize>,
    disconnect_tx: mpsc::Sender<()>,
) -> Result<conn::SendRequest<Body>, AnyError> {
    let stream = tcp_stream::CustomTcpStream::new(
        TcpStream::connect(&host_port).await?,
        counter.clone()
    );

    let (session, connection) = conn::handshake(stream).await?;
    tokio::spawn(async move {
        if let Err(_) = connection.await {

        }

        // Connection died
        // Should reconnect and log
        if let Err(_) = disconnect_tx.send(()).await {

        }
    });

    Ok(session)
}


