//! Core RLM engine implementing the prompt-execute-loop.

use std::process::Command as ProcessCommand;

use fm_bridge::FMClient;
use tracing::{debug, info, warn};

use crate::command::Command;
use crate::context::Context;
use crate::error::{Result, RlmError};
use crate::executor::{ExecutionEnvironment, ExecutionResult};
use crate::prompts::{continuation_prompt, error_prompt, initial_prompt, system_prompt_with_context};
use crate::subquery::{execute_sub_query, generate_result_injection, process_code_for_queries};

/// Configuration for the RLM engine.
#[derive(Debug, Clone)]
pub struct RlmConfig {
    /// Maximum number of iterations before giving up.
    pub max_iterations: u32,
    /// Whether to allow shell command execution.
    pub allow_shell: bool,
    /// Whether to log execution details.
    pub verbose: bool,
}

impl Default for RlmConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            allow_shell: false,
            verbose: false,
        }
    }
}

/// Result from an RLM execution.
#[derive(Debug, Clone)]
pub struct RlmResult {
    /// The final output.
    pub output: String,
    /// Number of iterations taken.
    pub iterations: u32,
    /// Execution log (code, output pairs).
    pub execution_log: Vec<ExecutionLogEntry>,
}

/// A single entry in the execution log.
#[derive(Debug, Clone)]
pub struct ExecutionLogEntry {
    /// The iteration number.
    pub iteration: u32,
    /// The LLM response.
    pub llm_response: String,
    /// The command that was parsed.
    pub command_type: String,
    /// The code or command that was executed.
    pub executed: String,
    /// The execution result.
    pub result: String,
}

/// The core RLM engine.
///
/// Implements the iterative prompt-execute-loop pattern:
/// 1. Send prompt to LLM
/// 2. Parse response for commands
/// 3. Execute commands in the REPL
/// 4. Feed results back to LLM
/// 5. Repeat until FINAL command
pub struct RlmEngine<E: ExecutionEnvironment> {
    /// The FM Bridge client for LLM inference.
    client: FMClient,
    /// The execution environment for running code.
    executor: E,
    /// Loaded context for the REPL (file/directory content).
    loaded_context: Option<Context>,
    /// Configuration options.
    config: RlmConfig,
}

impl<E: ExecutionEnvironment> RlmEngine<E> {
    /// Create a new RLM engine with default configuration.
    pub fn new(client: FMClient, executor: E) -> Self {
        Self {
            client,
            executor,
            loaded_context: None,
            config: RlmConfig::default(),
        }
    }

    /// Create a new RLM engine with custom configuration.
    pub fn with_config(client: FMClient, executor: E, config: RlmConfig) -> Self {
        Self {
            client,
            executor,
            loaded_context: None,
            config,
        }
    }

    /// Set the loaded context (file or directory content).
    pub fn set_context(&mut self, context: Context) {
        self.loaded_context = Some(context);
    }

    /// Wrap code with context injection if context is loaded.
    fn inject_context(&self, code: &str) -> String {
        if let Some(ref ctx) = self.loaded_context {
            // Escape the context content for Python string literal
            let escaped_content = ctx.content
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");

            format!(
                r#"# Injected context variable
context = """{}"""

# Helper functions for context queries
def search_context(pattern, max_results=10, window=200):
    """Search for a pattern in the context. Returns list of matches with surrounding text."""
    import re
    matches = []
    pattern_re = re.compile(re.escape(pattern), re.IGNORECASE)
    for match in pattern_re.finditer(context):
        start = max(0, match.start() - window)
        end = min(len(context), match.end() + window)
        matches.append({{
            "position": match.start(),
            "context": context[start:end],
        }})
        if len(matches) >= max_results:
            break
    return matches

# User code below
{}"#,
                escaped_content, code
            )
        } else {
            code.to_string()
        }
    }

    /// Process code for llm_query() calls and execute sub-queries.
    ///
    /// Returns the modified code with llm_query results injected as variables.
    async fn process_sub_queries(&self, code: &str) -> Result<String> {
        // Get context content if available
        let context_content = self.loaded_context.as_ref().map(|c| c.content.as_str());

        // Parse and process llm_query calls
        let processed = process_code_for_queries(code, context_content);

        if processed.pending_queries.is_empty() {
            return Ok(code.to_string());
        }

        if self.config.verbose {
            eprintln!(
                "[SUBQUERY] Processing {} llm_query calls",
                processed.pending_queries.len()
            );
        }

        // Execute each sub-query
        let mut results = Vec::new();
        for query in &processed.pending_queries {
            if self.config.verbose {
                eprintln!(
                    "[SUBQUERY] Executing: \"{}\" over {} chars",
                    query.prompt,
                    query.text.len()
                );
            }

            let result = execute_sub_query(&self.client, &query.prompt, &query.text).await?;

            if self.config.verbose {
                let preview = if result.len() > 100 {
                    format!("{}...", &result[..100])
                } else {
                    result.clone()
                };
                eprintln!("[SUBQUERY] Result: {}", preview);
            }

            results.push((query.id.clone(), result));
        }

        // Generate code to inject results
        let injection = generate_result_injection(&results);

        // Combine: injected results + processed code
        Ok(format!("{}{}", injection, processed.code))
    }

