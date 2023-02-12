use async_trait::async_trait;
use flume::Receiver;
use http::Request;
use hyper::Body;
use tokio::sync::oneshot;

/// A batch of requests or single to the workers.
pub enum RequestBatch {
    /// All requests have been produced and no more will be returned
    ///
    /// This will cause the workers to start shutting down.
    End,
    /// A new batch to process.
    Batch(Batch),
}

pub struct Batch {
    /// A optional tag ID for grouping results together.
    ///
    /// This is a `usize` for the sake of efficiency, this can
    /// be used as a mapping key for example.
    ///
    /// Samples are produced on a per-tag basis, so if a tag changes
    /// from the current sample vs the tag provided by the batch
    /// a new sample will be created.
    pub tag: usize,
    /// The batch requests.
    pub requests: Vec<Request<Body>>,
}

#[async_trait]
/// A producer creates requests used in benchmarking
pub trait Producer: Send + 'static {
    /// Signals to the producer that the system is ready and about to
    /// start benchmarking.
    fn ready(&mut self);

    /// Creates a new match of documents to be sent to workers.
    ///
    /// It's important to note that in order to accurately measure throughput
    /// the producer must be able to produce more requests than the target server
    /// can consume, otherwise the statistics may not be as accurate.
    async fn create_batch(&mut self) -> anyhow::Result<RequestBatch>;
}

pub type ProducerBatches = Receiver<Batch>;

/// A sample collector which waits for and calls the
/// specific collector handler.
pub struct ProducerActor;

impl ProducerActor {
    /// Spawn a new collector actor for processing incoming samples.
    pub async fn spawn(
        worker_id: usize,
        mut producer: impl Producer,
        ready: oneshot::Receiver<()>,
    ) -> ProducerBatches {
        let (tx, rx) = flume::bounded(10);

        tokio::spawn(async move {
            info!(worker_id = worker_id, "Starting producer actor.");

            let _ = ready.await;
            producer.ready();

            loop {
                let batch = match producer.create_batch().await {
                    Ok(RequestBatch::End) => break,
                    Ok(RequestBatch::Batch(batch)) => batch,
                    Err(e) => {
                        error!(
                            worker_id = worker_id,
                            error = ?e,
                            "Failed to produce batch due to error, aborting...",
                        );
                        break;
                    },
                };

                debug!(
                    worker_id = worker_id,
                    batch_tag = batch.tag,
                    "Submitting request batch."
                );
                if let Err(_) = tx.send_async(batch).await {
                    break;
                }
            }

            info!(worker_id = worker_id, "Producer actor has shutdown.");
        });

        rx
    }
}
