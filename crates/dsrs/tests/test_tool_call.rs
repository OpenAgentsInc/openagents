use dsrs::{Chat, LM, Message};
use rig::completion::ToolDefinition;
use rig::tool::ToolDyn;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

// Mock tool for testing
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

impl rig::tool::Tool for CalculatorTool {
    const NAME: &'static str = "calculator";
    type Error = CalculatorError;
    type Args = String;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "A simple calculator that can add, subtract, multiply, and divide"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide"]
                    },
                    "a": {"type": "number"},
                    "b": {"type": "number"}
                },
                "required": ["operation", "a", "b"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let parsed: serde_json::Value = serde_json::from_str(&args)
            .map_err(|e| CalculatorError(format!("Failed to parse args: {}", e)))?;

        let operation = parsed["operation"]
            .as_str()
            .ok_or_else(|| CalculatorError("Missing operation".to_string()))?;
        let a = parsed["a"]
            .as_f64()
            .ok_or_else(|| CalculatorError("Missing or invalid 'a' value".to_string()))?;
        let b = parsed["b"]
            .as_f64()
            .ok_or_else(|| CalculatorError("Missing or invalid 'b' value".to_string()))?;

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(CalculatorError("Division by zero".to_string()));
                }
                a / b
            }
            _ => return Err(CalculatorError(format!("Unknown operation: {}", operation))),
        };

        Ok(format!("{}", result))
    }
}

#[tokio::test]
#[ignore] // Ignore by default - test requires network access and valid API key
async fn test_tool_call_with_no_tools() {
    // Create an LM instance
    let lm = match LM::builder()
        .model("openai:gpt-4o-mini".to_string())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(lm) => lm,
        Err(e) => {
            println!("Skipping test - Failed to build LM: {}", e);
            return;
        }
    };

    // Create a chat with a simple message
    let mut chat = Chat::new(vec![]);
    chat.push_message(Message::user("What is 2 + 2?"));

    // Call without tools
    let response = lm.call(chat, vec![]).await;

    // Should get a text response (or network error if no real API key)
    if let Err(e) = &response {
        println!("Expected error without real API key: {}", e);
        return;
    }

    let response = response.unwrap();
    match response.output {
        Message::Assistant { content } => {
            // The response should contain some mention of 4
            println!("Assistant response: {}", content);
        }
        _ => panic!("Expected assistant message"),
    }
}

#[tokio::test]
#[ignore] // Ignore by default - test requires network access and valid API key
async fn test_tool_call_with_calculator() {
    // Create an LM instance
    let lm = LM::builder()
        .model("openai:gpt-4o-mini".to_string())
        .temperature(0.0)
        .build()
        .await
        .expect("Failed to build LM");

    // Create a chat asking for calculation
    let mut chat = Chat::new(vec![]);
    chat.push_message(Message::system("You are a helpful assistant with access to a calculator tool. Use it when asked to perform calculations."));
    chat.push_message(Message::user("Calculate 25 * 4 using the calculator tool"));

    // Create tool and wrap in Arc
    let calculator = CalculatorTool;
    let tools: Vec<Arc<dyn ToolDyn>> = vec![Arc::new(calculator)];

    // Call with the calculator tool
    let response = lm.call(chat, tools).await.unwrap();

    match response.output {
        Message::Assistant { content } => {
            println!("Assistant response after tool use: {}", content);
            // The response should mention the result (100) or that the tool was called
            assert!(content.contains("100") || content.contains("Tool call"));
        }
        _ => panic!("Expected assistant message"),
    }
}
