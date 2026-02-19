/// Example: Using LLM-as-a-Judge with GEPA for Math Word Problems
///
/// This example demonstrates how to use an LLM judge to automatically generate
/// rich textual feedback for GEPA optimization. The judge evaluates both the
/// correctness of answers AND the quality of reasoning.
///
/// To run:
/// ```
/// OPENAI_API_KEY=your_key cargo run --example 10-gepa-llm-judge
/// ```
use anyhow::Result;
use bon::Builder;
use dsrs::*;
use dsrs_macros::{Optimizable, Signature};
use std::sync::Arc;

// ============================================================================
// Step 1: Define the task signature with chain-of-thought reasoning
// ============================================================================

#[Signature(cot)]
struct MathWordProblem {
    /// Solve the math word problem step by step. Show your work clearly.

    #[input]
    pub problem: String,

    #[output]
    pub reasoning: String,

    #[output]
    pub answer: String,
}

// ============================================================================
// Step 2: Define the LLM judge signature
// ============================================================================

#[Signature]
struct MathJudge {
    /// You are an expert math teacher evaluating student work. Analyze both
    /// the final answer and the reasoning process. Be specific about what
    /// went wrong or what was done well.

    #[input(desc = "The math problem that was given")]
    pub problem: String,

    #[input(desc = "The expected correct answer")]
    pub expected_answer: String,

    #[input(desc = "The student's answer")]
    pub student_answer: String,

    #[input(desc = "The student's reasoning/work shown")]
    pub student_reasoning: String,

    #[output(desc = "Detailed evaluation of the work")]
    pub evaluation: String,
}

// ============================================================================
// Step 3: Create the main module with LLM judge
// ============================================================================

#[derive(Builder, Optimizable)]
struct MathSolver {
    // The main predictor we want to optimize
    #[parameter]
    solver: Predict,

    // The judge predictor (not optimized, just used for evaluation)
    judge: Predict,

    // LM for the judge (could be different/cheaper model)
    judge_lm: Arc<LM>,
}

impl Module for MathSolver {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        // Just forward to the solver - judge only used during evaluation
        self.solver.forward(inputs).await
    }
}

// ============================================================================
// Step 4: Implement regular Evaluator for non-GEPA optimizers
// ============================================================================

impl Evaluator for MathSolver {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        // For regular optimizers, just return scalar score
        let feedback = self.feedback_metric(example, prediction).await;
        feedback.score
    }
}

// ============================================================================
// Step 5: Implement FeedbackEvaluator with LLM judge for GEPA
// ============================================================================

impl FeedbackEvaluator for MathSolver {
    async fn feedback_metric(&self, example: &Example, prediction: &Prediction) -> FeedbackMetric {
        // Extract the problem and answers
        let problem = example
            .get("problem", None)
            .as_str()
            .unwrap_or("")
            .to_string();

        let expected = example
            .get("expected_answer", None)
            .as_str()
            .unwrap_or("")
            .to_string();

        let student_answer = prediction
            .get("answer", None)
            .as_str()
            .unwrap_or("")
            .to_string();

        let student_reasoning = prediction
            .get("reasoning", None)
            .as_str()
            .unwrap_or("No reasoning provided")
            .to_string();

        // Quick check: is the answer exactly correct?
        let answer_matches = student_answer.trim() == expected.trim();

        // Use LLM judge to analyze the reasoning quality
        // This is where the magic happens - the judge provides rich feedback
        let judge_input = example! {
            "problem": "input" => problem.clone(),
            "expected_answer": "input" => expected.clone(),
            "student_answer": "input" => student_answer.clone(),
            "student_reasoning": "input" => student_reasoning.clone()
        };

        let judge_output = match self
            .judge
            .forward_with_config(judge_input, Arc::clone(&self.judge_lm))
            .await
        {
            Ok(output) => output,
            Err(_) => {
                // If judge fails, fall back to simple feedback
                let score = if answer_matches { 1.0 } else { 0.0 };
                let simple_feedback = format!(
                    "Problem: {}\nExpected: {}\nPredicted: {}\nAnswer: {}",
                    problem,
                    expected,
                    student_answer,
                    if answer_matches {
                        "CORRECT"
                    } else {
                        "INCORRECT"
                    }
                );
                return FeedbackMetric::new(score, simple_feedback);
            }
        };

        let judge_evaluation = judge_output
            .get("evaluation", None)
            .as_str()
            .unwrap_or("Unable to evaluate")
            .to_string();

        // Calculate score based on answer correctness and reasoning quality
        // The judge's evaluation helps us assign partial credit
        let score = if answer_matches {
            // Correct answer - check if reasoning is also sound
            if judge_evaluation.to_lowercase().contains("sound reasoning")
                || judge_evaluation.to_lowercase().contains("correct approach")
            {
                1.0 // Perfect: right answer, good reasoning
            } else {
                0.7 // Right answer but flawed reasoning (lucky guess?)
            }
        } else {
            // Wrong answer - check if there's any partial credit
            if judge_evaluation.to_lowercase().contains("correct approach")
                || judge_evaluation.to_lowercase().contains("good start")
            {
                0.3 // Wrong answer but some valid steps
            } else {
                0.0 // Completely wrong
            }
        };

        // Construct rich textual feedback
        // This combines factual info with the judge's analysis
        let mut feedback = String::new();

        feedback.push_str(&format!("Problem: {}\n", problem));
        feedback.push_str(&format!("Expected: {}\n", expected));
        feedback.push_str(&format!("Predicted: {}\n", student_answer));

        if answer_matches {
            feedback.push_str("Answer: CORRECT\n\n");
        } else {
            feedback.push_str("Answer: INCORRECT\n\n");
        }

        feedback.push_str("Reasoning Quality Analysis:\n");
        feedback.push_str(&judge_evaluation);

        // Return the feedback metric with score and rich text
        FeedbackMetric::new(score, feedback)
    }
}

