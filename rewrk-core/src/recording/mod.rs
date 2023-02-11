mod sample;
mod collector;

pub use sample::{Sample, SampleFactory};
pub use collector::SampleCollector;
pub(crate) use collector::CollectorActor;