    /// Run the RLM on a query.
    ///
    /// This is the main entry point. It starts the prompt-execute-loop
    /// and returns when a FINAL command is received or max iterations
    /// are exceeded.
    pub async fn run(&self, query: &str) -> Result<RlmResult> {
        // Build initial prompt based on whether we have context
        let mut prompt = if let Some(ref ctx) = self.loaded_context {
            let system_prompt = system_prompt_with_context(ctx);
            format!(
                "{}\n\n---\n\nUser Query: {}\n\nYour response:",
                system_prompt, query
            )
        } else {
            initial_prompt(query)
        };

        let mut execution_log = Vec::new();
        let mut iteration = 0;

        info!("Starting RLM execution for query: {}", query);
        if self.loaded_context.is_some() {
            info!("Context loaded: {} chars", self.loaded_context.as_ref().unwrap().length);
        }

        loop {
            iteration += 1;

            if iteration > self.config.max_iterations {
                warn!("Max iterations exceeded: {}", self.config.max_iterations);
                return Err(RlmError::MaxIterationsExceeded(self.config.max_iterations));
            }

            if self.config.verbose {
                eprintln!("\n--- Iteration {} ---", iteration);
                eprintln!("[PROMPT TO FM]");
                eprintln!("{}", prompt);
                eprintln!("[/PROMPT TO FM]\n");
            }

            // Get LLM response
            let response = self.client.complete(&prompt, None).await?;
            let text = response
                .choices
                .first()
                .map(|c| c.message.content.clone())
                .unwrap_or_default();

            if self.config.verbose {
                debug!("LLM response: {}", text);
                eprintln!("[FM RESPONSE]");
                eprintln!("{}", text);
                eprintln!("[/FM RESPONSE]\n");
            }

            // Parse command from response
            let cmd = Command::parse(&text);

            let cmd_type = match &cmd {
                Command::Final(_) => "FINAL",
                Command::RunCode(_) => "RunCode",
                Command::RunCodeThenFinal(_, _) => "RunCode+FINAL",
                Command::Run(_) => "RUN",
                Command::Invalid => "Invalid",
            };

            if self.config.verbose {
                eprintln!("[PARSED] Command: {}", cmd_type);
            }

            match &cmd {
                Command::Final(result) => {
                    info!("FINAL received after {} iterations", iteration);
                    execution_log.push(ExecutionLogEntry {
                        iteration,
                        llm_response: text.clone(),
                        command_type: "FINAL".to_string(),
                        executed: String::new(),
                        result: result.clone(),
                    });

                    return Ok(RlmResult {
                        output: result.clone(),
                        iterations: iteration,
                        execution_log,
                    });
                }

                Command::RunCodeThenFinal(code, final_result) => {
                    // FM output both code and FINAL in one response
                    // Execute the code, then return the final result immediately
                    debug!("Executing code block (with pending FINAL)");

                    // Process sub-queries (llm_query calls) if any
                    let code_with_queries = self.process_sub_queries(code).await?;

                    // Inject context variable if loaded
                    let code_with_context = self.inject_context(&code_with_queries);

                    if self.config.verbose {
                        eprintln!("[EXECUTING PYTHON]");
                        eprintln!("{}", code);
                        eprintln!("[/EXECUTING PYTHON]\n");
                    }

                    let exec_result = self.executor.execute(&code_with_context).await?;

                    if self.config.verbose {
                        eprintln!("[EXECUTION RESULT]");
                        eprintln!("stdout: {}", exec_result.stdout);
                        if !exec_result.stderr.is_empty() {
                            eprintln!("stderr: {}", exec_result.stderr);
                        }
                        eprintln!("exit_code: {}", exec_result.exit_code);
                        eprintln!("duration: {}ms", exec_result.duration_ms);
                        eprintln!("[/EXECUTION RESULT]\n");
                    }

                    execution_log.push(ExecutionLogEntry {
                        iteration,
                        llm_response: text.clone(),
                        command_type: "RunCode+FINAL".to_string(),
                        executed: code.clone(),
                        result: exec_result.output().to_string(),
                    });

                    // If code executed successfully, return the FINAL result
                    if exec_result.is_success() {
                        info!("FINAL received (with code) after {} iterations", iteration);
                        if self.config.verbose {
                            eprintln!("[FINAL] {}", final_result);
                        }
                        return Ok(RlmResult {
                            output: final_result.clone(),
                            iterations: iteration,
                            execution_log,
                        });
                    } else {
                        // Code failed, continue with error prompt
                        prompt = error_prompt(&exec_result.stderr);
                    }
                }

                Command::RunCode(code) => {
                    debug!("Executing code block");

                    // Process sub-queries (llm_query calls) if any
                    let code_with_queries = self.process_sub_queries(code).await?;

                    // Inject context variable if loaded
                    let code_with_context = self.inject_context(&code_with_queries);

                    if self.config.verbose {
                        eprintln!("[EXECUTING PYTHON]");
                        eprintln!("{}", code);
                        eprintln!("[/EXECUTING PYTHON]\n");
                    }

                    let exec_result = self.executor.execute(&code_with_context).await?;

                    if self.config.verbose {
                        eprintln!("[EXECUTION RESULT]");
                        eprintln!("stdout: {}", exec_result.stdout);
                        if !exec_result.stderr.is_empty() {
                            eprintln!("stderr: {}", exec_result.stderr);
                        }
                        eprintln!("exit_code: {}", exec_result.exit_code);
                        eprintln!("duration: {}ms", exec_result.duration_ms);
                        eprintln!("[/EXECUTION RESULT]\n");
                    }

                    execution_log.push(ExecutionLogEntry {
                        iteration,
                        llm_response: text.clone(),
                        command_type: "RunCode".to_string(),
                        executed: code.clone(),
                        result: exec_result.output().to_string(),
                    });

                    prompt = if exec_result.is_success() {
                        continuation_prompt(&exec_result.stdout)
                    } else {
                        error_prompt(&exec_result.stderr)
                    };
                }

                Command::Run(args) => {
                    if !self.config.allow_shell {
                        warn!("Shell execution disabled, rejecting RUN command");
                        if self.config.verbose {
                            eprintln!("[SHELL BLOCKED] {} {:?}", args.program, args.args);
                        }
                        execution_log.push(ExecutionLogEntry {
                            iteration,
                            llm_response: text.clone(),
                            command_type: "RUN".to_string(),
                            executed: format!("{} {:?}", args.program, args.args),
                            result: "Shell execution is disabled".to_string(),
                        });
                        prompt = error_prompt("Shell execution is disabled for security reasons.");
                        continue;
                    }

                    debug!("Executing shell command: {} {:?}", args.program, args.args);

                    if self.config.verbose {
                        eprintln!("[EXECUTING SHELL]");
                        eprintln!("{} {:?}", args.program, args.args);
                        eprintln!("[/EXECUTING SHELL]\n");
                    }

                    let exec_result = self.execute_shell(&args.program, &args.args)?;

                    if self.config.verbose {
                        eprintln!("[EXECUTION RESULT]");
                        eprintln!("stdout: {}", exec_result.stdout);
                        if !exec_result.stderr.is_empty() {
                            eprintln!("stderr: {}", exec_result.stderr);
                        }
                        eprintln!("exit_code: {}", exec_result.exit_code);
                        eprintln!("[/EXECUTION RESULT]\n");
                    }

                    execution_log.push(ExecutionLogEntry {
                        iteration,
                        llm_response: text.clone(),
                        command_type: "RUN".to_string(),
                        executed: format!("{} {:?}", args.program, args.args),
                        result: exec_result.output().to_string(),
                    });

                    prompt = if exec_result.is_success() {
                        continuation_prompt(&exec_result.stdout)
                    } else {
                        error_prompt(&exec_result.stderr)
                    };
                }

                Command::Invalid => {
                    debug!("No valid command found in response");
                    execution_log.push(ExecutionLogEntry {
                        iteration,
                        llm_response: text.clone(),
                        command_type: "Invalid".to_string(),
                        executed: String::new(),
                        result: "No valid command found".to_string(),
                    });

                    // Ask LLM to try again
                    prompt = "Your response did not contain a valid command. Please use one of: RUN, FINAL, or a ```repl code block.".to_string();
                }
            }
        }
    }

    /// Execute a shell command.
    fn execute_shell(&self, program: &str, args: &[String]) -> Result<ExecutionResult> {
        let output = ProcessCommand::new(program)
            .args(args)
            .output()
            .map_err(|e| RlmError::ShellError(e.to_string()))?;

        Ok(ExecutionResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_executor::MockExecutor;

    // Note: These tests require FM Bridge to be running
    // For unit tests, we'd need to mock the FMClient

    #[test]
    fn test_config_default() {
        let config = RlmConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert!(!config.allow_shell);
        assert!(!config.verbose);
    }

    #[test]
    fn test_rlm_result() {
        let result = RlmResult {
            output: "42".to_string(),
            iterations: 3,
            execution_log: vec![],
        };
        assert_eq!(result.output, "42");
        assert_eq!(result.iterations, 3);
    }

    #[tokio::test]
    async fn test_mock_executor_works() {
        let executor = MockExecutor::new().expect("2 + 2", "4");

        let result = executor.execute("print(2 + 2)").await.unwrap();
        assert_eq!(result.stdout, "4");
    }
}
