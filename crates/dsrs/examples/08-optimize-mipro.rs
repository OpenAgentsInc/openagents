/*
Example: Optimize a QA module using MIPROv2

This example demonstrates the advanced MIPROv2 optimizer, which uses a 3-stage process:
1. Generate traces from your training data
2. Use an LLM to generate candidate prompts with best practices
3. Evaluate candidates and select the best one

MIPROv2 is more sophisticated than COPRO and typically produces better results
by leveraging prompting best practices and program understanding.

Run with:
```
cargo run --example 08-optimize-mipro --features dataloaders
```

Note: The `dataloaders` feature is required for loading datasets.
*/

use anyhow::Result;
use bon::Builder;
use dsrs::{
    ChatAdapter, DataLoader, Evaluator, Example, LM, MIPROv2, Module, Optimizable, Optimizer,
    Predict, Prediction, Predictor, Signature, configure, example,
};

#[Signature]
struct QuestionAnswering {
    /// Answer the question accurately and concisely.

    #[input]
    pub question: String,

    #[output]
    pub answer: String,
}

#[derive(Builder, Optimizable)]
pub struct SimpleQA {
    #[parameter]
    #[builder(default = Predict::new(QuestionAnswering::new()))]
    pub answerer: Predict,
}

impl Module for SimpleQA {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        self.answerer.forward(inputs).await
    }
}

impl Evaluator for SimpleQA {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        let expected = example
            .data
            .get("answer")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let predicted = prediction
            .data
            .get("answer")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Normalize and compare
        let expected_normalized = expected.to_lowercase().trim().to_string();
        let predicted_normalized = predicted.to_lowercase().trim().to_string();

        if expected_normalized == predicted_normalized {
            1.0
        } else {
            // Partial credit for substring matches
            if expected_normalized.contains(&predicted_normalized)
                || predicted_normalized.contains(&expected_normalized)
            {
                0.5
            } else {
                0.0
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== MIPROv2 Optimizer Example ===\n");

    // Configure the LM
    configure(LM::default(), ChatAdapter);

    // Load training data from HuggingFace
    println!("Loading training data from HuggingFace...");
    let train_examples = DataLoader::load_hf(
        "hotpotqa/hotpot_qa",
        vec!["question".to_string()],
        vec!["answer".to_string()],
        "fullwiki",
        "validation",
        true,
    )?;

    // Use a small subset for faster optimization
    let train_subset = train_examples[..15].to_vec();
    println!("Using {} training examples\n", train_subset.len());

    // Create the module
    let mut qa_module = SimpleQA::builder().build();

    // Show initial instruction
    println!("Initial instruction:");
    println!(
        "  \"{}\"\n",
        qa_module.answerer.get_signature().instruction()
    );

    // Test baseline performance
    println!("Evaluating baseline performance...");
    let baseline_score = qa_module.evaluate(train_subset[..5].to_vec()).await;
    println!("Baseline score: {:.3}\n", baseline_score);

    // Create MIPROv2 optimizer
    let optimizer = MIPROv2::builder()
        .num_candidates(8) // Generate 8 candidate prompts
        .num_trials(15) // Run 15 evaluation trials
        .minibatch_size(10) // Evaluate on 10 examples per candidate
        .temperature(1.0) // Temperature for prompt generation
        .track_stats(true) // Display detailed statistics
        .build();

    // Optimize the module
    println!("Starting MIPROv2 optimization...");
    println!("This will:");
    println!("  1. Generate execution traces");
    println!("  2. Create a program description using LLM");
    println!("  3. Generate {} candidate prompts with best practices", 8);
    println!("  4. Evaluate each candidate");
    println!("  5. Select and apply the best prompt\n");

    optimizer
        .compile(&mut qa_module, train_subset.clone())
        .await?;

    // Show optimized instruction
    println!("\nOptimized instruction:");
    println!(
        "  \"{}\"\n",
        qa_module.answerer.get_signature().instruction()
    );

    // Test optimized performance
    println!("Evaluating optimized performance...");
    let optimized_score = qa_module.evaluate(train_subset[..5].to_vec()).await;
    println!("Optimized score: {:.3}", optimized_score);

    // Show improvement
    let improvement = ((optimized_score - baseline_score) / baseline_score) * 100.0;
    println!(
        "\n✓ Improvement: {:.1}% ({:.3} -> {:.3})",
        improvement, baseline_score, optimized_score
    );

    // Test on a new example
    println!("\n--- Testing on a new example ---");
    let test_example = example! {
        "question": "input" => "What is the capital of France?",
    };

    let result = qa_module.forward(test_example).await?;
    println!("Question: What is the capital of France?");
    println!("Answer: {}", result.get("answer", None));

    println!("\n=== Example Complete ===");
    Ok(())
}
