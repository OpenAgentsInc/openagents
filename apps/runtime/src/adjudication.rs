use std::collections::HashMap;

use protocol::hash::{HashError, canonical_hash};
use protocol::verification::{AdjudicationStrategy, Verification};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AdjudicationError {
    #[error("no candidates provided")]
    NoCandidates,
    #[error("unsupported adjudication strategy: {0:?}")]
    UnsupportedStrategy(AdjudicationStrategy),
    #[error("candidate hashing failed: {0}")]
    Hash(#[from] HashError),
    #[error("judge required for judge_model adjudication")]
    JudgeRequired,
    #[error("judge returned invalid selection index")]
    JudgeInvalidSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdjudicationDecision {
    pub strategy: AdjudicationStrategy,
    pub selected_index: usize,
    pub selected_hash: String,
    pub candidate_hashes: Vec<String>,
    pub reason_codes: Vec<&'static str>,
}

pub trait JsonJudge: Send + Sync {
    fn select_index(
        &self,
        verification: &Verification,
        candidate_hashes: &[String],
    ) -> Result<usize, AdjudicationError>;
}

pub fn adjudicate_best_of_n<T: Serialize>(
    verification: &Verification,
    candidates: &[T],
    judge: Option<&dyn JsonJudge>,
) -> Result<AdjudicationDecision, AdjudicationError> {
    if candidates.is_empty() {
        return Err(AdjudicationError::NoCandidates);
    }

    let mut candidate_hashes = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        candidate_hashes.push(canonical_hash(candidate)?);
    }

    let mut reason_codes = Vec::new();
    let selected_index = match verification.adjudication {
        AdjudicationStrategy::None => {
            reason_codes.push("adjudication_none_select_first");
            0
        }
        AdjudicationStrategy::MajorityVote => {
            reason_codes.push("adjudication_majority_vote");
            majority_vote_index(&candidate_hashes)
        }
        AdjudicationStrategy::JudgeModel => {
            let judge = judge.ok_or(AdjudicationError::JudgeRequired)?;
            let idx = judge.select_index(verification, &candidate_hashes)?;
            if idx >= candidates.len() {
                return Err(AdjudicationError::JudgeInvalidSelection);
            }
            reason_codes.push("adjudication_judge_model");
            idx
        }
        AdjudicationStrategy::Merge => {
            return Err(AdjudicationError::UnsupportedStrategy(
                AdjudicationStrategy::Merge,
            ));
        }
    };

    Ok(AdjudicationDecision {
        strategy: verification.adjudication,
        selected_index,
        selected_hash: candidate_hashes[selected_index].clone(),
        candidate_hashes,
        reason_codes,
    })
}

fn majority_vote_index(candidate_hashes: &[String]) -> usize {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for hash in candidate_hashes {
        *counts.entry(hash.as_str()).or_insert(0) += 1;
    }

    let mut best_hash: Option<&str> = None;
    let mut best_count = 0usize;
    for (hash, count) in counts {
        match best_hash {
            None => {
                best_hash = Some(hash);
                best_count = count;
            }
            Some(current) => {
                if count > best_count || (count == best_count && hash < current) {
                    best_hash = Some(hash);
                    best_count = count;
                }
            }
        }
    }

    let winner = best_hash.unwrap_or(candidate_hashes[0].as_str());
    candidate_hashes
        .iter()
        .position(|hash| hash.as_str() == winner)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::verification::{AdjudicationStrategy, VerificationMode};

    #[derive(Debug, Clone, Serialize)]
    struct ToyResult {
        value: String,
    }

    struct LastIndexJudge;

    impl JsonJudge for LastIndexJudge {
        fn select_index(
            &self,
            _verification: &Verification,
            candidate_hashes: &[String],
        ) -> Result<usize, AdjudicationError> {
            Ok(candidate_hashes.len().saturating_sub(1))
        }
    }

    #[test]
    fn adjudication_none_selects_first() {
        let verification = Verification {
            mode: VerificationMode::Subjective,
            redundancy: 2,
            adjudication: AdjudicationStrategy::None,
            judge_model: None,
        };
        let candidates = vec![
            ToyResult {
                value: "a".to_string(),
            },
            ToyResult {
                value: "b".to_string(),
            },
        ];
        let decision = adjudicate_best_of_n(&verification, &candidates, None).expect("decision");
        assert_eq!(decision.selected_index, 0);
        assert_eq!(
            decision.reason_codes,
            vec!["adjudication_none_select_first"]
        );
    }

    #[test]
    fn adjudication_majority_vote_selects_majority() {
        let verification = Verification::subjective_with_majority(3);
        let candidates = vec![
            ToyResult {
                value: "same".to_string(),
            },
            ToyResult {
                value: "same".to_string(),
            },
            ToyResult {
                value: "diff".to_string(),
            },
        ];

        let decision = adjudicate_best_of_n(&verification, &candidates, None).expect("decision");
        assert_eq!(decision.selected_index, 0);
        assert_eq!(decision.reason_codes, vec!["adjudication_majority_vote"]);
    }

    #[test]
    fn adjudication_judge_model_requires_judge() {
        let verification = Verification::subjective_with_judge(2);
        let candidates = vec![ToyResult {
            value: "a".to_string(),
        }];
        let err = adjudicate_best_of_n(&verification, &candidates, None).unwrap_err();
        assert!(matches!(err, AdjudicationError::JudgeRequired));
    }

    #[test]
    fn adjudication_judge_model_uses_judge_selection() {
        let verification = Verification::subjective_with_judge(2);
        let candidates = vec![
            ToyResult {
                value: "a".to_string(),
            },
            ToyResult {
                value: "b".to_string(),
            },
        ];
        let judge = LastIndexJudge;
        let decision =
            adjudicate_best_of_n(&verification, &candidates, Some(&judge)).expect("decision");
        assert_eq!(decision.selected_index, 1);
        assert!(decision.reason_codes.contains(&"adjudication_judge_model"));
    }
}