// ============================================================================
// Step 6: Main function - Set up and run GEPA optimization
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    println!("GEPA with LLM-as-a-Judge Example\n");
    println!("This example shows how to use an LLM judge to automatically");
    println!("generate rich feedback for optimizing a math solver.\n");

    // Setup: Configure the LLM
    // Main LM for the task
    let task_lm = LM::builder().temperature(0.7).build().await.unwrap();

    // Judge LM (could use a different/cheaper model)
    let judge_lm = LM::builder().temperature(0.3).build().await.unwrap();

    configure(task_lm, ChatAdapter);

    // Create training examples
    let trainset = vec![
        example! {
            "problem": "input" => "Sarah has 12 apples. She gives 3 to her friend and buys 5 more. How many apples does she have now?",
            "expected_answer": "input" => "14"
        },
        example! {
            "problem": "input" => "A train travels 60 miles in 1 hour. How far will it travel in 3.5 hours at the same speed?",
            "expected_answer": "input" => "210"
        },
        example! {
            "problem": "input" => "There are 24 students in a class. If 1/3 of them are absent, how many students are present?",
            "expected_answer": "input" => "16"
        },
        example! {
            "problem": "input" => "A rectangle has length 8 cm and width 5 cm. What is its area?",
            "expected_answer": "input" => "40"
        },
        example! {
            "problem": "input" => "John has $50. He spends $12 on lunch and $8 on a book. How much money does he have left?",
            "expected_answer": "input" => "30"
        },
    ];

    // Create the module
    let mut module = MathSolver::builder()
        .solver(Predict::new(MathWordProblem::new()))
        .judge(Predict::new(MathJudge::new()))
        .judge_lm(Arc::new(judge_lm))
        .build();

    // Evaluate baseline performance
    println!("Step 1: Baseline Performance");
    println!("Testing the solver before optimization...\n");
    let baseline_score = module.evaluate(trainset.clone()).await;
    println!("  Baseline average score: {:.3}\n", baseline_score);

    // Configure GEPA optimizer
    println!("Step 2: Configure GEPA");
    println!("Setting up the optimizer with budget controls...\n");

    let gepa = GEPA::builder()
        .num_iterations(3) // Fewer iterations for demo
        .minibatch_size(3) // Smaller batches
        .temperature(0.9)
        .track_stats(true)
        .maybe_max_lm_calls(Some(100)) // Important: we're using 2x LM calls (task + judge)
        .build();

    // Run GEPA optimization
    println!("Step 3: Run GEPA Optimization");
    println!("The judge will analyze reasoning quality and provide feedback...\n");

    let result = gepa
        .compile_with_feedback(&mut module, trainset.clone())
        .await?;

    // Display results
    println!("\nStep 4: Results");
    println!("===============\n");
    println!("Optimization complete!");
    println!(
        "  Best average score: {:.3}",
        result.best_candidate.average_score()
    );
    println!(
        "  Improvement: {:.3}",
        result.best_candidate.average_score() - baseline_score
    );
    println!("  Total rollouts: {}", result.total_rollouts);
    println!(
        "  Total LM calls: {} (includes judge evaluations)",
        result.total_lm_calls
    );

    println!("\nEvolution over time:");
    for (generation, score) in &result.evolution_history {
        println!("  Generation {}: {:.3}", generation, score);
    }

    println!("\nOptimized instruction:");
    println!("  {}", result.best_candidate.instruction);

    // Test the optimized solver
    println!("\nStep 5: Test Optimized Solver");
    println!("==============================\n");

    let test_problem = example! {
        "problem": "input" => "A store sells pencils for $0.25 each. If you buy 8 pencils, how much will you pay?",
        "expected_answer": "input" => "2"
    };

    let test_prediction = module.forward(test_problem.clone()).await?;
    let test_feedback = module
        .feedback_metric(&test_problem, &test_prediction)
        .await;

    println!(
        "Test problem: A store sells pencils for $0.25 each. If you buy 8 pencils, how much will you pay?"
    );
    println!("\nAnswer: {}", test_prediction.get("answer", None));
    println!("Score: {:.3}\n", test_feedback.score);
    println!("Detailed Feedback from Judge:");
    println!("{}", test_feedback.feedback);

    Ok(())
}
