use std::fs;
use std::future::Future;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: usize = 200_000;
pub(crate) const MAX_COMMAND_BYTES: usize = 120_000;

pub(crate) async fn expand_prompt_text_async<F, Fut>(
    prompt: &str,
    cwd: &PathBuf,
    run_command: F,
) -> Result<String, String>
where
    F: Fn(String, PathBuf) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let with_commands = expand_command_lines_async(prompt, cwd, run_command).await?;
    expand_file_references(&with_commands, cwd)
}

async fn expand_command_lines_async<F, Fut>(
    prompt: &str,
    cwd: &PathBuf,
    run_command: F,
) -> Result<String, String>
where
    F: Fn(String, PathBuf) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
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
            let command_output = run_command(command.to_string(), cwd.clone()).await?;
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

pub(crate) fn format_command_output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = String::new();
    if !stdout.is_empty() {
        combined.push_str(&String::from_utf8_lossy(stdout));
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(stderr));
    }

    if combined.is_empty() {
        combined.push_str("(command produced no output)");
    }

    if exit_code != 0 {
        combined = format!("(exit code {})\n{}", exit_code, combined);
    }

    combined
}

pub(crate) fn truncate_bytes(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    text.truncate(max_bytes);
    text.push_str("\n... [truncated]");
    text
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
