/*
Run with:
```
OPENAI_API_KEY=... cargo run -p dsrs --example 16-openai-responses-stream
```

Optional env:
  - OPENAI_MODEL (default: gpt-5-nano)
  - OPENAI_BASE_URL (override API base URL)
  - OPENAI_RESPONSES_STORE (true/false to enable store)
*/

use anyhow::Result;
use dsrs::{Example, LM, Predict, Predictor, Signature, example};
use dsrs::callbacks::DspyCallback;
use std::io::{self, Write};
use std::sync::Arc;
use uuid::Uuid;

#[Signature]
struct HaikuSignature {
    #[input]
    pub topic: String,

    #[output]
    pub haiku: String,
}

struct TokenPrinter;

impl DspyCallback for TokenPrinter {
    fn on_lm_token(&self, _call_id: Uuid, token: &str) {
        print!("{}", token);
        let _ = io::stdout().flush();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-nano".to_string());
    let lm = LM::builder()
        .model(format!("openai-responses:{model}"))
        .build()
        .await?;

    let predictor = Predict::new(HaikuSignature::new());
    let input = example! {
        "topic": "input" => "write a haiku about ai",
    };

    let printer = TokenPrinter;
    let prediction = predictor
        .forward_with_streaming(input, Arc::new(lm), Some(&printer))
        .await?;

    println!("\n\n{prediction:?}");
    Ok(())
}
