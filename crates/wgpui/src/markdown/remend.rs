pub fn remend(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut result = text.to_string();

    result = complete_bold(&result);
    result = complete_strikethrough(&result);
    result = complete_italic(&result);
    result = complete_inline_code(&result);
    result = complete_links(&result);
    result = prevent_setext_heading(&result);

    result
}

fn complete_bold(text: &str) -> String {
    if is_within_fenced_code_block(text) {
        return text.to_string();
    }

    let pair_count = count_double_asterisks(text);

    if pair_count % 2 == 1
        && let Some(last_pos) = text.rfind("**")
    {
        let after = &text[last_pos + 2..];
        if !after.is_empty() && !is_escaped(text, last_pos) && has_content(after) {
            return format!("{}**", text);
        }
    }

    text.to_string()
}

fn complete_strikethrough(text: &str) -> String {
    if is_within_fenced_code_block(text) {
        return text.to_string();
    }

    let pair_count = count_double_tildes(text);

    if pair_count % 2 == 1
        && let Some(last_pos) = text.rfind("~~")
    {
        let after = &text[last_pos + 2..];
        if !after.is_empty() && !is_escaped(text, last_pos) && has_content(after) {
            return format!("{}~~", text);
        }
    }

    text.to_string()
}

fn complete_italic(text: &str) -> String {
    if is_within_fenced_code_block(text) {
        return text.to_string();
    }

    let single_count = count_single_asterisks(text);

    if single_count % 2 == 1
        && let Some(last_pos) = find_last_single_asterisk(text)
    {
        let after = &text[last_pos + 1..];

        if is_list_marker(text, last_pos) {
            return text.to_string();
        }

        if !after.is_empty() && !is_escaped(text, last_pos) && has_content(after) {
            return format!("{}*", text);
        }
    }

    text.to_string()
}

fn complete_inline_code(text: &str) -> String {
    if is_within_fenced_code_block(text) {
        return text.to_string();
    }

    let backtick_count = count_inline_backticks(text);

    if backtick_count % 2 == 1
        && let Some(last_pos) = find_last_inline_backtick(text)
    {
        let after = &text[last_pos + 1..];
        if !after.is_empty() && !is_escaped(text, last_pos) {
            return format!("{}`", text);
        }
    }

    text.to_string()
}

fn complete_links(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut bracket_start = None;
    let mut i = text.len();

    while i > 0 {
        i -= 1;
        if bytes[i] == b'[' && !is_escaped(text, i) {
            bracket_start = Some(i);
            break;
        }
    }

    if let Some(start) = bracket_start {
        let after_bracket = &text[start..];

        if let Some(paren_pos) = after_bracket.find("](") {
            let after_paren = &after_bracket[paren_pos + 2..];
            if !after_paren.contains(')') && !after_paren.is_empty() {
                let before = &text[..start];
                let link_text = &after_bracket[1..paren_pos];
                return format!("{}[{}](incomplete:link)", before, link_text);
            }
        }
    }

    text.to_string()
}

fn prevent_setext_heading(text: &str) -> String {
    if let Some(last_newline) = text.rfind('\n') {
        let last_line = text[last_newline + 1..].trim();
        if !last_line.is_empty()
            && (last_line.chars().all(|c| c == '-') || last_line.chars().all(|c| c == '='))
        {
            return format!("{}\u{200B}", text);
        }
    } else {
        let trimmed = text.trim();
        if !trimmed.is_empty()
            && (trimmed.chars().all(|c| c == '-') || trimmed.chars().all(|c| c == '='))
        {
            return format!("{}\u{200B}", text);
        }
    }
    text.to_string()
}

fn is_within_fenced_code_block(text: &str) -> bool {
    let fence_count = text.matches("```").count();
    fence_count % 2 == 1
}

fn is_escaped(text: &str, pos: usize) -> bool {
    if pos == 0 {
        return false;
    }
    let bytes = text.as_bytes();
    bytes.get(pos - 1) == Some(&b'\\')
}

fn has_content(text: &str) -> bool {
    text.chars().any(|c| !c.is_whitespace())
}

