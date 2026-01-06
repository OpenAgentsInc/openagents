//! Command parsing for RLM responses.
//!
//! Parses LLM responses into executable commands:
//! - `RUN <program> <args>` - Execute a shell command
//! - `FINAL <message>` - Return final result and stop
//! - ` ```repl\n<code>\n``` ` - Execute code in the REPL environment

/// Arguments for a shell command.
#[derive(Debug, Clone)]
pub struct RunArgs {
    /// The program to execute.
    pub program: String,
    /// Arguments to pass to the program.
    pub args: Vec<String>,
}

/// Commands that can be parsed from LLM responses.
#[derive(Debug, Clone)]
pub enum Command {
    /// Execute a shell command.
    Run(RunArgs),
    /// Return a final result and terminate.
    Final(String),
    /// Execute code in the REPL environment.
    RunCode(String),
    /// Execute code, then immediately return final result (no more iterations).
    /// This handles the case where FM outputs code block + FINAL in same response.
    RunCodeThenFinal(String, String),
    /// No valid command found.
    Invalid,
}

impl Command {
    /// Parse a command from LLM response text.
    ///
    /// Recognizes command types:
    /// - `RUN <program> <args>` - Shell execution
    /// - `FINAL <message>` - Terminal output
    /// - ` ```repl\n<code>\n``` ` - Code execution
    /// - Code block + FINAL in same response - Execute then finalize
    ///
    /// If both code and FINAL are present, returns RunCodeThenFinal to execute
    /// the code and immediately return the final answer without more iterations.
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();

        // Check for code block
        if let Some(code) = Self::extract_code_block(input) {
            // Also check if there's a FINAL after the code block
            // This handles FMs that output code + FINAL in one response
            if let Some(final_result) = Self::find_final_after_code(input) {
                return Self::RunCodeThenFinal(code, final_result);
            }
            return Self::RunCode(code);
        }

        // Check for RUN command at start
        if trimmed.starts_with("RUN") {
            return Self::parse_run(trimmed);
        }

        // Check for FINAL anywhere in response
        if let Some(final_result) = Self::find_final(input) {
            return Self::Final(final_result);
        }

