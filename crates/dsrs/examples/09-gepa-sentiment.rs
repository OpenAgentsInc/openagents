/// Example: Using GEPA to optimize a sentiment analysis module
///
/// This example demonstrates:
/// 1. Implementing FeedbackEvaluator with rich textual feedback
/// 2. Using GEPA optimizer for reflective prompt evolution
/// 3. Tracking optimization progress with detailed statistics
///
/// To run:
/// ```
/// OPENAI_API_KEY=your_key cargo run --example 09-gepa-sentiment
/// ```
use anyhow::Result;
use bon::Builder;
use dsrs::*;
use dsrs_macros::{Optimizable, Signature};

#[Signature]
struct SentimentSignature {
    /// Analyze the sentiment of the given text. Classify as 'Positive', 'Negative', or 'Neutral'.

    #[input]
    pub text: String,

    #[output]
    pub sentiment: String,

    #[output]
    pub reasoning: String,
}

#[derive(Builder, Optimizable)]
struct SentimentAnalyzer {
    #[parameter]
    predictor: Predict,
}

impl Module for SentimentAnalyzer {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        self.predictor.forward(inputs).await
    }
}

impl Evaluator for SentimentAnalyzer {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        let feedback = self.feedback_metric(example, prediction).await;
        feedback.score
    }
}

impl FeedbackEvaluator for SentimentAnalyzer {
    async fn feedback_metric(&self, example: &Example, prediction: &Prediction) -> FeedbackMetric {
        let predicted = prediction
            .get("sentiment", None)
            .as_str()
            .unwrap_or("")
            .to_string()
            .to_lowercase();

        let expected = example
            .get("expected_sentiment", None)
            .as_str()
            .unwrap_or("")
            .to_string()
            .to_lowercase();

        let text = example.get("text", None).as_str().unwrap_or("").to_string();

        let reasoning = prediction
            .get("reasoning", None)
            .as_str()
            .unwrap_or("")
            .to_string();

        // Calculate score
        let correct = predicted == expected;
        let score = if correct { 1.0 } else { 0.0 };

        // Create rich feedback
        let mut feedback = if correct {
            format!("Correct classification: \"{}\"\n", expected)
        } else {
            format!(
                "Incorrect classification\n  Expected: \"{}\"\n  Predicted: \"{}\"\n",
                expected, predicted
            )
        };

        // Add context about the input
        feedback.push_str(&format!("  Input text: \"{}\"\n", text));

        // Add reasoning analysis
        if !reasoning.is_empty() {
            feedback.push_str(&format!("  Reasoning: {}\n", reasoning));

            // Check if reasoning mentions key sentiment words
            let has_reasoning_quality = if correct {
                // For correct answers, check if reasoning is substantive
                reasoning.len() > 20
            } else {
                // For incorrect answers, note what went wrong
                false
            };

            if has_reasoning_quality {
                feedback.push_str("  Reasoning appears detailed\n");
            } else if !correct {
                feedback.push_str("  May have misunderstood the text sentiment\n");
            }
        }

        FeedbackMetric::new(score, feedback)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("GEPA Sentiment Analysis Optimization Example\n");

    // Setup LM
    let lm = LM::builder().temperature(0.7).build().await.unwrap();

    configure(lm.clone(), ChatAdapter);

    // Create training examples with diverse sentiments
    let trainset = vec![
        example! {
            "text": "input" => "This movie was absolutely fantastic! I loved every minute of it.",
            "expected_sentiment": "input" => "positive"
        },
        example! {
            "text": "input" => "Terrible service, will never come back again.",
            "expected_sentiment": "input" => "negative"
        },
        example! {
            "text": "input" => "The weather is okay, nothing special.",
            "expected_sentiment": "input" => "neutral"
        },
        example! {
            "text": "input" => "Despite some minor issues, I'm quite happy with the purchase.",
            "expected_sentiment": "input" => "positive"
        },
        example! {
            "text": "input" => "I have mixed feelings about this product.",
            "expected_sentiment": "input" => "neutral"
        },
        example! {
            "text": "input" => "This is the worst experience I've ever had!",
            "expected_sentiment": "input" => "negative"
        },
        example! {
            "text": "input" => "It's fine. Does what it's supposed to do.",
            "expected_sentiment": "input" => "neutral"
        },
        example! {
            "text": "input" => "Exceeded all my expectations! Highly recommend!",
            "expected_sentiment": "input" => "positive"
        },
        example! {
            "text": "input" => "Disappointed and frustrated with the outcome.",
            "expected_sentiment": "input" => "negative"
        },
        example! {
            "text": "input" => "Standard quality, nothing remarkable.",
            "expected_sentiment": "input" => "neutral"
        },
    ];

    // Create module
    let mut module = SentimentAnalyzer::builder()
        .predictor(Predict::new(SentimentSignature::new()))
        .build();

    // Evaluate baseline performance
    println!("Baseline Performance:");
    let baseline_score = module.evaluate(trainset.clone()).await;
    println!("  Average score: {:.3}\n", baseline_score);

    // Configure GEPA optimizer
    let gepa = GEPA::builder()
        .num_iterations(5)
        .minibatch_size(5)
        .num_trials(3)
        .temperature(0.9)
        .track_stats(true)
        .build();

    // Run optimization
    println!("Starting GEPA optimization...\n");
    let result = gepa
        .compile_with_feedback(&mut module, trainset.clone())
        .await?;

    // Display results
    println!("\nOptimization Results:");
    println!(
        "  Best average score: {:.3}",
        result.best_candidate.average_score()
    );
    println!("  Total rollouts: {}", result.total_rollouts);
    println!("  Total LM calls: {}", result.total_lm_calls);
    println!("  Generations: {}", result.evolution_history.len());

    println!("\nBest Instruction:");
    println!("  {}", result.best_candidate.instruction);

    if !result.evolution_history.is_empty() {
        println!("\nEvolution History:");
        for entry in &result.evolution_history {
            println!("  Generation {}: {:.3}", entry.0, entry.1);
        }
    }

    // Test optimized module on a new example
    println!("\nTesting Optimized Module:");
    let test_example = example! {
        "text": "input" => "This product changed my life! Absolutely amazing!",
        "expected_sentiment": "input" => "positive"
    };

    let test_prediction = module.forward(test_example.clone()).await?;
    let test_feedback = module
        .feedback_metric(&test_example, &test_prediction)
        .await;

    println!(
        "  Test prediction: {}",
        test_prediction.get("sentiment", None)
    );
    println!("  Test score: {:.3}", test_feedback.score);
    println!("  Feedback:\n{}", test_feedback.feedback);

    Ok(())
}
