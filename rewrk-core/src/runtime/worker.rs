use std::borrow::Cow;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use http::Request;
use hyper::Body;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::connection::{ReWrkConnection, ReWrkConnector};
use crate::producer::{Batch, Producer, ProducerActor, ProducerBatches};
use crate::recording::{CollectorMailbox, SampleFactory, SampleMetadata};
use crate::utils::RuntimeTimings;
use crate::validator::ValidationError;
use crate::{RequestKey, ResponseValidator, Sample};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
type ConnectionTask = JoinHandle<RuntimeTimings>;
type WorkerGuard = flume::Receiver<()>;

#[derive(Clone)]
pub(crate) struct WorkerConfig<P>
where
    P: Producer + Clone,
{
    /// The benchmarking connector.
    pub connector: ReWrkConnector,
    /// The selected validator for the benchmark.
    pub validator: Arc<dyn ResponseValidator>,
    /// The sample results collector.
    pub collector: CollectorMailbox,
    /// The request batch producer.
    pub producer: P,
    /// The duration which should elapse before a sample
    /// is submitted to be processed.
    pub sample_window: Duration,
    /// The percentage threshold that the system must be
    /// waiting on the producer in order for a warning to be raised.
    ///
    /// This is useful in situations where you know the producer will
    /// take more time than normal and want to silence the warning.
    pub producer_wait_warning_threshold: f32,
}

impl<P> WorkerConfig<P>
where
    P: Producer + Clone,
{
    fn clone_for_worker(&mut self, worker_id: usize) -> Self {
        Self {
            connector: self.connector.clone(),
            validator: self.validator.clone(),
            collector: self.collector.clone(),
            producer: self.producer.for_worker(worker_id),
            sample_window: self.sample_window,
            producer_wait_warning_threshold: self.producer_wait_warning_threshold,
        }
    }
}

/// Spawns N worker runtimes for executing search requests.
pub(crate) fn spawn_workers<P>(
    shutdown: ShutdownHandle,
    num_workers: usize,
    concurrency: usize,
    mut config: WorkerConfig<P>,
) -> WorkerGuard
where
    P: Producer + Clone,
{
    // We use a channel here as a guard in order to wait for all workers to shutdown.
    let (guard, waiter) = flume::bounded(1);

    let per_worker_concurrency = concurrency / num_workers;
    let mut remaining_concurrency = concurrency - (per_worker_concurrency * num_workers);

    for worker_id in 0..num_workers {
        let concurrency_modifier = if remaining_concurrency != 0 {
            remaining_concurrency -= 1;
            1
        } else {
            0
        };
        let concurrency = per_worker_concurrency + concurrency_modifier;

        spawn_worker(
            worker_id,
            concurrency,
            guard.clone(),
            shutdown.clone(),
            config.clone_for_worker(worker_id),
        );
    }

    waiter
}

/// Spawns a new runtime worker thread.
fn spawn_worker<P>(
    worker_id: usize,
    concurrency: usize,
    guard: flume::Sender<()>,
    handle: ShutdownHandle,
    config: WorkerConfig<P>,
) where
    P: Producer + Clone,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Create runtime");

    std::thread::Builder::new()
        .name(format!("rewrk-worker-{worker_id}"))
        .spawn(move || {
            debug!(worker_id = worker_id, "Spawning worker");
            rt.block_on(run_worker(worker_id, concurrency, handle, config));

            // Drop the guard explicitly to make sure it's not dropped
            // until after the runtime has completed.
            drop(guard);

            debug!(worker_id = worker_id, "Worker successfully shutdown");
        })
        .expect("Spawn thread");
}