fn count_double_asterisks(text: &str) -> usize {
    let mut count = 0;
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len().saturating_sub(1) {
        if bytes[i] == b'*' && bytes[i + 1] == b'*' {
            if !is_escaped(text, i) {
                count += 1;
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    count
}

fn count_double_tildes(text: &str) -> usize {
    let mut count = 0;
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len().saturating_sub(1) {
        if bytes[i] == b'~' && bytes[i + 1] == b'~' {
            if !is_escaped(text, i) {
                count += 1;
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    count
}

fn count_single_asterisks(text: &str) -> usize {
    let mut count = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();

    for i in 0..len {
        if bytes[i] != b'*' {
            continue;
        }

        let prev_is_asterisk = i > 0 && bytes[i - 1] == b'*';
        let next_is_asterisk = i + 1 < len && bytes[i + 1] == b'*';

        if prev_is_asterisk || next_is_asterisk {
            continue;
        }

        if !is_escaped(text, i) {
            count += 1;
        }
    }

    count
}

fn find_last_single_asterisk(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();

    for i in (0..len).rev() {
        if bytes[i] != b'*' {
            continue;
        }

        let prev_is_asterisk = i > 0 && bytes[i - 1] == b'*';
        let next_is_asterisk = i + 1 < len && bytes[i + 1] == b'*';

        if !prev_is_asterisk && !next_is_asterisk {
            return Some(i);
        }
    }

    None
}

fn is_list_marker(text: &str, pos: usize) -> bool {
    let line_start = text[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let before_asterisk = &text[line_start..pos];

    if !before_asterisk.chars().all(|c| c.is_whitespace()) {
        return false;
    }

    let bytes = text.as_bytes();
    if pos + 1 < bytes.len() && bytes[pos + 1] == b' ' {
        return true;
    }

    false
}

fn count_inline_backticks(text: &str) -> usize {
    let mut count = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }

        if i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            i += 3;
            continue;
        }

        if i >= 2 && bytes[i - 1] == b'`' && bytes[i - 2] == b'`' {
            i += 1;
            continue;
        }

        if !is_escaped(text, i) {
            count += 1;
        }
        i += 1;
    }

    count
}

fn find_last_inline_backtick(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();

    for i in (0..len).rev() {
        if bytes[i] != b'`' {
            continue;
        }

        let is_fence = (i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`')
            || (i >= 2 && bytes[i - 1] == b'`' && bytes[i - 2] == b'`')
            || (i >= 1 && i + 1 < len && bytes[i - 1] == b'`' && bytes[i + 1] == b'`');

        if !is_fence && !is_escaped(text, i) {
            return Some(i);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_bold() {
        assert_eq!(remend("This is **bold"), "This is **bold**");
        assert_eq!(remend("**bold** text"), "**bold** text");
        assert_eq!(remend("**bold** and **more"), "**bold** and **more**");
    }

    #[test]
    fn test_complete_italic() {
        assert_eq!(remend("This is *italic"), "This is *italic*");
        assert_eq!(remend("*italic* text"), "*italic* text");
    }

    #[test]
    fn test_complete_inline_code() {
        assert_eq!(remend("Check `code"), "Check `code`");
        assert_eq!(remend("`code` works"), "`code` works");
    }

    #[test]
    fn test_complete_strikethrough() {
        assert_eq!(remend("This is ~~struck"), "This is ~~struck~~");
        assert_eq!(remend("~~struck~~ out"), "~~struck~~ out");
    }

    #[test]
    fn test_code_block_preservation() {
        assert_eq!(remend("```\n**bold\n"), "```\n**bold\n");
        assert_eq!(
            remend("```rust\nlet x = *ptr;\n"),
            "```rust\nlet x = *ptr;\n"
        );
    }

    #[test]
    fn test_escaped_markers() {
        assert_eq!(remend(r"This is \**not bold"), r"This is \**not bold");
        assert_eq!(remend(r"This is \*not italic"), r"This is \*not italic");
    }

    #[test]
    fn test_list_markers() {
        assert_eq!(remend("* item one"), "* item one");
        assert_eq!(remend("  * nested item"), "  * nested item");
    }

    #[test]
    fn test_streaming_simulation() {
        let mut accumulated = String::new();

        accumulated.push_str("Here is");
        assert_eq!(remend(&accumulated), "Here is");

        accumulated.push_str(" a **bold");
        assert_eq!(remend(&accumulated), "Here is a **bold**");

        accumulated.push_str(" statement**");
        assert_eq!(remend(&accumulated), "Here is a **bold statement**");
    }

    #[test]
    fn test_mixed_formatting() {
        assert_eq!(remend("**bold** and *italic"), "**bold** and *italic*");
        assert_eq!(remend("*italic* and **bold"), "*italic* and **bold**");
    }

    #[test]
    fn test_incomplete_link() {
        assert_eq!(
            remend("Check [this](http://example"),
            "Check [this](incomplete:link)"
        );
    }

    #[test]
    fn test_setext_heading_prevention() {
        let result = remend("Some text\n---");
        assert!(result.ends_with('\u{200B}'));
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(remend(""), "");
    }

    #[test]
    fn test_no_markers() {
        assert_eq!(remend("Plain text here"), "Plain text here");
    }
}
