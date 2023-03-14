use std::fmt::{Debug, Formatter};
use std::ops::{Add, AddAssign};
use std::time::{Duration, Instant};

use flume::TrySendError;

use crate::recording::collector::CollectorMailbox;
use crate::validator::ValidationError;

#[derive(Debug, Clone, Copy)]
pub struct SampleMetadata {
    /// The unique ID of the worker thread.
    pub worker_id: usize,
    /// The unique ID of the concurrent connection.
    pub concurrency_id: usize,
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
    pub fn worker_id(&self) -> usize {
        self.metadata.worker_id
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
            latency: Vec::with_capacity(5 << 10),
            write_transfer: Vec::with_capacity(5 << 10),
            read_transfer: Vec::with_capacity(5 << 10),
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

    latency: Vec<Duration>,
    write_transfer: Vec<u32>,
    read_transfer: Vec<u32>,

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
    /// Sorts the sample values from smallest to largest.
    pub(crate) fn sort_values(&mut self) {
        self.latency.sort();
        self.write_transfer.sort();
        self.read_transfer.sort();
    }

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

    /// The sample latency
    pub fn latency(&self) -> &[Duration] {
        &self.latency
    }

    /// The sample write transfer rate
    pub fn write_transfer(&self) -> &[u32] {
        &self.write_transfer
    }

    /// The sample read transfer rate
    pub fn read_transfer(&self) -> &[u32] {
        &self.read_transfer
    }

    /// Calculates the mean average from a given percentile set of values.
    pub fn latency_mean(&self) -> Duration {
        if self.latency().is_empty() {
            return Duration::default();
        }

        self.latency().iter().sum::<Duration>() / self.latency().len() as u32
    }

    /// Calculates the mean average from a given percentile set of values.
    pub fn write_transfer_mean(&self) -> f64 {
        self.write_transfer().iter().map(|v| *v as f64).sum::<f64>()
            / self.write_transfer().len() as f64
    }

    /// Calculates the mean average from a given percentile set of values.
    pub fn read_transfer_mean(&self) -> f64 {
        self.read_transfer().iter().map(|v| *v as f64).sum::<f64>()
            / self.read_transfer().len() as f64
    }

    /// Calculates the mean average from a given percentile set of values.
    pub fn latency_percentile_mean(&self, pct: f64) -> Duration {
        get_latency_percentile_mean(self.latency(), pct)
    }

    /// Calculates the mean average from a given percentile set of values.
    pub fn write_transfer_percentile_mean(&self, pct: f64) -> f64 {
        get_transfer_percentile_mean(self.write_transfer(), pct)
    }

    /// Calculates the mean average from a given percentile set of values.
    pub fn read_transfer_percentile_mean(&self, pct: f64) -> f64 {
        get_transfer_percentile_mean(self.read_transfer(), pct)
    }

    /// Calculates the nth percentile of values.
    pub fn latency_percentile(&self, pct: f64) -> Duration {
        get_percentile(self.latency(), pct)
    }

    /// Calculates the nth percentile of values.
    pub fn write_transfer_percentile(&self, pct: f64) -> u32 {
        get_percentile(self.write_transfer(), pct)
    }

    /// Calculates the nth percentile of values.
    pub fn read_transfer_percentile(&self, pct: f64) -> u32 {
        get_percentile(self.read_transfer(), pct)
    }

    /// Gets the max value within the set.
    pub fn latency_max(&self) -> Duration {
        // We assume our values are ordered smallest -> largest
        self.latency().last().copied().unwrap_or_default()
    }

    /// Gets the max value within the set.
    pub fn write_transfer_max(&self) -> u32 {
        // We assume our values are ordered smallest -> largest
        self.write_transfer().last().copied().unwrap_or_default()
    }

    /// Gets the max value within the set.
    pub fn read_transfer_max(&self) -> u32 {
        // We assume our values are ordered smallest -> largest
        self.read_transfer().last().copied().unwrap_or_default()
    }

    /// Gets the min value within the set.
    pub fn latency_min(&self) -> Duration {
        // We assume our values are ordered smallest -> largest
        self.latency().first().copied().unwrap_or_default()
    }

    /// Gets the min value within the set.
    pub fn write_transfer_min(&self) -> u32 {
        // We assume our values are ordered smallest -> largest
        self.write_transfer().first().copied().unwrap_or_default()
    }

    /// Gets the min value within the set.
    // We assume our values are ordered smallest -> largest
    pub fn read_transfer_min(&self) -> u32 {
        self.read_transfer().first().copied().unwrap_or_default()
    }

    /// Gets the min value within the set.
    pub fn latency_stdev(&self) -> Duration {
        if self.latency().is_empty() {
            return Duration::default();
        }

        let mean = self.latency_mean().as_secs_f64();
        let sum_delta: f64 = self
            .latency()
            .iter()
            .map(|dur| {
                let time = dur.as_secs_f64();
                let delta = time - mean;

                delta.powi(2)
            })
            .sum();

        let variance = sum_delta / self.latency().len() as f64;
        Duration::from_secs_f64(variance.powf(0.5))
    }

    /// Gets the min value within the set.
    pub fn write_transfer_stdev(&self) -> f64 {
        if self.write_transfer().is_empty() {
            return 0.0;
        }

        let mean = self.write_transfer_mean();
        let sum_delta: f64 = self
            .write_transfer()
            .iter()
            .map(|rate| {
                let time = *rate as f64;
                let delta = time - mean;

                delta.powi(2)
            })
            .sum();

        let variance = sum_delta / self.write_transfer().len() as f64;
        variance.powf(0.5)
    }

    /// Gets the min value within the set.
    pub fn read_transfer_stdev(&self) -> f64 {
        if self.read_transfer().is_empty() {
            return 0.0;
        }

        let mean = self.read_transfer_mean();
        let sum_delta: f64 = self
            .read_transfer()
            .iter()
            .map(|rate| {
                let time = *rate as f64;
                let delta = time - mean;

                delta.powi(2)
            })
            .sum();

        let variance = sum_delta / self.read_transfer().len() as f64;
        variance.powf(0.5)
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
        self.latency.push(dur);
    }

    #[inline]
    /// Record a write transfer rate.
    pub(crate) fn record_write_transfer(
        &mut self,
        start_count: u64,
        end_count: u64,
        dur: Duration,
    ) {
        self.write_transfer
            .push(calculate_rate(start_count, end_count, dur));
    }

    #[inline]
    /// Record a read transfer rate.
    pub(crate) fn record_read_transfer(
        &mut self,
        start_count: u64,
        end_count: u64,
        dur: Duration,
    ) {
        self.read_transfer
            .push(calculate_rate(start_count, end_count, dur));
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
        self.latency.extend(rhs.latency);
        self.read_transfer.extend(rhs.read_transfer);
        self.write_transfer.extend(rhs.write_transfer);
        self.errors.extend(rhs.errors);
    }
}

#[inline]
fn calculate_rate(start: u64, stop: u64, dur: Duration) -> u32 {
    ((stop - start) as f64 / dur.as_secs_f64()).round() as u32
}

/// Calculates the mean latency from a percentile of the response times.
fn get_latency_percentile_mean(samples: &[Duration], pct: f64) -> Duration {
    if samples.is_empty() {
        return Duration::default();
    }

    let mut len = samples.len() as f64 * pct;
    if len < 1.0 {
        len = 1.0;
    }

    let e = format!("failed to calculate P{} avg latency", pct * 100.0);
    let pct = samples.chunks(len as usize).next().expect(&e);

    let total: f64 = pct.iter().map(|dur| dur.as_secs_f64()).sum();

    let avg = total / pct.len() as f64;

    Duration::from_secs_f64(avg)
}

/// Calculates the mean latency from a percentile of the response times.
fn get_transfer_percentile_mean(samples: &[u32], pct: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let mut len = samples.len() as f64 * pct;
    if len < 1.0 {
        len = 1.0;
    }

    let e = format!("failed to calculate P{} avg", pct * 100.0);
    let pct = samples.chunks(len as usize).next().expect(&e);

    let total: u64 = pct.iter().map(|v| *v as u64).sum();

    total as f64 / pct.len() as f64
}

fn get_percentile<V: Copy + Default>(samples: &[V], pct: f64) -> V {
    if samples.is_empty() {
        return V::default();
    }

    let mut len = samples.len() as f64 * pct;
    if len < 1.0 {
        len = 1.0;
    }

    let e = format!("failed to calculate P{}", pct * 100.0);
    let pct = samples.chunks(len as usize).next().expect(&e);

    *pct.iter().last().unwrap()
}
