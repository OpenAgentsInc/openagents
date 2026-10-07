use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable phase binding for one local train metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalTrainMetricPhase {
    /// One optimizer-step training metric.
    Train,
    /// One validation-side metric.
    Validation,
    /// One checkpoint-side local metric.
    Checkpoint,
    /// One run summary metric.
    Summary,
}

impl LocalTrainMetricPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
            Self::Checkpoint => "checkpoint",
            Self::Summary => "summary",
        }
    }
}

/// One typed numeric metric payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LocalTrainMetricValue {
    /// One `f32` scalar.
    F32(f32),
    /// One `f64` scalar.
    F64(f64),
    /// One `u64` count.
    U64(u64),
}

impl LocalTrainMetricValue {
    fn validate(&self, metric_id: &str) -> Result<(), LocalTrainMetricSinkError> {
        match self {
            Self::F32(value) => {
                if !value.is_finite() {
                    return Err(LocalTrainMetricSinkError::InvalidMetricValue {
                        metric_id: metric_id.to_string(),
                        detail: String::from("f32 metric values must be finite"),
                    });
                }
            }
            Self::F64(value) => {
                if !value.is_finite() {
                    return Err(LocalTrainMetricSinkError::InvalidMetricValue {
                        metric_id: metric_id.to_string(),
                        detail: String::from("f64 metric values must be finite"),
                    });
                }
            }
            Self::U64(_) => {}
        }
        Ok(())
    }

    #[must_use]
    pub fn display_string(&self) -> String {
        match self {
            Self::F32(value) => value.to_string(),
            Self::F64(value) => value.to_string(),
            Self::U64(value) => value.to_string(),
        }
    }
}

/// One typed local train metric event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalTrainMetricEvent {
    /// Stable run binding for the event.
    pub run_id: String,
    /// Stable phase binding for the event.
    pub phase: LocalTrainMetricPhase,
    /// Stable logical step binding.
    pub step: u64,
    /// Stable metric identifier.
    pub metric_id: String,
    /// Typed metric payload.
    pub value: LocalTrainMetricValue,
    /// Optional human-readable detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl LocalTrainMetricEvent {
    /// Creates one typed local train metric event.
    #[must_use]
    pub fn new(
        run_id: impl Into<String>,
        phase: LocalTrainMetricPhase,
        step: u64,
        metric_id: impl Into<String>,
        value: LocalTrainMetricValue,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            phase,
            step,
            metric_id: metric_id.into(),
            value,
            detail: None,
        }
    }

    /// Attaches one optional detail string.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    fn validate(&self) -> Result<(), LocalTrainMetricSinkError> {
        if self.run_id.trim().is_empty() {
            return Err(LocalTrainMetricSinkError::MissingRunId);
        }
        if self.metric_id.trim().is_empty() {
            return Err(LocalTrainMetricSinkError::MissingMetricId);
        }
        self.value.validate(self.metric_id.as_str())
    }
}

/// Failure surfaced while recording local train-loop telemetry.
#[derive(Debug, Error)]
pub enum LocalTrainMetricSinkError {
    /// One event was missing a run binding.
    #[error("local train metric events require a non-empty run id")]
    MissingRunId,
    /// One event was missing its metric identifier.
    #[error("local train metric events require a non-empty metric id")]
    MissingMetricId,
    /// One fanout session received an event for the wrong run.
    #[error("local train metric sink for run `{expected_run_id}` received metric for run `{actual_run_id}`")]
    RunBindingMismatch {
        /// Expected fanout run id.
        expected_run_id: String,
        /// Actual event run id.
        actual_run_id: String,
    },
    /// One event attempted to move backward in step order for the same phase.
    #[error(
        "local train metric `{metric_id}` moved backward in phase `{phase}`: previous step {previous_step}, current step {current_step}"
    )]
    NonMonotonicStep {
        /// Stable phase string.
        phase: String,
        /// Stable metric identifier.
        metric_id: String,
        /// Previous step observed in the phase.
        previous_step: u64,
        /// Current step that regressed.
        current_step: u64,
    },
    /// One event carried an invalid numeric payload.
    #[error("local train metric `{metric_id}` is invalid: {detail}")]
    InvalidMetricValue {
        /// Stable metric identifier.
        metric_id: String,
        /// Plain-language validation detail.
        detail: String,
    },
    /// Writing one sink output failed.
    #[error("local train metric sink could not write `{path}`: {error}")]
    Write {
        /// Logical output path or label.
        path: String,
        /// Underlying IO failure.
        error: std::io::Error,
    },
    /// Encoding one metric line failed.
    #[error("local train metric sink could not encode `{context}`: {error}")]
    Encode {
        /// Logical encoding context.
        context: &'static str,
        /// Underlying serialization failure.
        error: serde_json::Error,
    },
}