        Self::Invalid
    }

    /// Find FINAL command anywhere in the text.
    ///
    /// Supports multiple syntaxes from the RLM paper:
    /// - `FINAL answer here` - Space-separated
    /// - `FINAL(answer here)` - Function-style (paper's preferred format)
    /// - `FINAL_VAR(variable_name)` - Return a variable (paper format)
    fn find_final(input: &str) -> Option<String> {
        // Check for FINAL_VAR(variable_name) first
        if let Some(var_start) = input.find("FINAL_VAR(") {
            let after_paren = &input[var_start + 10..];
            if let Some(close_paren) = after_paren.find(')') {
                let var_name = &after_paren[..close_paren].trim();
                // Return the variable name - the REPL will handle extracting its value
                return Some(format!("VAR:{}", var_name));
            }
        }

        // Check for FINAL(answer) style
        if let Some(final_start) = input.find("FINAL(") {
            let after_paren = &input[final_start + 6..];
            if let Some(close_paren) = after_paren.find(')') {
                let answer = &after_paren[..close_paren];
                if !answer.trim().is_empty() {
                    return Some(answer.to_string());
                }
            }
        }

        // Fallback: Look for "FINAL" at the start of a line (legacy format)
        for line in input.lines() {
            let line = line.trim();
            if line.starts_with("FINAL") && !line.starts_with("FINAL_VAR") {
                let result = line
                    .split_ascii_whitespace()
                    .skip(1)
                    .collect::<Vec<&str>>()
                    .join(" ");
                if !result.is_empty() {
                    return Some(result);
                }
            }
        }
        None
    }

    /// Find FINAL command that appears after the last code block closes.
    /// This handles FMs that output both code and FINAL in one response.
    fn find_final_after_code(input: &str) -> Option<String> {
        // Find the last closing ``` of a code block
        let markers = ["```repl", "```python", "```py", "```"];
        let mut last_code_end = None;

        for start_marker in markers {
            if let Some(start_idx) = input.find(start_marker) {
                let code_start = start_idx + start_marker.len();
                let remaining = &input[code_start..];
                if let Some(end_idx) = remaining.find("```") {
                    let absolute_end = code_start + end_idx + 3; // +3 for "```"
                    if last_code_end.is_none() || absolute_end > last_code_end.unwrap() {
                        last_code_end = Some(absolute_end);
                    }
                }
            }
        }

        // Look for FINAL after the code block ends
        if let Some(end_pos) = last_code_end {
            let after_code = &input[end_pos..];

            // Check for FINAL_VAR(variable_name)
            if let Some(var_start) = after_code.find("FINAL_VAR(") {
                let after_paren = &after_code[var_start + 10..];
                if let Some(close_paren) = after_paren.find(')') {
                    let var_name = &after_paren[..close_paren].trim();
                    return Some(format!("VAR:{}", var_name));
                }
            }

            // Check for FINAL(answer)
            if let Some(final_start) = after_code.find("FINAL(") {
                let after_paren = &after_code[final_start + 6..];
                if let Some(close_paren) = after_paren.find(')') {
                    let answer = &after_paren[..close_paren];
                    if !answer.trim().is_empty() {
                        return Some(answer.to_string());
                    }
                }
            }

            // Legacy format fallback
            for line in after_code.lines() {
                let line = line.trim();
                if line.starts_with("FINAL") && !line.starts_with("FINAL_VAR") {
                    let result = line
                        .split_ascii_whitespace()
                        .skip(1)
                        .collect::<Vec<&str>>()
                        .join(" ");
                    if !result.is_empty() {
                        return Some(result);
                    }
                }
            }
        }
        None
    }

    /// Parse a RUN command.
    fn parse_run(input: &str) -> Self {
        let mut iter = input.split_ascii_whitespace().skip(1);
        let Some(program) = iter.next() else {
            return Self::Invalid;
        };
        let args: Vec<String> = iter.map(|x| x.to_owned()).collect();

        Self::Run(RunArgs {
            program: program.to_string(),
            args,
        })
    }

    /// Extract code from code blocks (```repl, ```python, or just ```).
    ///
    /// Following the paper: GPT-5 often outputs multiple ```repl blocks in one response.
    /// We concatenate all blocks to support this pattern.
    fn extract_code_block(input: &str) -> Option<String> {
        let mut all_code = Vec::new();
        let mut pos = 0;

        // Scan through input finding ANY code block marker
        while pos < input.len() {
            let remaining = &input[pos..];

            // Find the next occurrence of "```" (opening marker)
            if let Some(tick_idx) = remaining.find("```") {
                let tick_pos = pos + tick_idx;
                let after_ticks = &input[tick_pos + 3..];

                // Check what language marker follows (if any)
                let code_start = if after_ticks.starts_with("repl") {
                    tick_pos + 3 + 4 // Skip "```repl"
                } else if after_ticks.starts_with("python") {
                    tick_pos + 3 + 6 // Skip "```python"
                } else if after_ticks.starts_with("py") {
                    tick_pos + 3 + 2 // Skip "```py"
                } else {
                    tick_pos + 3 // Just "```"
                };

                // Skip optional newline after marker
                let code_start = if input[code_start..].starts_with('\n') {
                    code_start + 1
                } else if input[code_start..].starts_with("\r\n") {
                    code_start + 2
                } else {
                    code_start
                };

                // Find closing ```
                if let Some(end_idx) = input[code_start..].find("```") {
                    let code = input[code_start..code_start + end_idx].trim();
                    if !code.is_empty() {
                        all_code.push(code.to_string());
                    }
                    pos = code_start + end_idx + 3;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if all_code.is_empty() {
            return None;
        }

        // Concatenate all blocks with newlines
        Some(all_code.join("\n\n"))
    }

    /// Returns the run arguments if this is a Run command.
    pub fn as_run(&self) -> Option<&RunArgs> {
        if let Self::Run(args) = self {
            Some(args)
        } else {
            None
        }
    }

    /// Returns the result if this is a Final command.
    pub fn as_final(&self) -> Option<&str> {
        if let Self::Final(result) = self {
            Some(result)
        } else {
            None
        }
    }

    /// Returns the code if this is a RunCode command.
    pub fn as_code(&self) -> Option<&str> {
        if let Self::RunCode(code) = self {
            Some(code)
        } else {
            None
        }
    }

    /// Returns true if this is an Invalid command.
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_run() {
        let cmd = Command::parse("RUN ls -la /tmp");
        let args = cmd.as_run().unwrap();
        assert_eq!(args.program, "ls");
        assert_eq!(args.args, vec!["-la", "/tmp"]);
    }

    #[test]
    fn test_parse_final() {
        let cmd = Command::parse("FINAL The answer is 42");
        assert_eq!(cmd.as_final().unwrap(), "The answer is 42");
    }

    #[test]
    fn test_parse_code_block() {
        let input = r#"Let me calculate that.

```repl
result = 2 + 2
print(result)
```

That should give us the answer."#;

        let cmd = Command::parse(input);
        let code = cmd.as_code().unwrap();
        assert!(code.contains("result = 2 + 2"));
        assert!(code.contains("print(result)"));
    }

    #[test]
    fn test_parse_invalid() {
        let cmd = Command::parse("Just some regular text");
        assert!(cmd.is_invalid());
    }

    #[test]
    fn test_parse_final_with_whitespace() {
        let cmd = Command::parse("  FINAL   done  ");
        assert_eq!(cmd.as_final().unwrap(), "done");
    }

    #[test]
    fn test_parse_code_before_final() {
        // Code blocks are prioritized - we execute before allowing FINAL
        let input = r#"```repl
result = 15 * 23
print(result)
```

FINAL The answer is 345."#;

        let cmd = Command::parse(input);
        // Code should be executed first
        assert!(cmd.as_code().is_some());
        assert!(cmd.as_code().unwrap().contains("15 * 23"));
    }

    #[test]
    fn test_parse_final_alone() {
        // FINAL alone (no code) should work
        let input = "FINAL The answer is 345.";
        let cmd = Command::parse(input);
        assert_eq!(cmd.as_final().unwrap(), "The answer is 345.");
    }
}
