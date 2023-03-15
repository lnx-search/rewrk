use std::collections::BTreeMap;

use crate::Sample;

type SampleKey = (usize, usize, usize);

#[derive(Default, Debug)]
/// A smart sample merger.
///
/// This ensures samples are merged correctly forming correct totals
/// taking into account concurrency and tags.
pub struct SampleMerger {
    entries: BTreeMap<SampleKey, Sample>,
}

impl SampleMerger {
    /// Add a new sample to the merger.
    pub fn add_sample(&mut self, sample: Sample) {
        let key = (
            sample.tag(),
            sample.metadata().worker_id,
            sample.metadata().concurrency_id,
        );

        self.entries
            .entry(key)
            .and_modify(|sample| (*sample) += sample.clone())
            .or_insert(sample);
    }

    /// Iterate over the merged samples.
    pub fn iter_samples(&mut self) -> impl Iterator<Item = MergedSample> + '_ {
        self.entries
            .iter()
            .map(|((tag, worker_id, concurrency_id), sample)| MergedSample {
                tag: *tag,
                worker_id: *worker_id,
                concurrency_id: *concurrency_id,
                sample: sample.clone(),
            })
    }
}

pub struct MergedSample {
    /// The tag that the sample is associated with.
    pub tag: usize,
    /// The worker ID.
    pub worker_id: usize,
    /// The concurrency ID.
    pub concurrency_id: usize,
    /// The total sample result.
    pub sample: Sample,
}
