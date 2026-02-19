/*
Script to evaluate the answerer of the QARater module for a tiny sample of the HotpotQA dataset.

Run with:
```
cargo run --example 03-evaluate-hotpotqa --features dataloaders
```

Note: The `dataloaders` feature is required for loading datasets.
*/

use anyhow::Result;
use bon::Builder;
use dsrs::{
    ChatAdapter, Evaluator, Example, LM, Module, Optimizable, Predict, Prediction, Predictor,
    Signature, configure,
};

use dsrs::DataLoader;

#[Signature(cot)]
struct QASignature {
    /// Concisely answer the question but be accurate. If it's a yes no question, answer with yes or no.

    #[input]
    pub question: String,

    #[output(desc = "Answer in less than 5 words.")]
    pub answer: String,
}

#[derive(Builder, Optimizable)]
pub struct QARater {
    #[parameter]
    #[builder(default = Predict::new(QASignature::new()))]
    pub answerer: Predict,
}

impl Module for QARater {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        let answerer_prediction = self.answerer.forward(inputs.clone()).await?;

        Ok(answerer_prediction)
    }
}

impl Evaluator for QARater {
    const MAX_CONCURRENCY: usize = 16;
    const DISPLAY_PROGRESS: bool = true;

    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        let answer = example.data.get("answer").unwrap().clone();
        let prediction = prediction.data.get("answer").unwrap().clone();

        if answer.to_string().to_lowercase() == prediction.to_string().to_lowercase() {
            1.0
        } else {
            0.0
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    configure(
        LM::builder()
            .model("openai:gpt-4o-mini".to_string())
            .build()
            .await?,
        ChatAdapter {},
    );

    let examples = DataLoader::load_hf(
        "hotpotqa/hotpot_qa",
        vec!["question".to_string()],
        vec!["answer".to_string()],
        "fullwiki",
        "validation",
        true,
    )?[..128]
        .to_vec();

    let evaluator = QARater::builder().build();
    let metric = evaluator.evaluate(examples).await;

    println!("Metric: {metric}");
    Ok(())
}
