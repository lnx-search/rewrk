use std::fmt::{Debug, Formatter};
use std::ops::{Add, AddAssign};
use std::time::{Duration, Instant};

use flume::TrySendError;
use hdrhistogram::Histogram;

use crate::recording::collector::CollectorMailbox;
use crate::validator::ValidationError;

#[derive(Debug, Clone, Copy)]
pub struct SampleMetadata {
    /// The unique ID of the worker thread.
    pub worker_id: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("The service should shutdown.")]
/// The service worker has shutdown and should no longer process requests.
pub struct Shutdown;

#[derive(Clone)]
/// A sample factory produces and submits samples.
pub struct SampleFactory {
    /// The duration which should elapse before a sample
    /// is submitted to be processed.
    window_timeout: Duration,

    /// Metadata associated with the specific sample factory thread.
    metadata: SampleMetadata,
    submitter: CollectorMailbox,
}

impl SampleFactory {
    /// Create a new sample factory.
    pub fn new(
        window_timeout: Duration,
        metadata: SampleMetadata,
        submitter: CollectorMailbox,
    ) -> Self {
        Self {
            window_timeout,
            metadata,
            submitter,
        }
    }

    #[inline]
    /// Check if the handler should submit the current sample.
    pub fn should_submit(&self, instant: Instant) -> bool {
        self.window_timeout <= instant.elapsed()
    }

    #[inline]
    /// Create a new sample to record metrics.
    pub fn new_sample(&self, tag: usize) -> Sample {
        Sample {
            tag,
            total_duration: Default::default(),
            total_latency_duration: Default::default(),
            total_requests: 0,
            total_successful_requests: 0,
            latency_hist: Histogram::new_with_max(60000, 5).unwrap(),
            write_transfer_hist: Histogram::new_with_max(5 << 30, 5).unwrap(),
            read_transfer_hist: Histogram::new_with_max(5 << 30, 5).unwrap(),
            errors: Vec::with_capacity(4),
            metadata: self.metadata,
        }
    }

    #[inline]
    /// Attempts to submit a sample to the processor.
    pub fn submit_sample(&self, sample: Sample) -> Result<(), Shutdown> {
        debug!(sample = ?sample, "Submitting sample to processor");
        // This should never block as it's an unbounded channel.
        let result = self.submitter.try_send(sample);

        match result {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                panic!("Sample submitter should never be full.")
            },
            Err(TrySendError::Disconnected(_)) => Err(Shutdown),
        }
    }
}

#[derive(Clone)]
/// A collection of metrics taken from the benchmark for a given time window.
///
/// The sample contains the standard metrics (latency, IO, etc...) along with
/// any errors, the worker ID and sample tag which can be used to group results.
///
/// Internally this uses HDR Histograms which can generate the min, max, stdev and
/// varying percentile statistics of the benchmark.
pub struct Sample {
    tag: usize,
    total_duration: Duration,
    total_latency_duration: Duration,
    total_requests: usize,
    total_successful_requests: usize,

    latency_hist: Histogram<u32>,
    write_transfer_hist: Histogram<u32>,
    read_transfer_hist: Histogram<u32>,

    errors: Vec<ValidationError>,
    metadata: SampleMetadata,
}

impl Debug for Sample {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sample")
            .field("num_records", &self.total_successful_requests)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl Sample {
    /// The total number of requests within the sample including any errors.
    pub fn total_requests(&self) -> usize {
        self.total_requests
    }

    /// The total number of requests within the sample that passed validation
    /// and did not error.
    pub fn total_successful_requests(&self) -> usize {
        self.total_successful_requests
    }

    /// The total duration of requests within the sample.
    pub fn total_duration(&self) -> Duration {
        self.total_duration
    }

    /// The total duration of requests within the sample from the latency of
    /// each request.
    pub fn total_latency_duration(&self) -> Duration {
        self.total_latency_duration
    }

    /// The sample metadata.
    pub fn metadata(&self) -> SampleMetadata {
        self.metadata
    }

    /// The sample latency histogram
    pub fn latency(&self) -> &Histogram<u32> {
        &self.latency_hist
    }

    /// The sample write transfer rate histogram
    pub fn write_transfer(&self) -> &Histogram<u32> {
        &self.write_transfer_hist
    }

    /// The sample read transfer rate histogram
    pub fn read_transfer(&self) -> &Histogram<u32> {
        &self.read_transfer_hist
    }

    /// The errors that occurred during the sample
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    #[inline]
    /// The current sample batch tag.
    pub fn tag(&self) -> usize {
        self.tag
    }

    #[inline]
    /// Record a request validation error.
    pub(crate) fn record_successful_request(&mut self) {
        self.total_successful_requests += 1;
    }

    #[inline]
    /// Record a request validation error.
    pub(crate) fn record_total_request(&mut self) {
        self.total_requests += 1;
    }

    #[inline]
    /// Record a request validation error.
    pub(crate) fn set_total_duration(&mut self, dur: Duration) {
        self.total_duration = dur;
    }

    #[inline]
    /// Record a request validation error.
    pub(crate) fn record_error(&mut self, e: ValidationError) {
        self.errors.push(e);
    }

    #[inline]
    /// Record a latency duration.
    ///
    /// This value is converted to micro seconds.
    pub(crate) fn record_latency(&mut self, dur: Duration) {
        self.total_latency_duration += dur;
        let micros = dur.as_millis() as u64;
        self.latency_hist.record(micros).expect("Record value");
    }

    #[inline]
    /// Record a write transfer rate.
    pub(crate) fn record_write_transfer(
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
    pub(crate) fn record_read_transfer(
        &mut self,
        start_count: u64,
        end_count: u64,
        dur: Duration,
    ) {
        self.read_transfer_hist
            .record(calculate_rate(start_count, end_count, dur))
            .expect("Record value");
    }
}

impl Add for Sample {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl AddAssign for Sample {
    fn add_assign(&mut self, rhs: Self) {
        self.total_duration += rhs.total_duration;
        self.total_requests += rhs.total_requests;
        self.total_successful_requests += rhs.total_successful_requests;
        self.total_latency_duration += rhs.total_latency_duration;
        self.latency_hist += rhs.latency_hist;
        self.write_transfer_hist += rhs.write_transfer_hist;
        self.read_transfer_hist += rhs.read_transfer_hist;
        self.errors.extend(rhs.errors);
    }
}

#[inline]
fn calculate_rate(start: u64, stop: u64, dur: Duration) -> u64 {
    ((stop - start) as f64 / dur.as_secs_f64()).round() as u64
}
