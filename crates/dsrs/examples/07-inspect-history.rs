/*
Script to inspect the history of an LM.

Run with:
```
cargo run --example 07-inspect-history
```
*/

use anyhow::Result;
use bon::Builder;
use dsrs::{
    ChatAdapter, Example, LM, Module, Predict, Prediction, Predictor, configure, example, get_lm,
    sign,
};

#[derive(Builder)]
pub struct QARater {
    #[builder(default = Predict::new(sign! { (question: String) -> answer: String }))]
    pub answerer: Predict,
}

impl Module for QARater {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        return self.answerer.forward(inputs.clone()).await;
    }
}

#[tokio::main]
async fn main() {
    let lm = LM::builder()
        .model("openai:gpt-4o-mini".to_string())
        .build()
        .await
        .unwrap();
    configure(lm, ChatAdapter);

    let example = example! {
        "question": "input" => "What is the capital of France?",
    };

    let qa_rater = QARater::builder().build();
    let prediction = qa_rater.forward(example.clone()).await.unwrap();
    println!("Prediction: {prediction:?}");

    let history = get_lm().inspect_history(1).await;
    println!("History: {history:?}");
}
