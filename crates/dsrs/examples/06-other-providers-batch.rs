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
    example, hashmap, prediction,
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
        let answer_lm_usage = answerer_prediction.lm_usage;

        let inputs = Example::new(
            hashmap! {
                "answer".to_string() => answer.clone(),
                "question".to_string() => question.clone()
            },
            vec!["answer".to_string(), "question".to_string()],
            vec![],
        );
        let rating_prediction = self.rater.forward(inputs).await?;
        let rating_lm_usage = rating_prediction.lm_usage;

        Ok(prediction! {
            "answer"=> answer,
            "question"=> question,
            "rating"=> rating_prediction.data.get("rating").unwrap().clone(),
        }
        .set_lm_usage(answer_lm_usage + rating_lm_usage))
    }
}

#[tokio::main]
async fn main() {
    // OpenAI
    configure(
        LM::builder()
            .model("openai:codex-sonnet-4-5-20250929".to_string())
            .build()
            .await
            .unwrap(),
        ChatAdapter,
    );

    let example = vec![
        example! {
            "question": "input" => "What is the capital of France?",
        },
        example! {
            "question": "input" => "What is the capital of Germany?",
        },
        example! {
            "question": "input" => "What is the capital of Italy?",
        },
    ];

    let qa_rater = QARater::builder().build();
    let prediction = qa_rater.batch(example.clone(), 2, true).await.unwrap();
    println!("OpenAI: {prediction:?}");

    // Gemini
    configure(
        LM::builder()
            .model("gemini:gemini-2.0-flash".to_string())
            .build()
            .await
            .unwrap(),
        ChatAdapter,
    );

    let prediction = qa_rater.batch(example, 2, true).await.unwrap();
    println!("Gemini: {prediction:?}");
}