/// Runs a worker task.
///
/// This acts as the main runtime entrypoint.
async fn run_worker<P>(
    worker_id: usize,
    concurrency: usize,
    shutdown: ShutdownHandle,
    config: WorkerConfig<P>,
) where
    P: Producer + Clone,
{
    let (ready_tx, ready_rx) = oneshot::channel();
    let producer =
        ProducerActor::spawn(concurrency * 4, worker_id, config.producer, ready_rx)
            .await;
    let metadata = SampleMetadata { worker_id };
    let sample_factory =
        SampleFactory::new(config.sample_window, metadata, config.collector);

    let mut pending_futures = Vec::<ConnectionTask>::with_capacity(concurrency);
    for _ in 0..concurrency {
        let task_opt = create_worker_connection(
            worker_id,
            &config.connector,
            shutdown.clone(),
            sample_factory.clone(),
            config.validator.clone(),
            producer.clone(),
        )
        .await;

        match task_opt {
            None => {
                info!(worker_id = ?worker_id, "Cleaning up futures and shutting down...");
                for pending in pending_futures {
                    pending.abort();
                }
                return;
            },
            Some(task) => {
                pending_futures.push(task);
            },
        }
    }

    // Begin benchmarking.
    let _ = ready_tx.send(());

    // Wait for all tasks to complete.
    let timings = join_all(pending_futures)
        .await
        .into_iter()
        .collect::<Result<RuntimeTimings, _>>()
        .expect("Join tasks");

    info!(worker_id = worker_id, "Benchmark completed for worker.");

    let total_duration = timings.execute_wait_runtime + timings.producer_wait_runtime;
    let producer_wait_pct = (timings.producer_wait_runtime.as_secs_f32()
        / total_duration.as_secs_f32())
        * 100.0;

    if producer_wait_pct >= config.producer_wait_warning_threshold {
        warn!(
            worker_id = worker_id,
            producer_wait_pct = producer_wait_pct,
            request_execute_wait_duration = ?timings.execute_wait_runtime,
            producer_wait_duration = ?timings.producer_wait_runtime,
            total_runtime_duration = ?total_duration,
            "The system spent {producer_wait_pct:.2}% of it's runtime waiting for the producer.\
             Results may not be accurate."
        );
    }
}

async fn create_worker_connection(
    worker_id: usize,
    connector: &ReWrkConnector,
    shutdown: ShutdownHandle,
    sample_factory: SampleFactory,
    validator: Arc<dyn ResponseValidator>,
    producer: ProducerBatches,
) -> Option<ConnectionTask> {
    let connect_result = connector.connect_timeout(CONNECT_TIMEOUT).await;
    let conn = match connect_result {
        Err(e) => {
            // We check this to prevent spam of the logs.
            if !shutdown.should_abort() {
                error!(worker_id = worker_id, error = ?e, "Failed to connect to server due to error, aborting.");
                shutdown.set_abort();
            }
            return None;
        },
        Ok(None) => {
            // We check this to prevent spam of the logs.
            if !shutdown.should_abort() {
                error!(worker_id = worker_id, "Worker failed to connect to server within {CONNECT_TIMEOUT:?}, aborting.");
                shutdown.set_abort();
            }
            return None;
        },
        Ok(Some(conn)) => conn,
    };

    let mut connection = WorkerConnection::new(
        conn,
        sample_factory,
        validator,
        producer,
        shutdown.clone(),
    );

    let fut = async move {
        while !shutdown.should_abort() {
            let can_continue = connection.execute_next_batch().await;

            if !can_continue {
                break;
            }
        }

        // Submit the remaining sample.
        connection.submit_sample(0);

        connection.timings
    };

    Some(tokio::spawn(fut))
}

#[derive(Default, Clone)]
pub struct ShutdownHandle {
    /// A signal flag telling all workers to shutdown.
    should_stop: Arc<AtomicBool>,
}

impl ShutdownHandle {
    /// Checks if the worker should abort processing.
    pub fn should_abort(&self) -> bool {
        self.should_stop.load(Ordering::Relaxed)
    }

    /// Sets the abort flag across workers.
    pub fn set_abort(&self) {
        self.should_stop.store(true, Ordering::Relaxed);
    }
}

pub struct WorkerConnection {
    /// The ReWrk benchmarking connection.
    conn: ReWrkConnection,
    /// The sample factory for producing metric samples.
    sample_factory: SampleFactory,
    /// The current sample being populated with metrics.
    sample: Sample,
    /// The selected validator for the benchmark.
    validator: Arc<dyn ResponseValidator>,
    /// The request batch producer.
    producer: ProducerBatches,
    /// The point in time when the last sample was submitted to
    /// the collectors.
    last_sent_sample: Instant,
    /// A signal flag telling all workers to shutdown.
    shutdown: ShutdownHandle,
    /// Internal timings which are useful for debugging.
    timings: RuntimeTimings,
    /// A check for if the first batch has been received already.
    ///
    /// This is so that timings can be adjusted while waiting for
    /// benchmarking to start, which would otherwise skew results.
    is_first_batch: bool,
}

impl WorkerConnection {
    /// Create a new worker instance
    fn new(
        conn: ReWrkConnection,
        sample_factory: SampleFactory,
        validator: Arc<dyn ResponseValidator>,
        producer: ProducerBatches,
        shutdown: ShutdownHandle,
    ) -> Self {
        let sample = sample_factory.new_sample(0);
        let last_sent_sample = Instant::now();

        Self {
            conn,
            sample_factory,
            sample,
            validator,
            producer,
            last_sent_sample,
            shutdown,
            timings: RuntimeTimings::default(),
            is_first_batch: true,
        }
    }

