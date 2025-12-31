use std::path::Path;

pub fn shorten_path(path: &Path) -> String {
    let path_str = path.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if path_str.starts_with(&home) {
            return path_str.replacen(&home, "~", 1);
        }
    }
    path_str
}

pub fn sanitize_text(text: &str) -> String {
    text.replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "")
}

pub fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let text = sanitize_text(text);
    if text.chars().count() <= max_chars {
        return vec![text];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let indent = "  ";

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let current_len = current_line.chars().count();
        let is_continuation = !lines.is_empty();
        let effective_max = if is_continuation {
            max_chars - 2
        } else {
            max_chars
        };

        if word_len > effective_max {
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }

            let mut chars = word.chars().peekable();
            while chars.peek().is_some() {
                let is_cont = !lines.is_empty();
                let max = if is_cont { max_chars - 2 } else { max_chars };
                let chunk: String = chars.by_ref().take(max).collect();
                if is_cont {
                    lines.push(format!("{}{}", indent, chunk));
                } else {
                    lines.push(chunk);
                }
            }
            continue;
        }

        let space_needed = if current_line.is_empty() { 0 } else { 1 };
        if current_len + space_needed + word_len <= effective_max {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        } else {
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        if lines.is_empty() {
            lines.push(current_line);
        } else {
            lines.push(format!("{}{}", indent, current_line));
        }
    }

    for i in 1..lines.len() {
        if !lines[i].starts_with(indent) {
            lines[i] = format!("{}{}", indent, lines[i]);
        }
    }

    lines
}
