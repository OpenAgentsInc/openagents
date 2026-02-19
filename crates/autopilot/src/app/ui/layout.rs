pub(crate) fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.len() <= max_chars {
            lines.push(line.to_string());
        } else {
            let mut current_line = String::new();
            for word in line.split_whitespace() {
                if current_line.is_empty() {
                    if word.len() > max_chars {
                        for chunk in word.as_bytes().chunks(max_chars) {
                            lines.push(String::from_utf8_lossy(chunk).to_string());
                        }
                    } else {
                        current_line = word.to_string();
                    }
                } else if current_line.len() + 1 + word.len() <= max_chars {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    if word.len() > max_chars {
                        for chunk in word.as_bytes().chunks(max_chars) {
                            lines.push(String::from_utf8_lossy(chunk).to_string());
                        }
                        current_line = String::new();
                    } else {
                        current_line = word.to_string();
                    }
                }
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(crate) fn split_into_words_for_layout(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = 0;
    let mut in_word = false;

    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if in_word {
                in_word = false;
            }
        } else if !in_word && start < i {
            words.push(&text[start..i]);
            start = i;
            in_word = true;
        } else {
            in_word = true;
        }
    }

    if start < text.len() {
        words.push(&text[start..]);
    }

    words
}
