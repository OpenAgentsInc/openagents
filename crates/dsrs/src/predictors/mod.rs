pub mod predict;
pub mod refine;

pub use predict::*;
pub use refine::*;

use crate::callbacks::DspyCallback;
use crate::{Example, LM, LmUsage, Prediction};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::Arc;

#[allow(async_fn_in_trait)]
pub trait Predictor: Send + Sync {
    async fn forward(&self, inputs: Example) -> anyhow::Result<Prediction>;
    async fn forward_with_config(&self, inputs: Example, lm: Arc<LM>)
    -> anyhow::Result<Prediction>;

    /// Forward with streaming callback support.
    async fn forward_with_streaming(
        &self,
        inputs: Example,
        lm: Arc<LM>,
        callback: Option<&dyn DspyCallback>,
    ) -> anyhow::Result<Prediction>;

    async fn batch(&self, inputs: Vec<Example>) -> Result<Vec<Prediction>> {
        let indexed_results: Vec<(usize, Result<Prediction>)> =
            stream::iter(inputs.into_iter().enumerate())
                .map(|(idx, input)| async move {
                    let result = self.forward(input).await;
                    (idx, result)
                })
                .buffer_unordered(32) // Match MAX_CONCURRENCY from Evaluator
                .collect()
                .await;

        // Sort results back to original order
        let mut indexed_results = indexed_results;
        indexed_results.sort_by_key(|(idx, _)| *idx);

        // Collect predictions and handle errors
        let mut predictions = Vec::with_capacity(indexed_results.len());
        for (_, result) in indexed_results {
            predictions.push(result?);
        }
        Ok(predictions)
    }

    async fn batch_with_config(
        &self,
        inputs: Vec<Example>,
        lm: Arc<LM>,
    ) -> Result<Vec<Prediction>> {
        let lm_ref = lm.clone();
        let indexed_results: Vec<(usize, Result<Prediction>)> =
            stream::iter(inputs.into_iter().enumerate())
                .map(|(idx, input)| {
                    let lm_clone = lm_ref.clone();
                    async move {
                        let result = self.forward_with_config(input, lm_clone).await;
                        (idx, result)
                    }
                })
                .buffer_unordered(32) // Match MAX_CONCURRENCY from Evaluator
                .collect()
                .await;

        // Sort results back to original order
        let mut indexed_results = indexed_results;
        indexed_results.sort_by_key(|(idx, _)| *idx);

        // Collect predictions and handle errors
        let mut predictions = Vec::with_capacity(indexed_results.len());
        for (_, result) in indexed_results {
            predictions.push(result?);
        }
        Ok(predictions)
    }
}

pub struct DummyPredict;

impl Predictor for DummyPredict {
    async fn forward(&self, inputs: Example) -> anyhow::Result<Prediction> {
        Ok(Prediction::new(inputs.data, LmUsage::default()))
    }

    #[allow(unused_variables)]
    async fn forward_with_config(
        &self,
        inputs: Example,
        lm: Arc<LM>,
    ) -> anyhow::Result<Prediction> {
        Ok(Prediction::new(inputs.data, LmUsage::default()))
    }

    #[allow(unused_variables)]
    async fn forward_with_streaming(
        &self,
        inputs: Example,
        lm: Arc<LM>,
        callback: Option<&dyn DspyCallback>,
    ) -> anyhow::Result<Prediction> {
        Ok(Prediction::new(inputs.data, LmUsage::default()))
    }
}
