use std::time::{Duration, Instant};
use async_trait::async_trait;
use hdrhistogram::Histogram;
use flume::{Sender, TrySendError};

#[async_trait]
pub trait SampleSink {
    async fn process_sample(&mut self, sample: Sample) -> anyhow::Result<()>;
}


#[derive(Debug, thiserror::Error)]
#[error("The service should shutdown.")]
pub struct Shutdown;

#[derive(Clone)]
pub struct SampleFactory {
    window_timeout: Duration,

    sample_submitter: Sender<Sample>,
}

impl SampleFactory {
    pub fn new(
        window_timeout: Duration,
    ) -> Self {
        let (sample_submitter, rx) = flume::unbounded();

        Self {
            window_timeout,

            sample_submitter,
        }
    }

    pub fn new_sample(&self) -> Sample {
        todo!()
    }

    /// Attempts to submit a sample to the processor.
    pub fn submit_sample(&self, sample: Sample) -> Result<(), Shutdown> {
        let result = self.sample_submitter.try_send(sample);

        match result {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => panic!("Sample submitter should never be full."),
            Err(TrySendError::Disconnected(_)) => Err(Shutdown),
        }
    }
}


#[derive(Debug)]
pub struct Sample {
    latency_hist: Histogram<u32>,
    write_transfer_hist: Histogram<u32>,
    read_transfer_hist: Histogram<u32>,
}

impl Sample {
    #[inline]
    /// Record a latency duration.
    ///
    /// This value is converted to micro seconds.
    pub fn record_latency(&mut self, dur: Duration) {
        let micros = dur.as_micros() as u64;
        self.latency_hist
            .record(micros)
            .expect("Record value");
    }

    #[inline]
    /// Record a write transfer rate.
    pub fn record_write_transfer(
        &mut self,
        start_count: u64,
        end_count: u64,
        dur: Duration,
    ) {
        self.write_transfer_hist
            .record(calculate_rate(start_count, end_count, dur))
            .expect("Record value");
    }

    #[inline]
    /// Record a read transfer rate.
    pub fn record_read_transfer(
        &mut self,
        start_count: u64,
        end_count: u64,
        dur: Duration,
    ) {
        self.write_transfer_hist
            .record(calculate_rate(start_count, end_count, dur))
            .expect("Record value");
    }
}


#[inline]
fn calculate_rate(start: u64, stop: u64, dur: Duration) -> u64 {
    (stop - start) / dur.as_secs()
}