    /// Sets the abort flag across workers.
    fn set_abort(&self) {
        self.shutdown.set_abort()
    }

    /// Submit the current sample to the collectors and create a new
    /// sample with a given tag.
    fn submit_sample(&mut self, next_sample_tag: usize) -> bool {
        let new_sample = self.sample_factory.new_sample(next_sample_tag);
        let mut old_sample = mem::replace(&mut self.sample, new_sample);
        old_sample.set_total_duration(self.last_sent_sample.elapsed());
        if self.sample_factory.submit_sample(old_sample).is_err() {
            return false;
        }
        self.last_sent_sample = Instant::now();
        true
    }

    /// Gets the next batch from the producer and submits it to be executed.
    ///
    /// The method returns if more batches are possibly available.
    async fn execute_next_batch(&mut self) -> bool {
        let producer_start = Instant::now();
        let (request_key, batch) = match self.producer.recv_async().await {
            Ok(batch) => batch,
            // We've completed all batches.
            Err(_) => return false,
        };
        let producer_elapsed = producer_start.elapsed();

        if self.is_first_batch {
            self.is_first_batch = false;
            self.last_sent_sample = Instant::now();
        } else {
            self.timings.producer_wait_runtime += producer_elapsed;
        }

        let execute_start = Instant::now();
        self.execute_batch(request_key, batch).await;
        self.timings.execute_wait_runtime += execute_start.elapsed();

        true
    }

    /// Executes a batch of requests to measure the metrics.
    async fn execute_batch(&mut self, request_key: RequestKey, batch: Batch) {
        if self.sample.tag() != batch.tag {
            let success = self.submit_sample(batch.tag);

            if !success {
                self.set_abort();
                return;
            }
        }

        for (n, request) in batch.requests.into_iter().enumerate() {
            let key =
                RequestKey::new(request_key.worker_id(), request_key.request_id() + n);
            let result = self.send(key, request).await;

            match result {
                Ok(should_continue) if !should_continue => {
                    self.set_abort();
                    return;
                },
                Err(e) => {
                    error!(error = ?e, "Worker encountered an error while benchmarking, aborting...");
                    self.set_abort();
                    return;
                },
                _ => {},
            }
        }
    }

    /// Send a HTTP request and record the relevant metrics
    async fn send(
        &mut self,
        key: RequestKey,
        request: Request<Body>,
    ) -> Result<bool, hyper::Error> {
        self.sample.record_total_request();

        let read_transfer_start = self.conn.usage().get_received_count();
        let write_transfer_start = self.conn.usage().get_written_count();
        let start = Instant::now();

        let (head, body) = match self.conn.execute_req(request).await {
            Ok(resp) => resp,
            Err(e) => {
                if e.is_body_write_aborted() || e.is_closed() || e.is_connect() {
                    self.sample.record_error(ValidationError::ConnectionAborted);
                    return Ok(false);
                } else if e.is_incomplete_message()
                    || e.is_parse()
                    || e.is_parse_too_large()
                    || e.is_parse_status()
                {
                    self.sample.record_error(ValidationError::InvalidBody(
                        Cow::Borrowed("invalid-http-body"),
                    ));
                } else if e.is_timeout() {
                    self.sample.record_error(ValidationError::Timeout);
                } else {
                    return Err(e);
                }

                return Ok(true);
            },
        };

        let elapsed_time = start.elapsed();
        let read_transfer_end = self.conn.usage().get_received_count();
        let write_transfer_end = self.conn.usage().get_written_count();

        if let Err(e) = self.validator.validate(key, head, body.clone()) {
            #[cfg(feature = "log-body-errors")]
            debug!(body = ?body, "Failed to validate request body.");

            self.sample.record_error(e);
        } else {
            self.sample.record_successful_request();
            self.sample.record_latency(elapsed_time);
            self.sample.record_read_transfer(
                read_transfer_start,
                read_transfer_end,
                elapsed_time,
            );
            self.sample.record_write_transfer(
                write_transfer_start,
                write_transfer_end,
                elapsed_time,
            );
        }

        // Submit the sample if it's window interval has elapsed.
        if self.sample_factory.should_submit(self.last_sent_sample) {
            let batch_tag = self.sample.tag();
            let success = self.submit_sample(batch_tag);
            return Ok(success);
        }

        Ok(true)
    }
}
