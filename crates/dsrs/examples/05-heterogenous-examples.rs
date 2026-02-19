/*
Script to run a heterogenous example.

Run with:
```
cargo run --example 05-heterogenous-examples
```
*/

use dsrs::{ChatAdapter, LM, Predict, Predictor, configure, example, sign};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    configure(
        LM::builder()
            .model("openai:gpt-4o-mini".to_string())
            .build()
            .await
            .unwrap(),
        ChatAdapter {},
    );

    let exp = example! {
        "number": "input" => 10,
    };
    let predict = Predict::new(sign! {
        (number: i32) -> number_squared: i32, number_cubed: i32
    });

    let prediction = predict.forward(exp).await?;
    println!("{prediction:?}");

    Ok(())
}
