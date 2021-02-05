use async_channel::Receiver;

use std::time::Instant;

use rand::random;

use tokio::time::{Duration, sleep};

use hyper::StatusCode;
use hyper::Client;

use crate::results::WorkerResult;
use crate::utils::get_request;


/// A multi connection worker.
///
/// This tries to simulate the random state of clients connecting and
/// disconnected to the server rather than connecting and just flooding
/// requests down the socket.
///
/// NOTE:
///     This is still fairly experimental so im by no means finished
///     tinkering with this.
///
/// It then waits for the signaller to start sending pings to queue requests,
/// a client can take a request from the queue and then send the request,
/// these times are then measured and compared against previous latencies
/// to work out the min, max, total time and total requests of the given
/// worker which can then be sent back to the controller when the handle
/// is awaited.
pub async fn client(
    waiter: Receiver<()>,
    host: String,
    predicted_size: usize,
) -> Result<WorkerResult, String> {
    // Should we care if its http/1 or http/2 if
    // its supposed to be real clients?
    let mut session = Client::new();

    let mut times: Vec<Duration> = Vec::with_capacity(predicted_size);
    let mut buffer_counter: usize = 0;

    let start = Instant::now();
    while let Ok(_) = waiter.recv().await {
        // Its time to get funky
        if random::<u8>() <= 60u8 {
            session = Client::new();
            sleep(Duration::from_secs_f64(0.5)).await;
        }

        let req = get_request(&host);

        let ts = Instant::now();
        let re = session.request(req).await;
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


