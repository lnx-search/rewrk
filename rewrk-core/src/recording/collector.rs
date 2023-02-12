use async_trait::async_trait;
use flume::Sender;
use tokio::task::JoinHandle;

use super::sample::Sample;

#[async_trait]
/// A collector for processing submitted samples.
pub trait SampleCollector: Send + 'static {
    async fn process_sample(&mut self, sample: Sample) -> anyhow::Result<()>;
}

pub type CollectorMailbox = Sender<Sample>;

/// A sample collector which waits for and calls the
/// specific collector handler.
pub struct CollectorActor<C>(pub(crate) JoinHandle<C>);

impl<C> CollectorActor<C>
where
    C: SampleCollector,
{
    /// Spawn a new collector actor for processing incoming samples.
    pub async fn spawn(mut collector: C) -> (Self, CollectorMailbox) {
        let (tx, rx) = flume::unbounded();

        let handle = tokio::spawn(async move {
            info!("Starting collector actor.");

            while let Ok(sample) = rx.recv_async().await {
                trace!(sample = ?sample, "Collector actor received processing sample.");
                if let Err(e) = collector.process_sample(sample).await {
                    warn!(error = ?e, "Collector failed to process sample due to error.");
                }
            }

            info!("Collector actor has shutdown.");
            collector
        });

        (Self(handle), tx)
    }
}
