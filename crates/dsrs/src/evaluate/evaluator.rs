use crate::core::Module;
use crate::data::{example::Example, prediction::Prediction};
use futures::stream::{self, StreamExt};

#[allow(async_fn_in_trait)]
pub trait Evaluator: Module {
    const MAX_CONCURRENCY: usize = 32;
    const DISPLAY_PROGRESS: bool = true;

    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32;

    async fn evaluate(&self, examples: Vec<Example>) -> f32 {
        let predictions = self
            .batch(
                examples.clone(),
                Self::MAX_CONCURRENCY,
                Self::DISPLAY_PROGRESS,
            )
            .await
            .unwrap();

        let total = examples.len();

        // Pair examples with predictions and evaluate with controlled concurrency
        let metrics: Vec<f32> = stream::iter(examples.iter().zip(predictions.iter()).enumerate())
            .map(|(_, (example, prediction))| {
                let prediction = prediction.clone();
                async move { self.metric(example, &prediction).await }
            })
            .buffer_unordered(Self::MAX_CONCURRENCY)
            .collect()
            .await;

        metrics.iter().sum::<f32>() / total as f32
    }
}
