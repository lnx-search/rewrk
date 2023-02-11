use async_trait::async_trait;
use flume::Sender;

use super::sample::Sample;

#[async_trait]
/// A collector for processing submitted samples.
pub trait SampleCollector: Send + 'static {
    async fn process_sample(&mut self, sample: Sample) -> anyhow::Result<()>;
}


pub type CollectorMailbox = Sender<Sample>;


/// A sample collector which waits for and calls the 
/// specific collector handler.
pub struct CollectorActor {
    collector: Box<dyn SampleCollector>,
}

impl CollectorActor {
    /// Spawn a new collector actor for processing incoming samples.
    pub async fn spawn(collector: impl SampleCollector) -> CollectorMailbox {
        let (tx, rx) = flume::unbounded();
        let mut actor = Self {
            collector: Box::new(collector),
        };
        
        tokio::spawn(async move {
            info!("Starting collector actor.");
            
            while let Ok(sample) = rx.recv_async().await {
                trace!(sample = ?sample, "Collector actor received processing sample.");
                if let Err(e) = actor.collector.process_sample(sample).await {
                    warn!(error = ?e, "Collector failed to process sample due to error.");
                } 
            }
            
            info!("Collector actor has shutdown.");
        });
        
        tx
    }
}

