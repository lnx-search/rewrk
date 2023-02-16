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
///
/// # Example
///
/// This basic collector simply appends each sample into a vector which
/// can then be consumed after the benchmark using `benchmarker.consume_collector().await`
///
/// ```
/// use rewrk_core::{Sample, SampleCollector};
///
/// #[derive(Default)]
/// pub struct BasicCollector {
///     samples: Vec<Sample>,
/// }
///
/// #[rewrk_core::async_trait]
/// impl SampleCollector for BasicCollector {
///     async fn process_sample(&mut self, sample: Sample) -> anyhow::Result<()> {
///         self.samples.push(sample);
///         Ok(())
///     }
/// }
/// ```
pub struct CollectorActor<C>(pub(crate) JoinHandle<C>);

impl<C> CollectorActor<C>
where
    C: SampleCollector,
{
    /// Spawn a new collector actor for processing incoming samples.
    pub async fn spawn(mut collector: C) -> (Self, CollectorMailbox) {
        let (tx, rx) = flume::unbounded::<Sample>();

        let handle = tokio::spawn(async move {
            info!("Starting collector actor");

            while let Ok(mut sample) = rx.recv_async().await {
                sample.sort_values();

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
