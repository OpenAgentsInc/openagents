use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

const MAX_FILE_BYTES: usize = 200_000;
const MAX_COMMAND_BYTES: usize = 120_000;

pub(crate) fn expand_prompt_text(prompt: &str, cwd: &PathBuf) -> Result<String, String> {
    let with_commands = expand_command_lines(prompt, cwd)?;
    expand_file_references(&with_commands, cwd)
}

fn expand_command_lines(prompt: &str, cwd: &PathBuf) -> Result<String, String> {
    let mut output = String::new();
    for line in prompt.lines() {
        let trimmed = line.trim_start();
        if let Some(command) = trimmed.strip_prefix('!') {
            let command = command.trim();
            if command.is_empty() {
                output.push_str(line);
                output.push('\n');
                continue;
            }
            let command_output = run_shell_command(command, cwd)?;
            output.push_str("--- BEGIN COMMAND: ");
            output.push_str(command);
            output.push_str(" ---\n");
            output.push_str(&command_output);
            if !command_output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("--- END COMMAND ---\n");
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    Ok(output.trim_end_matches('\n').to_string())
}

fn run_shell_command(command: &str, cwd: &PathBuf) -> Result<String, String> {
    let output = ProcessCommand::new("bash")
        .arg("-lc")
        .arg(command)
        .current_dir(cwd)
        .output()
        .map_err(|err| format!("Failed to run command '{}': {}", command, err))?;

    let mut combined = String::new();
    if !output.stdout.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    if combined.is_empty() {
        combined.push_str("(command produced no output)");
    }

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        combined = format!("(exit code {})\n{}", code, combined);
    }

    Ok(truncate_bytes(combined, MAX_COMMAND_BYTES))
}

fn expand_file_references(prompt: &str, cwd: &PathBuf) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = prompt.chars().peekable();
    let mut last_was_space = true;

    while let Some(ch) = chars.next() {
        if ch == '@' && last_was_space {
            let mut token = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() {
                    break;
                }
                token.push(next);
                chars.next();
            }

            if token.is_empty() {
                output.push('@');
                last_was_space = false;
                continue;
            }

            let (path_token, trailing) = split_trailing_punct(&token);
            let path = cwd.join(&path_token);
            if !path.is_file() {
                return Err(format!("File not found: {}", path_token));
            }
            let contents = read_file_limited(&path)
                .map_err(|err| format!("Failed to read {}: {}", path_token, err))?;
            output.push_str("\n\n--- BEGIN FILE: ");
            output.push_str(&path_token);
            output.push_str(" ---\n");
            output.push_str(&contents);
            if !contents.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("--- END FILE ---\n\n");
            output.push_str(&trailing);
            last_was_space = true;
            continue;
        }

        output.push(ch);
        last_was_space = ch.is_whitespace();
    }

    Ok(output)
}

fn split_trailing_punct(token: &str) -> (String, String) {
    let mut path = token.to_string();
    let mut trailing = String::new();
    loop {
        let last = path.chars().last();
        match last {
            Some(ch) if matches!(ch, ',' | '.' | ':' | ';' | ')' | ']' | '}') => {
                path.pop();
                trailing.insert(0, ch);
            }
            _ => break,
        }
    }
    (path, trailing)
}

fn read_file_limited(path: &Path) -> io::Result<String> {
    let file = fs::File::open(path)?;
    let file_len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let mut buffer = Vec::new();
    let mut handle = file.take(MAX_FILE_BYTES as u64);
    handle.read_to_end(&mut buffer)?;
    let mut text = String::from_utf8_lossy(&buffer).to_string();
    if file_len as usize > MAX_FILE_BYTES {
        text.push_str("\n... [truncated]");
    }
    Ok(text)
}

fn build_context_sections(label: &str, path: &Path, contents: &str) -> String {
    format!(
        "--- BEGIN {}: {} ---\n{}\n--- END {} ---",
        label,
        path.display(),
        contents,
        label
    )
}

fn candidate_agent_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    paths.push(cwd.join("AGENTS.md"));
    paths.push(cwd.join(".openagents").join("AGENTS.md"));
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".openagents").join("AGENTS.md"));
    }
    paths
}

fn candidate_todo_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    paths.push(cwd.join("TODO.md"));
    paths.push(cwd.join("todo.md"));
    paths.push(cwd.join(".openagents").join("TODO.md"));
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".openagents").join("TODO.md"));
    }
    paths
}

pub(crate) fn build_context_injection(cwd: &Path) -> Option<String> {
    let mut sections = Vec::new();
    for path in candidate_agent_paths(cwd) {
        if path.is_file() {
            if let Ok(contents) = read_file_limited(&path) {
                if !contents.trim().is_empty() {
                    sections.push(build_context_sections("AGENTS.md", &path, &contents));
                }
            }
        }
    }
    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

pub(crate) fn build_todo_context(cwd: &Path) -> Option<String> {
    let mut sections = Vec::new();
    for path in candidate_todo_paths(cwd) {
        if path.is_file() {
            if let Ok(contents) = read_file_limited(&path) {
                if !contents.trim().is_empty() {
                    sections.push(build_context_sections("TODO", &path, &contents));
                }
            }
        }
    }
    if sections.is_empty() {
        Some("No TODO.md found. Track tasks explicitly before finishing.".to_string())
    } else {
        Some(sections.join("\n\n"))
    }
}

fn truncate_bytes(input: String, max_bytes: usize) -> String {
    super::super::truncate_bytes(input, max_bytes)
}
