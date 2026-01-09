/*
Example demonstrating how to use LMClient::from_custom() with a custom Azure OpenAI client
in a simple pipeline, similar to 01-simple.rs.

This shows how to create a completion model directly and use it with LM.

Run with:
```
cargo run --example 11-custom-client
```
*/

use anyhow::Result;
use dsrs::{ChatAdapter, LM, LMClient, Predict, Predictor, Signature, configure, example};
use rig::providers::*;
use std::env;

#[Signature(cot)]
struct QASignature {
    #[input]
    pub question: String,

    #[output]
    pub answer: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create a custom Azure OpenAI completion model directly
    let api_key = env::var("AZURE_OPENAI_API_KEY").unwrap_or_else(|_| "dummy-key".to_string());
    let endpoint = env::var("AZURE_OPENAI_ENDPOINT")
        .unwrap_or_else(|_| "https://your-resource.openai.azure.com".to_string());

    let azure_client = azure::Client::builder()
        .api_key(api_key)
        .azure_endpoint(endpoint)
        .build()?;
    let azure_model = azure::CompletionModel::new(azure_client, "gpt-4o-mini"); // deployment name

    // Convert to LMClient using Into trait (enum_dispatch generates From implementations)
    let custom_lm_client: LMClient = azure_model.into();

    // Create LM with the custom client
    let lm = LM::builder()
        .build()
        .await?
        .with_client(custom_lm_client)
        .await?;

    // Configure the global settings with our custom LM
    configure(lm, ChatAdapter);

    let example = example! {
        "question": "input" => "What is the capital of France?",
    };

    let qa_predictor = Predict::new(QASignature::new());
    let prediction = qa_predictor.forward(example).await?;
    println!("{prediction:?}");

    Ok(())
}
