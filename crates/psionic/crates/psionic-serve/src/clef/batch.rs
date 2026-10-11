//! Continuous text prefill with isolated state and shared FFN projections.

use super::chunk_schedule::{ChunkScheduler, Completion, Limits, RequestId};
use super::worker::Worker;
use super::*;
use crate::qwen35::clef_metal::batching::MetalSession;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, atomic::AtomicBool, mpsc};

#[derive(Clone, Copy, Debug)]
pub(super) struct Config {
    requests: usize,
    tokens: usize,
    request_bytes: u64,
    cohort_bytes: u64,
}

impl Config {
    pub fn from_env() -> Result<Option<Self>, String> {
        Self::parse(|name| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned()))
    }

    fn parse(mut get: impl FnMut(&str) -> Option<String>) -> Result<Option<Self>, String> {
        fn setting(
            get: &mut impl FnMut(&str) -> Option<String>,
            name: &str,
            default: u64,
            max: u64,
        ) -> Result<u64, String> {
            let value = get(name).map_or(Ok(default), |value| {
                value
                    .parse::<u64>()
                    .map_err(|_| format!("{name} must be a positive integer"))
            })?;
            if value == 0 || value > max {
                return Err(format!("{name} must be between 1 and {max}"));
            }
            Ok(value)
        }
        let requests = setting(&mut get, "PSIONIC_CLEF_BATCH_REQUESTS", 1, 4)? as usize;
        if requests == 1 {
            return Ok(None);
        }
        if get("PSIONIC_CLEF_PROFILE").is_some() || get("PSIONIC_CLEF_SKIP").is_some() {
            return Err(String::from(
                "disable Clef profiling and skip settings before batching",
            ));
        }
        // A 2,048-token cohort fits both Flash and 27B in the default 1 GiB
        // scratch budget, including the optional Q4 conversion and Delta buffers.
        let tokens = setting(&mut get, "PSIONIC_CLEF_BATCH_TOKENS", 2048, 16384)? as usize;
        if tokens < requests {
            return Err(String::from(
                "Clef batch tokens must cover every batch request",
            ));
        }
        Ok(Some(Self {
            requests,
            tokens,
            request_bytes: setting(&mut get, "PSIONIC_CLEF_BATCH_BYTES", 4 << 30, 32 << 30)?,
            cohort_bytes: setting(
                &mut get,
                "PSIONIC_CLEF_BATCH_COHORT_BYTES",
                1 << 30,
                4 << 30,
            )?,
        }))
    }

    fn check_cohort_bytes(&self, bytes: u64) -> Result<(), String> {
        if bytes > self.cohort_bytes {
            return Err(format!(
                "Clef batch scratch needs {bytes} bytes; configured limit is {}",
                self.cohort_bytes
            ));
        }
        Ok(())
    }
}

pub(super) struct BatchResult {
    pub result: LogitResult,
    pub queue_seconds: f64,
    pub prefill_chunk: usize,
    pub chunks: usize,
    pub max_cohort: usize,
}

struct Job {
    record: EncodedRecord,
    chunk: usize,
    submitted: Instant,
    cancelled: Arc<AtomicBool>,
    reply: mpsc::Sender<Result<BatchResult, ClefRefusal>>,
}

impl Job {
    fn fail(self, error: impl Into<String>) {
        let _ = self.reply.send(Err(ClefRefusal::internal(error)));
    }
}

struct Session {
    job: Job,
    metal: Option<MetalSession>,
    prefill_seconds: f64,
    chunks: usize,
    max_cohort: usize,
}

struct Ticket {
    reply: mpsc::Receiver<Result<BatchResult, ClefRefusal>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

pub(super) struct Dispatcher {
    worker: Worker<Job>,
}

impl Dispatcher {
    pub fn new(
        cpu: CpuGgufQwen35TextGenerationService,
        metal: crate::ClefMetalTrunk,
        head: Arc<ClefHeadWeights>,
        running: Arc<Mutex<()>>,
        serial_waiters: Arc<AtomicUsize>,
        config: Config,
        max_queue: usize,
    ) -> Result<Self, String> {
        config.check_cohort_bytes(metal.cohort_bytes(config.tokens)?)?;
        let worker = Worker::spawn(max_queue.max(1), move |receiver, stop| {
            execute(
                cpu,
                metal,
                head,
                running,
                serial_waiters,
                config,
                max_queue.max(1),
                receiver,
                stop,
            );
        })
        .map_err(|error| format!("cannot start Clef batch worker: {error}"))?;
        Ok(Self { worker })
    }

