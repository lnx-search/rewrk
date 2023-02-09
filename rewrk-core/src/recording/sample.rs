use std::time::{Duration, Instant};
use hdrhistogram::Histogram;


#[derive(Clone)]
pub struct SampleFactory {
    window_timeout: Duration,
}

impl SampleFactory {
    pub fn new_sample(&mut self) -> Sample {
        todo!()
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