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
    fn find_final(input: &str) -> Option<String> {
        // Look for "FINAL" at the start of a line
        for line in input.lines() {
            let line = line.trim();
            if line.starts_with("FINAL") {
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
            for line in after_code.lines() {
                let line = line.trim();
                if line.starts_with("FINAL") {
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

    /// Extract code from a code block (```repl, ```python, or just ```).
    fn extract_code_block(input: &str) -> Option<String> {
        // Try multiple code block markers
        let markers = ["```repl", "```python", "```py", "```"];

        for start_marker in markers {
            if let Some(start_idx) = input.find(start_marker) {
                let code_start = start_idx + start_marker.len();
                let remaining = &input[code_start..];

                // Find the closing ```
                if let Some(end_idx) = remaining.find("```") {
                    let code = remaining[..end_idx].trim();
                    if !code.is_empty() {
                        return Some(code.to_string());
                    }
                }
            }
        }

        None
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
