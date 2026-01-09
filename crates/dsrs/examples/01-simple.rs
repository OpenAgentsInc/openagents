/*
Script to run a simple pipeline.

Run with:
```
cargo run --example 01-simple
```
*/

use anyhow::Result;
use bon::Builder;
use dsrs::{
    ChatAdapter, Example, LM, Module, Predict, Prediction, Predictor, Signature, configure,
    example, prediction,
};

#[Signature(cot)]
struct QASignature {
    #[input]
    pub question: String,

    #[output]
    pub answer: String,
}

#[Signature]
struct RateSignature {
    /// Rate the answer on a scale of 1(very bad) to 10(very good)

    #[input]
    pub question: String,

    #[input]
    pub answer: String,

    #[output]
    pub rating: i8,
}

#[derive(Builder)]
pub struct QARater {
    #[builder(default = Predict::new(QASignature::new()))]
    pub answerer: Predict,
    #[builder(default = Predict::new(RateSignature::new()))]
    pub rater: Predict,
}

impl Module for QARater {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        let answerer_prediction = self.answerer.forward(inputs.clone()).await?;

        let question = inputs.data.get("question").unwrap().clone();
        let answer = answerer_prediction.data.get("answer").unwrap().clone();

        let inputs = example! {
            "question": "input" => question.clone(),
            "answer": "output" => answer.clone()
        };

        let rating_prediction = self.rater.forward(inputs).await?;
        Ok(prediction! {
            "answer"=> answer,
            "question"=> question,
            "rating"=> rating_prediction.data.get("rating").unwrap().clone(),
        }
        .set_lm_usage(rating_prediction.lm_usage))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    configure(
        LM::builder()
            .model("openai:gpt-4o-mini".to_string())
            .build()
            .await
            .unwrap(),
        ChatAdapter,
    );

    let example = example! {
        "question": "input" => "What is the capital of France?",
    };

    let qa_rater = QARater::builder().build();
    let prediction = qa_rater.forward(example).await.unwrap();
    println!("{prediction:?}");

    Ok(())
}
