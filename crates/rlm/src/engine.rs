//! Core RLM engine implementing the prompt-execute-loop.

use std::collections::HashMap;
use std::process::Command as ProcessCommand;

use fm_bridge::FMClient;
use tracing::{debug, info, warn};

use crate::command::Command;
use crate::error::{Result, RlmError};
use crate::executor::{ExecutionEnvironment, ExecutionResult};
use crate::prompts::{continuation_prompt, error_prompt, initial_prompt};

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
    /// Context variables accessible to the REPL.
    context: HashMap<String, String>,
    /// Configuration options.
    config: RlmConfig,
}

impl<E: ExecutionEnvironment> RlmEngine<E> {
    /// Create a new RLM engine with default configuration.
    pub fn new(client: FMClient, executor: E) -> Self {
        Self {
            client,
            executor,
            context: HashMap::new(),
            config: RlmConfig::default(),
        }
    }

    /// Create a new RLM engine with custom configuration.
    pub fn with_config(client: FMClient, executor: E, config: RlmConfig) -> Self {
        Self {
            client,
            executor,
            context: HashMap::new(),
            config,
        }
    }

    /// Set a context variable accessible to the REPL.
    pub fn set_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.context.insert(key.into(), value.into());
    }

    /// Run the RLM on a query.
    ///
    /// This is the main entry point. It starts the prompt-execute-loop
    /// and returns when a FINAL command is received or max iterations
    /// are exceeded.
    pub async fn run(&self, query: &str) -> Result<RlmResult> {
        let mut prompt = initial_prompt(query);
        let mut execution_log = Vec::new();
        let mut iteration = 0;

        info!("Starting RLM execution for query: {}", query);

        loop {
            iteration += 1;

            if iteration > self.config.max_iterations {
                warn!("Max iterations exceeded: {}", self.config.max_iterations);
                return Err(RlmError::MaxIterationsExceeded(self.config.max_iterations));
            }

            if self.config.verbose {
                eprintln!("[RLM] Iteration {}: prompt ({} chars)", iteration, prompt.len());
                eprintln!("[RLM] Prompt preview: {}...", &prompt[..prompt.len().min(100)]);
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
            }

            if self.config.verbose {
                eprintln!("[RLM] Response: {}", &text[..text.len().min(200)]);
            }

            // Parse command from response
            let cmd = Command::parse(&text);

            if self.config.verbose {
                let cmd_type = match &cmd {
                    Command::Final(_) => "FINAL",
                    Command::RunCode(_) => "RunCode",
                    Command::Run(_) => "RUN",
                    Command::Invalid => "Invalid",
                };
                eprintln!("[RLM] Parsed command: {}", cmd_type);
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

                Command::RunCode(code) => {
                    debug!("Executing code block");
                    let exec_result = self.executor.execute(code).await?;

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
                    let exec_result = self.execute_shell(&args.program, &args.args)?;

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