/// One consumer for local train metric events.
pub trait LocalTrainMetricConsumer {
    /// Records one validated local train metric event.
    fn record(&mut self, event: &LocalTrainMetricEvent) -> Result<(), LocalTrainMetricSinkError>;

    /// Flushes the consumer deterministically.
    fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError>;
}

/// One run-scoped fanout surface for local train metric consumers.
pub struct LocalTrainMetricFanout {
    run_id: String,
    last_step_by_phase: BTreeMap<LocalTrainMetricPhase, u64>,
    consumers: Vec<Box<dyn LocalTrainMetricConsumer>>,
}

impl LocalTrainMetricFanout {
    /// Creates one run-scoped metric fanout.
    #[must_use]
    pub fn new(run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            last_step_by_phase: BTreeMap::new(),
            consumers: Vec::new(),
        }
    }

    /// Adds one metric consumer to the fanout.
    pub fn add_sink<S>(&mut self, sink: S)
    where
        S: LocalTrainMetricConsumer + 'static,
    {
        self.consumers.push(Box::new(sink));
    }

    /// Records one typed metric event across all configured consumers.
    pub fn record(
        &mut self,
        event: LocalTrainMetricEvent,
    ) -> Result<(), LocalTrainMetricSinkError> {
        event.validate()?;
        if event.run_id != self.run_id {
            return Err(LocalTrainMetricSinkError::RunBindingMismatch {
                expected_run_id: self.run_id.clone(),
                actual_run_id: event.run_id,
            });
        }
        if let Some(previous_step) = self.last_step_by_phase.get(&event.phase) {
            if event.step < *previous_step {
                return Err(LocalTrainMetricSinkError::NonMonotonicStep {
                    phase: event.phase.as_str().to_string(),
                    metric_id: event.metric_id.clone(),
                    previous_step: *previous_step,
                    current_step: event.step,
                });
            }
        }
        self.last_step_by_phase.insert(event.phase, event.step);
        for consumer in &mut self.consumers {
            consumer.record(&event)?;
        }
        Ok(())
    }

    /// Flushes all configured consumers in insertion order.
    pub fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError> {
        for consumer in &mut self.consumers {
            consumer.flush()?;
        }
        Ok(())
    }
}

/// One human-readable progress sink.
pub struct LocalTrainMetricProgressSink<W> {
    writer: W,
}

