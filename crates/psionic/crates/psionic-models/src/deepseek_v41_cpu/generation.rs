//! Bounded deterministic greedy generation over the complete CPU transformer.
//!
//! Each call starts a new request. Cancellation is cooperative between chunks
//! and decode steps; a running matrix operation is not interrupted.

use super::{CpuError, finite, transformer::CpuTransformer};
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct GreedyOptions {
    pub max_output_tokens: usize,
    pub prefill_chunk_size: usize,
}
impl Default for GreedyOptions {
    fn default() -> Self {
        Self {
            max_output_tokens: 32,
            prefill_chunk_size: 128,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Eos,
    OutputLimit,
    ContextLimit,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GenerationOutput {
    pub prompt_tokens: usize,
    /// Tokens actually passed through all blocks. The final selected token,
    /// including EOS, does not need another forward pass and is not counted.
    pub processed_tokens: usize,
    /// Selected tokens only, including EOS when it causes the stop.
    pub generated_tokens: Vec<u32>,
    pub selected_logits: Vec<f32>,
    pub stop: StopReason,
}

/// Selects the smallest token ID among equal finite maxima.
pub fn argmax(logits: &[f32]) -> Result<u32, CpuError> {
    if logits.is_empty() || logits.len() > u32::MAX as usize {
        return Err(CpuError::Invalid("greedy vocabulary length"));
    }
    finite("greedy logits", logits)?;
    let mut best = 0;
    for i in 1..logits.len() {
        if logits[i] > logits[best] {
            best = i;
        }
    }
    Ok(best as u32)
}

impl CpuTransformer<'_> {
    /// Runs a fresh request, without sampling or implicit prompt truncation.
    /// Prompt plus selected tokens cannot exceed `max_context()`. EOS takes
    /// precedence over the output limit, which takes precedence over context.
    /// Errors leave the previous request state intact. Successful stops commit
    /// this request's processed state, including partial prefill on cancellation.
    pub fn generate_greedy(
        &mut self,
        prompt: &[u32],
        options: GreedyOptions,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<GenerationOutput, CpuError> {
        if prompt.is_empty()
            || prompt.len() > self.max_context()
            || prompt
                .iter()
                .any(|t| *t as usize >= self.config().vocabulary)
            || options.prefill_chunk_size == 0
        {
            return Err(CpuError::Invalid("greedy prompt or prefill chunk size"));
        }
        let mut next = self.clone();
        next.reset();
        let mut output = GenerationOutput {
            prompt_tokens: prompt.len(),
            processed_tokens: 0,
            generated_tokens: Vec::new(),
            selected_logits: Vec::new(),
            stop: StopReason::OutputLimit,
        };
        if options.max_output_tokens != 0 {
            let mut logits = Vec::new();
            for chunk in prompt.chunks(options.prefill_chunk_size) {
                if cancelled() {
                    output.stop = StopReason::Cancelled;
                    break;
                }
                logits = next.forward_tokens(chunk)?.logits;
            }
            if output.stop != StopReason::Cancelled {
                loop {
                    if prompt.len() + output.generated_tokens.len() >= next.max_context() {
                        output.stop = StopReason::ContextLimit;
                        break;
                    }
                    if cancelled() {
                        output.stop = StopReason::Cancelled;
                        break;
                    }
                    let token = argmax(&logits)?;
                    output.generated_tokens.push(token);
                    output.selected_logits.push(logits[token as usize]);
                    if token == next.config().eos_token {
                        output.stop = StopReason::Eos;
                        break;
                    }
                    if output.generated_tokens.len() == options.max_output_tokens {
                        break;
                    }
                    if prompt.len() + output.generated_tokens.len() == next.max_context() {
                        output.stop = StopReason::ContextLimit;
                        break;
                    }
                    // Check before the expensive next forward, not only after it.
                    if cancelled() {
                        output.stop = StopReason::Cancelled;
                        break;
                    }
                    logits = next.forward_tokens(&[token])?.logits;
                }
            }
        }
        output.processed_tokens = next.position();
        *self = next;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek_v41_cpu::{
        MatrixRows,
        projection::WeightSource,
        quantization::Arithmetic,
        transformer::tests::{self as fixture, ReferenceWeights},
    };
    use serde::Deserialize;
    use std::cell::Cell;

    #[derive(Deserialize)]
    struct Fixture {
        prompt: Vec<u32>,
        tokens: Vec<u32>,
        logits: Vec<Vec<f32>>,
    }
    fn options(max: usize) -> GreedyOptions {
        GreedyOptions {
            max_output_tokens: max,
            prefill_chunk_size: 2,
        }
    }
    #[test]
    fn maxima_refuse_nonfinite_and_break_ties_by_token_id() -> Result<(), CpuError> {
        assert_eq!(argmax(&[-0.0, 0.0, -1.0])?, 0);
        assert_eq!(argmax(&[-3.0, 2.0, 2.0])?, 1);
        for values in [
            vec![],
            vec![f32::NAN],
            vec![1.0, f32::INFINITY],
            vec![f32::NEG_INFINITY],
        ] {
            assert!(argmax(&values).is_err());
        }
        Ok(())
    }
    #[test]
    fn complete_model_greedy_tokens_and_logits_match_numpy()
    -> Result<(), Box<dyn std::error::Error>> {
        let f: Fixture =
            serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/greedy.json"))?;
        let w = fixture::weights()?;
        let mut config = fixture::config(false)?;
        config.eos_token = 7; // Fixture generates three non-EOS tokens.
        let mut model = CpuTransformer::new(&w, config, fixture::map()?, 16, Arithmetic::Float32)?;
        let output = model.generate_greedy(&f.prompt, options(3), || false)?;
        assert_eq!(output.generated_tokens, f.tokens);
        assert_eq!(output.stop, StopReason::OutputLimit);
        assert_eq!(output.processed_tokens, f.prompt.len() + 2);
        for (logit, (token, all)) in output
            .selected_logits
            .iter()
            .zip(f.tokens.iter().zip(&f.logits))
        {
            assert!((logit - all[*token as usize]).abs() < 2e-4);
        }
        model.reset();
        let mut full_logits = model.forward_tokens(&f.prompt)?.logits;
        for (i, expected) in f.logits.iter().enumerate() {
            for (a, b) in full_logits.iter().zip(expected) {
                assert!((a - b).abs() < 2e-4 + 3e-5 * b.abs());
            }
            if i + 1 < f.tokens.len() {
                full_logits = model.forward_tokens(&[f.tokens[i]])?.logits;
            }
        }
        assert_eq!(
            model.generate_greedy(&f.prompt, options(3), || false)?,
            output
        );
        Ok(())
    }
    #[test]
    fn eos_output_context_ties_and_validation_use_the_complete_model()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut w = fixture::weights()?;
        w.tensors
            .get_mut("head.weight")
            .ok_or("head missing")?
            .values
            .fill(0.0);
        let mut c = fixture::config(false)?;
        c.eos_token = 0;
        let mut model = CpuTransformer::new(&w, c, fixture::map()?, 5, Arithmetic::Float32)?;
        let eos = model.generate_greedy(&[3, 4], options(1), || false)?;
        assert_eq!(eos.stop, StopReason::Eos);
        assert_eq!(eos.generated_tokens, vec![0]);
        assert_eq!(eos.processed_tokens, 2);
        let mut model = CpuTransformer::new(
            &w,
            fixture::config(false)?,
            fixture::map()?,
            5,
            Arithmetic::Float32,
        )?;
        let limited = model.generate_greedy(&[3, 4], options(1), || false)?;
        assert_eq!(limited.stop, StopReason::OutputLimit);
        assert_eq!(limited.generated_tokens, vec![0]);
        let context = model.generate_greedy(&[3, 4], options(100), || false)?;
        assert_eq!(context.stop, StopReason::ContextLimit);
        assert_eq!(context.generated_tokens, vec![0, 0, 0]);
        assert_eq!(context.processed_tokens, 4);
        let full = model.generate_greedy(&[3, 4, 5, 6, 7], options(100), || false)?;
        assert_eq!(full.stop, StopReason::ContextLimit);
        assert!(full.generated_tokens.is_empty());
        assert_eq!(full.processed_tokens, 5);
        let before = model.summary();
        for prompt in [vec![], vec![8], vec![0; 6]] {
            assert!(
                model
                    .generate_greedy(&prompt, options(1), || false)
                    .is_err()
            );
            assert_eq!(model.summary(), before);
        }
        assert!(
            model
                .generate_greedy(
                    &[3],
                    GreedyOptions {
                        prefill_chunk_size: 0,
                        ..options(1)
                    },
                    || false
                )
                .is_err()
        );
        assert_eq!(model.summary(), before);
        let zero =
            model.generate_greedy(&[3], options(0), || panic!("zero output does no work"))?;
        assert_eq!(zero.stop, StopReason::OutputLimit);
        assert_eq!(zero.processed_tokens, 0);
        assert!(zero.generated_tokens.is_empty());
        assert_eq!(model.position(), 0);
        Ok(())
    }
    #[test]
    fn cancellation_before_prefill_during_prefill_and_decode()
    -> Result<(), Box<dyn std::error::Error>> {
        let w = ReferenceWeights;
        let mut model = CpuTransformer::new(
            &w,
            fixture::config(true)?,
            fixture::map()?,
            16,
            Arithmetic::ReferenceBf16,
        )?;
        let before = model.generate_greedy(&[3, 4, 5], options(4), || true)?;
        assert_eq!(before.stop, StopReason::Cancelled);
        assert_eq!(before.processed_tokens, 0);
        let mut checks = 0;
        let partial = model.generate_greedy(&[3, 4, 5], options(4), || {
            checks += 1;
            checks == 2
        })?;
        assert_eq!(partial.stop, StopReason::Cancelled);
        assert_eq!(partial.processed_tokens, 2);
        assert!(partial.generated_tokens.is_empty());
        let mut checks = 0;
        let partial = model.generate_greedy(&[3, 4], options(4), || {
            checks += 1;
            checks == 3
        })?;
        assert_eq!(partial.stop, StopReason::Cancelled);
        assert_eq!(partial.generated_tokens.len(), 1);
        assert_eq!(partial.processed_tokens, 2);
        let first = model.generate_greedy(&[3, 4], options(2), || false)?;
        let second = model.generate_greedy(&[3, 4], options(2), || false)?;
        assert_eq!(first, second);
        assert!(!first.generated_tokens.is_empty());
        Ok(())
    }
    struct Fail<'a> {
        source: &'a dyn WeightSource,
        heads: Cell<usize>,
        fail_on: Cell<usize>,
    }
    impl WeightSource for Fail<'_> {
        fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            if name == "head.weight" {
                self.heads.set(self.heads.get() + 1);
                if self.heads.get() == self.fail_on.get() {
                    return Err(CpuError::Invalid("injected decode failure"));
                }
            }
            self.source.matrix(name)
        }
        fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
            self.source.vector(name)
        }
    }
    #[test]
    fn generation_failure_preserves_the_previous_request() -> Result<(), Box<dyn std::error::Error>>
    {
        let w = fixture::weights()?;
        let fail = Fail {
            source: &w,
            heads: Cell::new(0),
            fail_on: Cell::new(usize::MAX),
        };
        let mut c = fixture::config(false)?;
        c.eos_token = 7;
        let mut model = CpuTransformer::new(&fail, c, fixture::map()?, 16, Arithmetic::Float32)?;
        model.forward_tokens(&[3])?;
        let previous = model.summary();
        fail.fail_on.set(fail.heads.get() + 2);
        let error = model
            .generate_greedy(&[0, 3], options(3), || false)
            .err()
            .ok_or("expected failure")?;
        assert!(error.to_string().contains("injected decode failure"));
        assert_eq!(model.summary(), previous);
        Ok(())
    }
}