    pub fn run(
        &self,
        record: EncodedRecord,
        chunk: usize,
        cancelled: Arc<AtomicBool>,
    ) -> Result<BatchResult, ClefRefusal> {
        let (reply, receiver) = mpsc::channel();
        let ticket = Ticket {
            reply: receiver,
            cancelled: Arc::clone(&cancelled),
        };
        let job = Job {
            record,
            chunk,
            submitted: Instant::now(),
            cancelled,
            reply,
        };
        self.worker.submit(job).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => ClefRefusal {
                status: 503,
                code: "busy",
                message: String::from("too many decisions are waiting"),
            },
            mpsc::TrySendError::Disconnected(_) => {
                ClefRefusal::internal("the decision worker stopped")
            }
        })?;
        ticket
            .reply
            .recv()
            .map_err(|_| ClefRefusal::internal("the decision worker stopped"))?
    }
}

#[allow(clippy::too_many_arguments)]
fn execute(
    cpu: CpuGgufQwen35TextGenerationService,
    metal: crate::ClefMetalTrunk,
    head: Arc<ClefHeadWeights>,
    running: Arc<Mutex<()>>,
    serial_waiters: Arc<AtomicUsize>,
    config: Config,
    max_queue: usize,
    receiver: mpsc::Receiver<Job>,
    stop: Arc<AtomicBool>,
) {
    let scheduler = ChunkScheduler::new(Limits {
        max_requests: config.requests,
        max_batch_requests: config.requests,
        max_batch_tokens: config.tokens,
        max_reserved_bytes: config.request_bytes,
    })
    .expect("validated batch limits");
    let trunk = DeviceTrunk::Metal(metal.clone());
    let mut pending = VecDeque::<Job>::new();
    let mut active = BTreeMap::<RequestId, Arc<AtomicBool>>::new();
    let mut prefix_owner = None;
    let mut execution_guard = None;
    while !stop.load(Ordering::Acquire) {
        if active.is_empty() {
            execution_guard.take();
            if serial_waiters.load(Ordering::Acquire) > 0 {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
        }
        if pending.is_empty() && active.is_empty() {
            match receiver.recv() {
                Ok(job) => pending.push_back(job),
                Err(_) => break,
            }
        }
        while pending.len() < max_queue {
            match receiver.try_recv() {
                Ok(job) => pending.push_back(job),
                Err(_) => break,
            }
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        // Serial diagnostics and media use this same lock. They must not
        // allocate a singleton request alongside the active set's reservations.
        // Arrivals still join the worker between chunks through its queue.
        execution_guard.get_or_insert_with(|| {
            running
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        let cancelled: Vec<_> = active
            .iter()
            .filter_map(|(&id, cancelled)| cancelled.load(Ordering::Acquire).then_some(id))
            .collect();
        for id in cancelled {
            drop(scheduler.cancel(id));
            active.remove(&id);
            if prefix_owner == Some(id) {
                prefix_owner = None;
            }
        }
        metal.reap_pending_prefix();
        while let Some(job) = pending.front() {
            if serial_waiters.load(Ordering::Acquire) > 0 {
                break;
            }
            if job.cancelled.load(Ordering::Acquire) {
                pending.pop_front();
                continue;
            }
            // Leave room for all admitted requests in the next cohort.
            let chunk = job.chunk.min(config.tokens / config.requests).max(1);
            let estimate = metal.session_bytes(
                job.record.input_ids.len(),
                chunk,
                head_spans(&job.record).len(),
                head.config.heads,
            );
            let bytes = match estimate {
                Ok(bytes) if bytes <= config.request_bytes => bytes,
                Ok(_) => {
                    let job = pending.pop_front().expect("front job");
                    let _ = job.reply.send(Err(ClefRefusal::not_admitted(
                        413,
                        "this request exceeds the configured batch memory limit",
                    )));
                    continue;
                }
                Err(error) => {
                    pending.pop_front().expect("front job").fail(error);
                    continue;
                }
            };
            let (count, reserved) = scheduler.reserved();
            if count == config.requests || bytes > config.request_bytes - reserved {
                break;
            }
            let job = pending.pop_front().expect("front job");
            let began = Instant::now();
            let session = metal.begin_session(
                job.record.input_ids.iter().map(|&id| TokenId(id)).collect(),
                head_spans(&job.record),
                chunk,
                job.record.prefix_tokens,
            );
            let session = match session {
                Ok(session) => session,
                Err(error) => {
                    job.fail(error);
                    continue;
                }
            };
            if session.device_bytes() > bytes {
                job.fail("batch memory estimate is smaller than its allocation");
                continue;
            }
            let tokens = session.remaining_tokens();
            let boundaries = session.boundaries();
            let chunk = session.chunk_tokens();
            let cancelled = Arc::clone(&job.cancelled);
            let state = Session {
                job,
                metal: Some(session),
                prefill_seconds: began.elapsed().as_secs_f64(),
                chunks: 0,
                max_cohort: 0,
            };
            match scheduler.admit_with_boundaries(state, tokens, chunk, bytes, &boundaries) {
                Ok(id) => {
                    active.insert(id, cancelled);
                }
                Err((_, state)) => state.job.fail("batch reservation failed"),
            }
        }
        let mut chunks = scheduler.next_batch();
        if chunks.is_empty() {
            execution_guard.take();
            continue;
        }
        let ids: Vec<_> = chunks.iter().map(|chunk| chunk.id()).collect();
        let size = chunks.len();
        let began = Instant::now();
        let outcome = {
            let mut sessions: Vec<_> = chunks
                .iter_mut()
                .map(|chunk| {
                    let count = chunk.range().len();
                    (
                        chunk.state_mut().metal.as_mut().expect("active session"),
                        count,
                    )
                })
                .collect();
            metal.advance_sessions(&cpu, &mut sessions, prefix_owner.is_none())
        };
        let elapsed = began.elapsed().as_secs_f64();
        match outcome {
            Ok(Some(index)) => prefix_owner = Some(ids[index]),
            Ok(None) => {}
            Err(error) => {
                for mut chunk in chunks {
                    let id = chunk.id();
                    let _ = chunk
                        .state_mut()
                        .job
                        .reply
                        .send(Err(ClefRefusal::internal(error.clone())));
                    active.remove(&id);
                    if prefix_owner == Some(id) {
                        prefix_owner = None;
                    }
                }
                metal.reap_pending_prefix();
                continue;
            }
        }
        for mut chunk in chunks {
            let id = chunk.id();
            let state = chunk.state_mut();
            state.prefill_seconds += elapsed;
            state.chunks += 1;
            state.max_cohort = state.max_cohort.max(size);
            if stop.load(Ordering::Acquire) || state.job.cancelled.load(Ordering::Acquire) {
                scheduler.cancel(id);
            }
            match chunk.complete() {
                Completion::Ready => {}
                Completion::Cancelled(_) => {
                    active.remove(&id);
                    if prefix_owner == Some(id) {
                        prefix_owner = None;
                    }
                }
                Completion::Finished(mut completed) => {
                    let result = finish(&cpu, &metal, &trunk, &head, completed.state_mut());
                    let _ = completed.state_mut().job.reply.send(result);
                    active.remove(&id);
                    if prefix_owner == Some(id) {
                        prefix_owner = None;
                    }
                }
            }
        }
        metal.reap_pending_prefix();
        if active.is_empty() {
            execution_guard.take();
        }
    }
    // Drop all request buffers and senders before Worker::drop joins this thread.
    for id in active.keys() {
        drop(scheduler.cancel(*id));
    }
    metal.reap_pending_prefix();
}

fn finish(
    cpu: &CpuGgufQwen35TextGenerationService,
    metal: &crate::ClefMetalTrunk,
    trunk: &DeviceTrunk,
    head: &ClefHeadWeights,
    state: &mut Session,
) -> Result<BatchResult, ClefRefusal> {
    let began = Instant::now();
    let session = state.metal.take().expect("completed session");
    let prefill_chunk = session.chunk_tokens();
    let (prefill, cached_input_tokens, _active) = metal
        .activate_session(session)
        .map_err(ClefRefusal::internal)?;
    let record = &state.job.record;
    let width = head.config.hidden_size;
    let means = head_spans(record)
        .iter()
        .zip(prefill.span_sums.chunks(width))
        .map(|((start, end), sum)| {
            sum.iter()
                .map(|value| value / end.saturating_sub(*start).max(1) as f32)
                .collect()
        })
        .collect::<Vec<Vec<f32>>>();
    let lexical = |ids: &[u32]| {
        cpu.output_embedding_rows(ids)
            .map_err(|error| error.to_string())
    };
    let mut memory = DeviceMemory::new(trunk);
    let logits = run_head(head, record, &means, &prefill.last, &mut memory, &lexical)
        .map_err(ClefRefusal::internal)?;
    let head_seconds = began.elapsed().as_secs_f64();
    Ok(BatchResult {
        queue_seconds: (state.job.submitted.elapsed().as_secs_f64()
            - state.prefill_seconds
            - head_seconds)
            .max(0.0),
        prefill_chunk,
        chunks: state.chunks,
        max_cohort: state.max_cohort,
        result: LogitResult {
            logits,
            cached_input_tokens,
            prefill_seconds: state.prefill_seconds,
            head_seconds,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_response_reports_session_chunk_instead_of_configured_chunk() {
        // Two/four-request cohorts reduce the default 2,048-token chunk;
        // a shorter prompt reduces the session's allocation further.
        for (requests, effective_chunk) in [(2, 1024), (4, 512), (2, 37)] {
            let mut answer = json!({
                "psionic": { "prefill_chunk": 2048, "backend": "metal" },
                "usage": { "input_tokens": 37, "cached_input_tokens": 0 }
            });
            set_batch_metadata(&mut answer, 3, requests, effective_chunk);
            assert_eq!(answer["psionic"]["prefill_chunk"], effective_chunk);
            assert_eq!(answer["psionic"]["batch"]["chunks"], 3);
            assert_eq!(answer["psionic"]["batch"]["max_requests"], requests);
            assert_eq!(answer["psionic"]["batch"]["grouped_ffn"], true);
            assert_eq!(answer["psionic"]["backend"], "metal");
            assert_eq!(answer["usage"]["input_tokens"], 37);
        }
    }

    #[test]
    fn batching_is_off_by_default_and_does_not_apply_batch_limits_to_serial_work() {
        assert!(Config::parse(|_| None).unwrap().is_none());
        assert!(
            Config::parse(|name| match name {
                "PSIONIC_CLEF_BATCH_REQUESTS" => Some(String::from("1")),
                "PSIONIC_CLEF_BATCH_BYTES" => Some(String::from("invalid")),
                "PSIONIC_CLEF_PROFILE" => Some(String::from("1")),
                _ => None,
            })
            .unwrap()
            .is_none()
        );
        let enabled = Config::parse(|name| {
            (name == "PSIONIC_CLEF_BATCH_REQUESTS").then(|| String::from("2"))
        })
        .unwrap()
        .unwrap();
        assert_eq!(enabled.requests, 2);
        assert_eq!(enabled.tokens, 2048);
        assert!(
            Config::parse(|name| match name {
                "PSIONIC_CLEF_BATCH_REQUESTS" => Some(String::from("2")),
                "PSIONIC_CLEF_BATCH_TOKENS" => Some(String::from("1")),
                _ => None,
            })
            .is_err()
        );
    }

    #[test]
    fn enabling_batching_fits_real_flash_and_27b_cohort_budgets() {
        use psionic_backend_metal::clef_delta::ClefMetalDelta;

        for requests in [2, 4] {
            let config = Config::parse(|name| {
                (name == "PSIONIC_CLEF_BATCH_REQUESTS").then(|| requests.to_string())
            })
            .unwrap()
            .unwrap();
            for large in [false, true] {
                for delta in [ClefMetalDelta::Sequential, ClefMetalDelta::BlockTensorOps] {
                    for low_memory in [false, true] {
                        let bytes = crate::ClefMetalTrunk::scratch_estimate_for_test(
                            large,
                            config.tokens,
                            delta,
                            low_memory,
                        )
                        .unwrap();
                        assert!(config.check_cohort_bytes(bytes).is_ok());
                        let too_small = Config {
                            cohort_bytes: bytes - 1,
                            ..config
                        };
                        assert!(too_small.check_cohort_bytes(bytes).is_err());
                    }
                }
            }
        }

        // Raising only the token count still requires an explicit memory budget.
        let mut larger = Config::parse(|name| match name {
            "PSIONIC_CLEF_BATCH_REQUESTS" => Some(String::from("2")),
            "PSIONIC_CLEF_BATCH_TOKENS" => Some(String::from("4096")),
            _ => None,
        })
        .unwrap()
        .unwrap();
        let bytes = crate::ClefMetalTrunk::scratch_estimate_for_test(
            true,
            larger.tokens,
            ClefMetalDelta::BlockTensorOps,
            true,
        )
        .unwrap();
        assert!(larger.check_cohort_bytes(bytes).is_err());
        larger.cohort_bytes = 2 << 30;
        assert!(larger.check_cohort_bytes(bytes).is_ok());
    }

    #[test]
    fn dropping_ticket_cancels_without_releasing_running_state() {
        let (_, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let ticket = Ticket {
            reply: receiver,
            cancelled: Arc::clone(&cancelled),
        };
        assert!(!cancelled.load(Ordering::Acquire));
        drop(ticket);
        assert!(cancelled.load(Ordering::Acquire));
    }
}