impl<W> LocalTrainMetricProgressSink<W> {
    /// Creates one progress sink around an arbitrary writer.
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W> LocalTrainMetricConsumer for LocalTrainMetricProgressSink<W>
where
    W: Write,
{
    fn record(&mut self, event: &LocalTrainMetricEvent) -> Result<(), LocalTrainMetricSinkError> {
        writeln!(
            self.writer,
            "{} {} step={} {}={}",
            event.run_id,
            event.phase.as_str(),
            event.step,
            event.metric_id,
            event.value.display_string()
        )
        .map_err(|error| LocalTrainMetricSinkError::Write {
            path: String::from("progress-output"),
            error,
        })
    }

    fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError> {
        self.writer
            .flush()
            .map_err(|error| LocalTrainMetricSinkError::Write {
                path: String::from("progress-output"),
                error,
            })
    }
}

/// One structured-log sink that emits stable JSON-backed log lines.
pub struct LocalTrainMetricStructuredLogSink<W> {
    writer: W,
}

impl<W> LocalTrainMetricStructuredLogSink<W> {
    /// Creates one structured-log sink around an arbitrary writer.
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W> LocalTrainMetricConsumer for LocalTrainMetricStructuredLogSink<W>
where
    W: Write,
{
    fn record(&mut self, event: &LocalTrainMetricEvent) -> Result<(), LocalTrainMetricSinkError> {
        let encoded =
            serde_json::to_vec(event).map_err(|error| LocalTrainMetricSinkError::Encode {
                context: "structured_log_metric_event",
                error,
            })?;
        self.writer
            .write_all(b"metric_event ")
            .and_then(|_| self.writer.write_all(encoded.as_slice()))
            .and_then(|_| self.writer.write_all(b"\n"))
            .map_err(|error| LocalTrainMetricSinkError::Write {
                path: String::from("structured-log-output"),
                error,
            })
    }

    fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError> {
        self.writer
            .flush()
            .map_err(|error| LocalTrainMetricSinkError::Write {
                path: String::from("structured-log-output"),
                error,
            })
    }
}

/// One JSONL sink for deterministic local metric files.
pub struct LocalTrainMetricJsonlSink<W> {
    writer: W,
}

impl<W> LocalTrainMetricJsonlSink<W> {
    /// Creates one JSONL sink around an arbitrary writer.
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl LocalTrainMetricJsonlSink<BufWriter<File>> {
    /// Creates one JSONL sink that truncates and rewrites the target file.
    pub fn create(path: &Path) -> Result<Self, LocalTrainMetricSinkError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| LocalTrainMetricSinkError::Write {
                path: parent.display().to_string(),
                error,
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .map_err(|error| LocalTrainMetricSinkError::Write {
                path: path.display().to_string(),
                error,
            })?;
        Ok(Self::new(BufWriter::new(file)))
    }
}

impl<W> LocalTrainMetricConsumer for LocalTrainMetricJsonlSink<W>
where
    W: Write,
{
    fn record(&mut self, event: &LocalTrainMetricEvent) -> Result<(), LocalTrainMetricSinkError> {
        serde_json::to_writer(&mut self.writer, event).map_err(|error| {
            LocalTrainMetricSinkError::Encode {
                context: "jsonl_metric_event",
                error,
            }
        })?;
        self.writer
            .write_all(b"\n")
            .map_err(|error| LocalTrainMetricSinkError::Write {
                path: String::from("jsonl-output"),
                error,
            })
    }

    fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError> {
        self.writer
            .flush()
            .map_err(|error| LocalTrainMetricSinkError::Write {
                path: String::from("jsonl-output"),
                error,
            })
    }
}

/// One cloneable in-memory collector for pre-aggregation inputs.
#[derive(Clone, Default)]
pub struct LocalTrainMetricCollector {
    events: Arc<Mutex<Vec<LocalTrainMetricEvent>>>,
}

impl LocalTrainMetricCollector {
    /// Returns the collected events in insertion order.
    #[must_use]
    pub fn events(&self) -> Vec<LocalTrainMetricEvent> {
        self.events
            .lock()
            .expect("metric collector mutex should not be poisoned")
            .clone()
    }
}

impl LocalTrainMetricConsumer for LocalTrainMetricCollector {
    fn record(&mut self, event: &LocalTrainMetricEvent) -> Result<(), LocalTrainMetricSinkError> {
        self.events
            .lock()
            .expect("metric collector mutex should not be poisoned")
            .push(event.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
    };

    use super::{
        LocalTrainMetricCollector, LocalTrainMetricConsumer, LocalTrainMetricEvent,
        LocalTrainMetricFanout, LocalTrainMetricJsonlSink, LocalTrainMetricPhase,
        LocalTrainMetricProgressSink, LocalTrainMetricSinkError, LocalTrainMetricStructuredLogSink,
        LocalTrainMetricValue,
    };

    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl SharedWriter {
        fn contents(&self) -> String {
            String::from_utf8(
                self.0
                    .lock()
                    .expect("shared writer mutex should not be poisoned")
                    .clone(),
            )
            .expect("shared writer should only contain utf8")
        }
    }

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("shared writer mutex should not be poisoned")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct TraceSink {
        name: &'static str,
        trace: Arc<Mutex<Vec<String>>>,
    }

    impl LocalTrainMetricConsumer for TraceSink {
        fn record(
            &mut self,
            event: &LocalTrainMetricEvent,
        ) -> Result<(), LocalTrainMetricSinkError> {
            self.trace
                .lock()
                .expect("trace sink mutex should not be poisoned")
                .push(format!(
                    "record:{}:{}:{}",
                    self.name, event.metric_id, event.step
                ));
            Ok(())
        }

        fn flush(&mut self) -> Result<(), LocalTrainMetricSinkError> {
            self.trace
                .lock()
                .expect("trace sink mutex should not be poisoned")
                .push(format!("flush:{}", self.name));
            Ok(())
        }
    }

    fn event(step: u64, metric_id: &str, value: LocalTrainMetricValue) -> LocalTrainMetricEvent {
        LocalTrainMetricEvent::new(
            "run-local-metrics",
            LocalTrainMetricPhase::Train,
            step,
            metric_id,
            value,
        )
    }

    #[test]
    fn fanout_records_and_flushes_in_insertion_order() -> Result<(), Box<dyn std::error::Error>> {
        let trace = Arc::new(Mutex::new(Vec::<String>::new()));
        let mut fanout = LocalTrainMetricFanout::new("run-local-metrics");
        fanout.add_sink(TraceSink {
            name: "first",
            trace: trace.clone(),
        });
        fanout.add_sink(TraceSink {
            name: "second",
            trace: trace.clone(),
        });

        fanout.record(event(
            1,
            "mean_microbatch_loss",
            LocalTrainMetricValue::F32(1.25),
        ))?;
        fanout.flush()?;

        assert_eq!(
            trace
                .lock()
                .expect("trace sink mutex should not be poisoned")
                .clone(),
            vec![
                String::from("record:first:mean_microbatch_loss:1"),
                String::from("record:second:mean_microbatch_loss:1"),
                String::from("flush:first"),
                String::from("flush:second"),
            ]
        );
        Ok(())
    }

    #[test]
    fn jsonl_sink_emits_deterministic_metric_lines() -> Result<(), Box<dyn std::error::Error>> {
        let shared = SharedWriter::default();
        let mut sink = LocalTrainMetricJsonlSink::new(shared.clone());
        let first = event(1, "mean_microbatch_loss", LocalTrainMetricValue::F32(1.25));
        let second = LocalTrainMetricEvent::new(
            "run-local-metrics",
            LocalTrainMetricPhase::Validation,
            1,
            "validation_mean_loss",
            LocalTrainMetricValue::F64(1.5),
        );
        sink.record(&first)?;
        sink.record(&second)?;
        sink.flush()?;

        let lines = shared
            .contents()
            .lines()
            .map(serde_json::from_str::<LocalTrainMetricEvent>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines, vec![first, second]);
        Ok(())
    }

    #[test]
    fn fanout_rejects_invalid_metric_schema() {
        let mut fanout = LocalTrainMetricFanout::new("run-local-metrics");
        let error = fanout
            .record(LocalTrainMetricEvent::new(
                "run-local-metrics",
                LocalTrainMetricPhase::Train,
                1,
                "mean_microbatch_loss",
                LocalTrainMetricValue::F32(f32::NAN),
            ))
            .expect_err("nan metric values should be refused");
        assert!(matches!(
            error,
            LocalTrainMetricSinkError::InvalidMetricValue { .. }
        ));
    }

    #[test]
    fn fanout_rejects_non_monotonic_step_bindings() {
        let mut fanout = LocalTrainMetricFanout::new("run-local-metrics");
        fanout
            .record(event(
                2,
                "mean_microbatch_loss",
                LocalTrainMetricValue::F32(1.25),
            ))
            .expect("first metric should be accepted");
        let error = fanout
            .record(event(
                1,
                "mean_microbatch_loss",
                LocalTrainMetricValue::F32(1.0),
            ))
            .expect_err("step regressions should be refused");
        assert!(matches!(
            error,
            LocalTrainMetricSinkError::NonMonotonicStep { .. }
        ));
    }

    #[test]
    fn collector_preserves_events_for_pre_aggregation() -> Result<(), Box<dyn std::error::Error>> {
        let collector = LocalTrainMetricCollector::default();
        let mut fanout = LocalTrainMetricFanout::new("run-local-metrics");
        fanout.add_sink(collector.clone());
        let first = event(1, "mean_microbatch_loss", LocalTrainMetricValue::F32(1.25));
        let second = event(1, "tokens_per_second", LocalTrainMetricValue::U64(2048));
        fanout.record(first.clone())?;
        fanout.record(second.clone())?;

        assert_eq!(collector.events(), vec![first, second]);
        Ok(())
    }

    #[test]
    fn same_metric_event_can_drive_progress_and_structured_logs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let progress = SharedWriter::default();
        let structured = SharedWriter::default();
        let collector = LocalTrainMetricCollector::default();
        let mut fanout = LocalTrainMetricFanout::new("run-local-metrics");
        fanout.add_sink(LocalTrainMetricProgressSink::new(progress.clone()));
        fanout.add_sink(LocalTrainMetricStructuredLogSink::new(structured.clone()));
        fanout.add_sink(collector.clone());
        let recorded = event(3, "mean_microbatch_loss", LocalTrainMetricValue::F32(0.75));

        fanout.record(recorded.clone())?;
        fanout.flush()?;

        assert!(progress
            .contents()
            .contains("run-local-metrics train step=3"));
        assert!(structured.contents().starts_with("metric_event {"));
        assert_eq!(collector.events(), vec![recorded]);
        Ok(())
    }
}
