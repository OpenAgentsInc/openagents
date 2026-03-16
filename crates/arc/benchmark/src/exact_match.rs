use arc_core::{ArcBenchmark, ArcGrid, ArcTask, ArcTaskId, ContractSerializationError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcStaticAnswerKey {
    pub task_id: ArcTaskId,
    pub outputs: Vec<ArcGrid>,
}

impl ArcStaticAnswerKey {
    pub fn new(task_id: ArcTaskId, outputs: Vec<ArcGrid>) -> Result<Self, ArcBenchmarkError> {
        if outputs.is_empty() {
            return Err(ArcBenchmarkError::EmptyAnswerKey {
                task_id: task_id.clone(),
            });
        }
        Ok(Self { task_id, outputs })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcStaticPairSubmission {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<Option<ArcGrid>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcStaticTaskSubmission {
    pub task_id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_pairs: Vec<ArcStaticPairSubmission>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcExactMatchAttemptReport {
    pub attempt_index: u16,
    pub submitted: bool,
    pub correct: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcExactMatchPairReport {
    pub pair_index: u16,
    pub pair_correct: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_attempt_index: Option<u16>,
    pub expected_output_digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempt_reports: Vec<ArcExactMatchAttemptReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcExactMatchTaskReport {
    pub benchmark: ArcBenchmark,
    pub task_id: ArcTaskId,
    pub pairs_correct: u16,
    pub total_pairs: u16,
    pub score: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pair_reports: Vec<ArcExactMatchPairReport>,
}

impl ArcExactMatchTaskReport {
    #[must_use]
    pub fn is_exact_match(&self) -> bool {
        self.total_pairs > 0 && self.pairs_correct == self.total_pairs
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcExactMatchBenchmarkSummary {
    pub benchmark: ArcBenchmark,
    pub total_tasks: u32,
    pub exact_match_tasks: u32,
    pub total_pairs: u32,
    pub pairs_correct: u32,
    pub mean_task_score: f32,
    pub pair_accuracy: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_reports: Vec<ArcExactMatchTaskReport>,
}

impl ArcExactMatchBenchmarkSummary {
    pub fn from_task_reports(
        benchmark: ArcBenchmark,
        task_reports: Vec<ArcExactMatchTaskReport>,
    ) -> Result<Self, ArcBenchmarkError> {
        ensure_static_benchmark(benchmark)?;

        for report in &task_reports {
            if report.benchmark != benchmark {
                return Err(ArcBenchmarkError::TaskReportBenchmarkMismatch {
                    task_id: report.task_id.clone(),
                    expected: benchmark,
                    actual: report.benchmark,
                });
            }
        }

        let total_tasks = u32::try_from(task_reports.len()).unwrap_or(u32::MAX);
        let exact_match_tasks = task_reports
            .iter()
            .filter(|report| report.is_exact_match())
            .count();
        let total_pairs = task_reports
            .iter()
            .map(|report| u32::from(report.total_pairs))
            .sum();
        let pairs_correct = task_reports
            .iter()
            .map(|report| u32::from(report.pairs_correct))
            .sum();
        let total_score = task_reports.iter().map(|report| report.score).sum::<f32>();

        Ok(Self {
            benchmark,
            total_tasks,
            exact_match_tasks: u32::try_from(exact_match_tasks).unwrap_or(u32::MAX),
            total_pairs,
            pairs_correct,
            mean_task_score: if total_tasks == 0 {
                0.0
            } else {
                total_score / total_tasks as f32
            },
            pair_accuracy: if total_pairs == 0 {
                0.0
            } else {
                pairs_correct as f32 / total_pairs as f32
            },
            task_reports,
        })
    }
}

pub fn score_exact_match_task(
    benchmark: ArcBenchmark,
    task: &ArcTask,
    answer_key: &ArcStaticAnswerKey,
    submission: &ArcStaticTaskSubmission,
) -> Result<ArcExactMatchTaskReport, ArcBenchmarkError> {
    ensure_static_benchmark(benchmark)?;

    if answer_key.task_id != task.id {
        return Err(ArcBenchmarkError::AnswerKeyTaskMismatch {
            task_id: task.id.clone(),
            answer_key_task_id: answer_key.task_id.clone(),
        });
    }
    if submission.task_id != task.id {
        return Err(ArcBenchmarkError::SubmissionTaskMismatch {
            task_id: task.id.clone(),
            submission_task_id: submission.task_id.clone(),
        });
    }
    if answer_key.outputs.is_empty() {
        return Err(ArcBenchmarkError::EmptyAnswerKey {
            task_id: task.id.clone(),
        });
    }
    if answer_key.outputs.len() != task.test.len() {
        return Err(ArcBenchmarkError::AnswerKeyLengthMismatch {
            task_id: task.id.clone(),
            expected: task.test.len(),
            actual: answer_key.outputs.len(),
        });
    }
    if submission.test_pairs.len() > task.test.len() {
        return Err(ArcBenchmarkError::ExtraSubmissionPairs {
            task_id: task.id.clone(),
            expected: task.test.len(),
            actual: submission.test_pairs.len(),
        });
    }

    let mut pair_reports = Vec::with_capacity(task.test.len());
    let mut pairs_correct = 0u16;

    for (pair_index, expected_output) in answer_key.outputs.iter().enumerate() {
        let expected_output_digest = expected_output.contract_digest()?;
        let pair_submission = submission.test_pairs.get(pair_index);
        let attempt_reports = pair_submission
            .map(|pair_submission| {
                pair_submission
                    .attempts
                    .iter()
                    .enumerate()
                    .map(|(attempt_index, attempt)| {
                        let submitted = attempt.is_some();
                        let correct = attempt
                            .as_ref()
                            .is_some_and(|attempt| attempt == expected_output);
                        let prediction_digest =
                            attempt.as_ref().map(ArcGrid::contract_digest).transpose()?;
                        Ok(ArcExactMatchAttemptReport {
                            attempt_index: u16::try_from(attempt_index).unwrap_or(u16::MAX),
                            submitted,
                            correct,
                            prediction_digest,
                        })
                    })
                    .collect::<Result<Vec<_>, ArcBenchmarkError>>()
            })
            .transpose()?
            .unwrap_or_default();

        let matched_attempt_index = attempt_reports
            .iter()
            .find(|attempt| attempt.correct)
            .map(|attempt| attempt.attempt_index);
        let pair_correct = matched_attempt_index.is_some();
        if pair_correct {
            pairs_correct = pairs_correct.saturating_add(1);
        }

        pair_reports.push(ArcExactMatchPairReport {
            pair_index: u16::try_from(pair_index).unwrap_or(u16::MAX),
            pair_correct,
            matched_attempt_index,
            expected_output_digest,
            attempt_reports,
        });
    }

    let total_pairs = u16::try_from(task.test.len()).unwrap_or(u16::MAX);
    let score = if total_pairs == 0 {
        0.0
    } else {
        f32::from(pairs_correct) / f32::from(total_pairs)
    };

    Ok(ArcExactMatchTaskReport {
        benchmark,
        task_id: task.id.clone(),
        pairs_correct,
        total_pairs,
        score,
        pair_reports,
    })
}

fn ensure_static_benchmark(benchmark: ArcBenchmark) -> Result<(), ArcBenchmarkError> {
    match benchmark {
        ArcBenchmark::ArcAgi1 | ArcBenchmark::ArcAgi2 => Ok(()),
        other => Err(ArcBenchmarkError::UnsupportedStaticBenchmark { benchmark: other }),
    }
}

#[derive(Debug, Error)]
pub enum ArcBenchmarkError {
    #[error("exact-match scoring only supports ARC-AGI-1 and ARC-AGI-2, got {benchmark:?}")]
    UnsupportedStaticBenchmark { benchmark: ArcBenchmark },
    #[error("exact-match answer key for `{task_id}` must contain at least one test output")]
    EmptyAnswerKey { task_id: ArcTaskId },
    #[error("answer key task `{answer_key_task_id}` does not match task `{task_id}`")]
    AnswerKeyTaskMismatch {
        task_id: ArcTaskId,
        answer_key_task_id: ArcTaskId,
    },
    #[error("submission task `{submission_task_id}` does not match task `{task_id}`")]
    SubmissionTaskMismatch {
        task_id: ArcTaskId,
        submission_task_id: ArcTaskId,
    },
    #[error(
        "answer key for `{task_id}` contains {actual} outputs but task defines {expected} test pairs"
    )]
    AnswerKeyLengthMismatch {
        task_id: ArcTaskId,
        expected: usize,
        actual: usize,
    },
    #[error(
        "submission for `{task_id}` provides {actual} test pairs but the task only defines {expected}"
    )]
    ExtraSubmissionPairs {
        task_id: ArcTaskId,
        expected: usize,
        actual: usize,
    },
    #[error(
        "task report `{task_id}` used benchmark `{actual:?}` but summary expected `{expected:?}`"
    )]
    TaskReportBenchmarkMismatch {
        task_id: ArcTaskId,
        expected: ArcBenchmark,
        actual: ArcBenchmark,
    },
    #[error(transparent)]
    Serialization(#[from] ContractSerializationError),
}
