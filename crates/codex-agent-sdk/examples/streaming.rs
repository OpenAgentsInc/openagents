use codex_agent_sdk::{Codex, ThreadEvent, ThreadItemDetails, ThreadOptions, TurnOptions};

#[tokio::main]
async fn main() -> Result<(), codex_agent_sdk::Error> {
    // Create a new Codex client
    let codex = Codex::new();

    // Start a new thread with default options
    let mut thread = codex.start_thread(ThreadOptions::default());

    println!("Starting Codex agent with streaming...\n");

    // Run a query that will generate multiple events
    let mut streamed = thread
        .run_streamed(
            "List the files in the current directory and explain what you find",
            TurnOptions::default(),
        )
        .await?;

    // Process events as they arrive
    while let Some(event) = streamed.next().await {
        match event? {
            ThreadEvent::ThreadStarted(started) => {
                println!("Thread started: {}", started.thread_id);
            }

            ThreadEvent::TurnStarted(_) => {
                println!("Turn started\n");
            }

            ThreadEvent::ItemStarted(item) => match &item.item.details {
                ThreadItemDetails::Reasoning(_) => {
                    println!("[Reasoning] Agent is thinking...");
                }
                ThreadItemDetails::CommandExecution(cmd) => {
                    println!("[Command] Executing: {}", cmd.command);
                }
                ThreadItemDetails::FileChange(file) => {
                    if let Some(change) = file.changes.first() {
                        println!("[File] Modifying: {}", change.path);
                    }
                }
                ThreadItemDetails::WebSearch(search) => {
                    println!("[Search] Query: {}", search.query);
                }
                ThreadItemDetails::McpToolCall(tool) => {
                    println!("[MCP Tool] Calling: {}", tool.tool);
                }
                ThreadItemDetails::AgentMessage(_) => {
                    println!("[Message] Agent is responding...");
                }
                ThreadItemDetails::TodoList(_) => {
                    println!("[Todo] Updating task list...");
                }
                ThreadItemDetails::Error(err) => {
                    println!("[Error] {}", err.message);
                }
            },

            ThreadEvent::ItemCompleted(item) => match &item.item.details {
                ThreadItemDetails::AgentMessage(msg) => {
                    println!("\nAgent response:\n{}\n", msg.text);
                }
                ThreadItemDetails::CommandExecution(cmd) => {
                    if !cmd.aggregated_output.is_empty() {
                        println!("Command output:\n{}\n", cmd.aggregated_output);
                    }
                }
                ThreadItemDetails::Reasoning(reasoning) => {
                    println!("Reasoning: {}\n", reasoning.text);
                }
                _ => {}
            },

            ThreadEvent::TurnCompleted(tc) => {
                println!("\nTurn completed!");
                println!("Usage:");
                println!("  Input tokens: {}", tc.usage.input_tokens);
                println!("  Cached input tokens: {}", tc.usage.cached_input_tokens);
                println!("  Output tokens: {}", tc.usage.output_tokens);
            }

            ThreadEvent::TurnFailed(failed) => {
                eprintln!("\nTurn failed: {}", failed.error.message);
            }

            ThreadEvent::Error(err) => {
                eprintln!("\nError: {}", err.message);
            }

            _ => {}
        }
    }

    // Print the thread ID for potential resumption
    if let Some(id) = thread.id() {
        println!("\nThread ID: {}", id);
    }

    Ok(())
}
