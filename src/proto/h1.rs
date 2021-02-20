use async_channel::Receiver;

use std::str::FromStr;
use std::time::Instant;

use tokio::time::Duration;
use tokio::net::TcpStream;

use hyper::Uri;
use hyper::StatusCode;
use hyper::client::conn;

use crate::results::WorkerResult;
use crate::utils::get_request_new;

/// A macro that converts Error to String
macro_rules! conv_err {
    ( $e:expr ) => ( $e.map_err(|e| format!("{}", e)) )
}

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
pub async fn client(
    waiter: Receiver<()>,
    uri_string: String,
    predicted_size: usize,
) -> Result<WorkerResult, String> {
    let uri = conv_err!( Uri::from_str(&uri_string) )?;

    let host = uri.host().ok_or("cant find host")?;
    let port = uri.port_u16().unwrap_or(80);
    
    let host_port = format!("{}:{}", host, port);

    let stream = conv_err!( TcpStream::connect(&host_port).await )?;

    let (mut session, connection) = conv_err!( conn::handshake(stream).await )?;
    tokio::spawn(async move {
        if let Err(_) = connection.await {
        
        }

        // Connection died
        // Should reconnect and log
    });

    let mut times: Vec<Duration> = Vec::with_capacity(predicted_size);
    let mut buffer_counter: usize = 0;

    let start = Instant::now();
    while let Ok(_) = waiter.recv().await {
        let req = get_request_new(&uri);

        let ts = Instant::now();
        let re = session.send_request(req).await;
        let took = ts.elapsed();

        if let Err(e) = &re {
            return Err(format!("{:?}", e));
        } else if let Ok(r) = re {
            let status = r.status();
            assert_eq!(status, StatusCode::OK);

            let buff = match hyper::body::to_bytes(r).await {
                Ok(buff) => buff,
                Err(e) => return Err(format!(
                    "Failed to read stream {:?}",
                     e
                ))
            };
            buffer_counter += buff.len();
        }

        times.push(took);

    }
    let time_taken = start.elapsed();

    let result = WorkerResult{
        total_times: vec![time_taken],
        request_times: times,
        buffer_sizes: vec![buffer_counter]
    };

    Ok(result)
}


