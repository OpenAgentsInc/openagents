use anyhow::Result;
use futures::stream::{self, StreamExt};
use indexmap::IndexMap;
use kdam::{BarExt, tqdm};

use crate::{Example, Prediction, core::MetaSignature};

#[allow(async_fn_in_trait)]
pub trait Module: Send + Sync {
    async fn forward(&self, inputs: Example) -> Result<Prediction>;

    async fn batch(
        &self,
        inputs: Vec<Example>,
        max_concurrency: usize,
        display_progress: bool,
    ) -> Result<Vec<Prediction>> {
        let total = inputs.len();
        let mut pb = if display_progress {
            Some(tqdm!(total = total, desc = "Processing"))
        } else {
            None
        };

        // Pair each input with its index to maintain order
        let indexed_results: Vec<(usize, Result<Prediction>)> =
            stream::iter(inputs.into_iter().enumerate())
                .map(|(idx, example)| async move {
                    let result = self.forward(example).await;
                    (idx, result)
                })
                .buffer_unordered(max_concurrency)
                .inspect(|_| {
                    if let Some(ref mut progress) = pb {
                        let _ = progress.update(1);
                    }
                })
                .collect()
                .await;

        // Sort results back to original order
        let mut indexed_results = indexed_results;
        indexed_results.sort_by_key(|(idx, _)| *idx);

        // Collect predictions and handle errors
        let mut predictions = Vec::with_capacity(total);
        for (_, result) in indexed_results {
            predictions.push(result?);
        }

        Ok(predictions)
    }
}

#[allow(unused_variables)]
pub trait Optimizable {
    fn get_signature(&self) -> &dyn MetaSignature {
        todo!()
    }

    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable>;

    fn update_signature_instruction(&mut self, instruction: String) -> anyhow::Result<()> {
        todo!()
    }
}
