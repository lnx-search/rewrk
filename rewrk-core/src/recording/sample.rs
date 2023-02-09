use std::time::Duration;
use hdrhistogram::Histogram;


#[derive(Clone)]
pub struct SampleFactory {

}

impl SampleFactory {
    pub fn new_sample(&self) -> Sample {
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
    /// Record a latency duration.
    ///
    /// This value is converted to micro seconds.
    pub fn record_latency(&mut self, dur: Duration) {
        let micros = dur.as_micros() as u64;
        self.latency_hist
            .record(micros)
            .expect("Record value");
    }

    /// Record a write transfer rate.
    pub fn record_write_transfer(&mut self, bytes: u64) {
        self.write_transfer_hist
            .record(bytes)
            .expect("Record value");
    }

    /// Record a read transfer rate.
    pub fn record_read_transfer(&mut self, bytes: u64) {
        self.write_transfer_hist
            .record(bytes)
            .expect("Record value");
    }
}