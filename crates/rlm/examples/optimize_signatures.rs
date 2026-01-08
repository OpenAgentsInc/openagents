//! Offline signature optimization example.
//!
//! This example demonstrates how to use DSPy optimizers (COPRO, MIPROv2)
//! to improve signature prompts. Run optimization during development/CI,
//! not at runtime.
//!
//! # Running
//!
//! ```bash
//! # Set your API key
//! export OPENAI_API_KEY=your-key
//!
//! # Run optimizer
//! cargo run --example optimize_signatures --features dspy
//! ```
//!
//! # Optimization Workflow
//!
//! 1. Collect training examples (query/document pairs with expected outputs)
//! 2. Run optimizer on each signature type
//! 3. Save optimized prompts to assets/optimized_prompts/
//! 4. Load optimized prompts at runtime
//!
//! # Notes
//!
//! - Optimization is expensive (many LLM calls)
//! - Run on a representative sample of your data
//! - Review optimized prompts before deploying

#[cfg(feature = "dspy")]
use std::fs;
#[cfg(feature = "dspy")]
use std::path::Path;

#[cfg(not(feature = "dspy"))]
fn main() {
    println!("Error: This example requires the 'dspy' feature.");
    println!("Run with: cargo run --example optimize_signatures --features dspy");
}

#[cfg(feature = "dspy")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== DSPy Signature Optimizer ===\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        println!("Warning: OPENAI_API_KEY not set.");
        println!("Set it to run actual optimization.\n");
    }

    // Show the optimization workflow (scaffolding)
    println!("This is a scaffolding example showing the optimization workflow.\n");
    println!("To implement real optimization:\n");

    println!("1. CREATE TRAINING DATA");
    println!("   - Collect representative query/document pairs");
    println!("   - Include expected outputs for each");
    println!("   - Store in assets/training_data/*.json\n");

    println!("2. DEFINE METRIC FUNCTIONS");
    println!("   - RouterMetric: Does the router select relevant sections?");
    println!("   - ExtractorMetric: Are findings accurate and relevant?");
    println!("   - ReducerMetric: Is the final answer correct and complete?\n");

    println!("3. RUN OPTIMIZERS");
    println!("   ```rust");
    println!("   // Configure LM for optimization");
    println!("   configure_dspy_lm(\"openai:gpt-4o-mini\", None, None).await?;");
    println!();
    println!("   // Load training data");
    println!("   let train_data = load_training_data(\"assets/training_data/router.json\")?;");
    println!();
    println!("   // Create optimizer");
    println!("   let optimizer = COPRO::new(RouterMetric);");
    println!();
    println!("   // Optimize the signature");
    println!("   let optimized = optimizer.compile(");
    println!("       Predict::new(RouterSignature::new()),");
    println!("       train_data,");
    println!("   ).await?;");
    println!("   ```\n");

    println!("4. SAVE OPTIMIZED PROMPTS");
    println!("   - Save to assets/optimized_prompts/router.json");
    println!("   - Include version and training metadata");
    println!("   - Commit to version control\n");

    println!("5. LOAD AT RUNTIME");
    println!("   ```rust");
    println!("   // Load optimized instructions");
    println!("   let optimized = load_optimized(\"router\")?;");
    println!("   let router = Predict::new(RouterSignature::new())");
    println!("       .with_instructions(optimized.instructions);");
    println!("   ```\n");

    // Create directory structure if it doesn't exist
    let dirs = [
        "assets/training_data",
        "assets/optimized_prompts",
    ];

    for dir in dirs {
        let path = Path::new(dir);
        if !path.exists() {
            println!("Creating directory: {}", dir);
            fs::create_dir_all(path)?;
        }
    }

    // Create example training data file
    let example_training = r##"{
  "signature": "RouterSignature",
  "version": "0.1.0",
  "examples": [
    {
      "input": {
        "query": "What is the main topic of this document?",
        "document_preview": "# Introduction..."
      },
      "expected_output": {
        "relevant_sections": "[\"Introduction\", \"Overview\"]",
        "confidence": 0.9
      }
    }
  ],
  "notes": "Add more examples with diverse queries and documents"
}
"##;

    let training_path = "assets/training_data/router_example.json";
    if !Path::new(training_path).exists() {
        println!("Creating example training data: {}", training_path);
        fs::write(training_path, example_training)?;
    }

    // Create example optimized prompt file
    let example_optimized = r##"{
  "signature": "RouterSignature",
  "version": "0.1.0",
  "optimized_at": "2024-01-01T00:00:00Z",
  "optimizer": "COPRO",
  "instructions": "Given a query and document preview, identify the most relevant sections to examine. Focus on section headers, code blocks, and key terms that match the query intent.",
  "metrics": {
    "accuracy": 0.85,
    "f1": 0.82
  },
  "training_samples": 100
}
"##;

    let optimized_path = "assets/optimized_prompts/router_example.json";
    if !Path::new(optimized_path).exists() {
        println!("Creating example optimized prompt: {}", optimized_path);
        fs::write(optimized_path, example_optimized)?;
    }

    println!("\n=== Example files created ===");
    println!("- {}", training_path);
    println!("- {}", optimized_path);
    println!("\nEdit these files to add real training data, then run optimization.");

    Ok(())
}

/// Example metric function for router evaluation.
#[allow(dead_code)]
fn router_metric(prediction: &serde_json::Value, expected: &serde_json::Value) -> f32 {
    // Compare predicted sections with expected sections
    // Return score 0.0-1.0 based on overlap
    let pred_sections = prediction["relevant_sections"].as_str().unwrap_or("");
    let exp_sections = expected["relevant_sections"].as_str().unwrap_or("");

    // Simple overlap metric (in practice, use more sophisticated comparison)
    if pred_sections == exp_sections {
        1.0
    } else if !exp_sections.is_empty() && pred_sections.contains(&exp_sections[..exp_sections.len().min(20)]) {
        0.5
    } else {
        0.0
    }
}

/// Example: Load training data from JSON file.
#[allow(dead_code)]
#[cfg(feature = "dspy")]
fn load_training_data(_path: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    // In practice, load and parse the JSON file
    // Return Vec of training examples
    Ok(vec![])
}

/// Example: Load optimized prompt from JSON file.
#[allow(dead_code)]
#[cfg(feature = "dspy")]
fn load_optimized(_signature: &str) -> anyhow::Result<OptimizedPrompt> {
    // In practice, load from assets/optimized_prompts/{signature}.json
    Ok(OptimizedPrompt {
        instructions: String::new(),
        version: "0.0.0".to_string(),
    })
}

#[allow(dead_code)]
struct OptimizedPrompt {
    instructions: String,
    version: String,
}
