/*
Example: Using Tools with dsrs

This example demonstrates how to create and use custom tools with dsrs Predictors.
Tools allow LLMs to call external functions during prediction, enabling them to
perform calculations, lookups, API calls, and other operations.

Important Note: When tools are used, the LLM's final response after tool execution
must include field markers like [[ ## answer ## ]] for the parser to extract the answer.
If the LLM doesn't format its response with these markers, the answer field may be empty,
but you can still see that tools were called via the tool_calls and tool_executions fields.

Run with:
```
cargo run --example 15-tools
*/

use anyhow::Result;
use dsrs::{ChatAdapter, LM, Predict, Predictor, Signature, configure, example};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

// ============================================================================
// 1. Define Custom Tools
// ============================================================================

/// Args struct that matches the JSON schema
#[derive(Debug, Deserialize, Serialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

/// A simple calculator tool that can perform basic arithmetic operations
#[derive(Clone)]
struct CalculatorTool;

#[derive(Debug)]
struct CalculatorError(String);

impl fmt::Display for CalculatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Calculator error: {}", self.0)
    }
}

impl Error for CalculatorError {}

impl Tool for CalculatorTool {
    const NAME: &'static str = "calculator";

    type Error = CalculatorError;
    type Args = CalculatorArgs; // Typed args that match the JSON schema
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "A calculator that can perform arithmetic operations: add, subtract, multiply, divide, and power".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide", "power"],
                        "description": "The arithmetic operation to perform"
                    },
                    "a": {
                        "type": "number",
                        "description": "First number"
                    },
                    "b": {
                        "type": "number",
                        "description": "Second number"
                    }
                },
                "required": ["operation", "a", "b"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("[CalculatorTool] Called with: {:?}", args);
        println!(
            "[CalculatorTool] Performing {} on {} and {}",
            args.operation, args.a, args.b
        );

        let result = match args.operation.as_str() {
            "add" => args.a + args.b,
            "subtract" => args.a - args.b,
            "multiply" => args.a * args.b,
            "divide" => {
                if args.b == 0.0 {
                    return Err(CalculatorError("Division by zero".to_string()));
                }
                args.a / args.b
            }
            "power" => args.a.powf(args.b),
            _ => {
                return Err(CalculatorError(format!(
                    "Unknown operation: {}",
                    args.operation
                )));
            }
        };

        println!("[CalculatorTool] Result: {}", result);
        Ok(format!("{}", result))
    }
}

// ============================================================================
// 2. Define Signatures
// ============================================================================

#[Signature]
struct MathQuestionSignature {
    /// You MUST use the calculator tool to perform any calculations. Do not calculate manually.
    /// When asked a math question, call the calculator tool with the appropriate operation and numbers.
    #[input]
    question: String,

    #[output]
    answer: String,
}

// ============================================================================
// 3. Main Execution
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Setup LM
    let lm = LM::builder()
        .model("groq:openai/gpt-oss-120b".to_string())
        .build()
        .await?;
    configure(lm.clone(), ChatAdapter);

    println!("=== Using Tools with dsrs ===\n");

    // Create a predictor with the calculator tool
    let calculator_tool = CalculatorTool;
    let predictor = Predict::new_with_tools(
        MathQuestionSignature::new(),
        vec![Box::new(calculator_tool)],
    );

    println!("Created predictor with calculator tool\n");

    // Ask a math question - make it very explicit that the tool must be used
    // Some models need very explicit instructions to use tools
    let question = example! {
        "question": "input" => "I need you to calculate 15 multiplied by 23. You MUST call the calculator tool with operation='multiply', a=15, and b=23. Do not calculate this yourself - use the tool."
    };

    let prediction = predictor.forward(question).await?;
    println!("Question: Calculate 15 multiplied by 23 using the calculator tool");

    // Check if tools were called
    let tool_calls_count = prediction
        .data
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);

    if tool_calls_count == 0 {
        println!("\n⚠️  WARNING: No tool calls detected!");
        println!("The LLM did not call the calculator tool.");
        println!("This could mean:");
        println!("  1. The LLM chose to answer directly without using tools");
        println!("  2. The tool wasn't properly registered");
        println!("  3. The prompt didn't encourage tool use strongly enough\n");
    } else {
        println!("\n✓ Tool was called successfully!\n");
    }

    // Extract answer
    let answer_value = prediction.get("answer", None);
    let answer_str = answer_value.as_str().unwrap_or("");

    if answer_str.is_empty() {
        println!("Answer: (empty - LLM response may not have included field markers)");
    } else {
        println!("Answer: {}", answer_str);
    }
    println!();

    // Print tool usage details
    if let Some(tool_calls) = prediction.data.get("tool_calls") {
        if let Some(calls_array) = tool_calls.as_array() {
            println!("Tool calls made: {}", calls_array.len());
            for (i, call) in calls_array.iter().enumerate() {
                if let Some(call_obj) = call.as_object()
                    && let Some(func) = call_obj.get("function")
                    && let Some(func_obj) = func.as_object()
                {
                    let name = func_obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let args = func_obj
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    println!("  Tool call {}: {} with args: {}", i + 1, name, args);
                }
            }
        }
    } else {
        println!("Tool calls: None");
    }

    if let Some(tool_executions) = prediction.data.get("tool_executions") {
        if let Some(exec_array) = tool_executions.as_array() {
            println!("Tool executions:");
            for (i, exec) in exec_array.iter().enumerate() {
                let exec_str = exec.as_str().unwrap_or("N/A");
                println!("  Execution {}: {}", i + 1, exec_str);
            }
        }
    } else {
        println!("Tool executions: None");
    }

    Ok(())
}
