//! Everything the terminal shows is ASCII: program output, model answers,
//! and its own frame. Other characters map to an ASCII equivalent or `?`.

/// One character as printable ASCII. Control characters other than a tab
/// become nothing; a tab becomes a space.
#[must_use]
pub fn char_ascii(c: char) -> &'static str {
    const PRINTABLE: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
    if (' '..='~').contains(&c) {
        let at = c as usize - 32;
        return &PRINTABLE[at..=at];
    }
    match c {
        '\t' => " ",
        '\u{00a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{3000}' => " ",
        '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{2032}' | '\u{00b4}' => "'",
        '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{2033}' | '\u{00ab}' | '\u{00bb}' => "\"",
        '\u{2010}'..='\u{2015}'
        | '\u{2212}'
        | '\u{2500}'
        | '\u{2501}'
        | '\u{2504}'
        | '\u{2505}'
        | '\u{2508}'
        | '\u{2509}'
        | '\u{254c}'
        | '\u{254d}'
        | '\u{2550}' => "-",
        '\u{2502}' | '\u{2503}' | '\u{2506}' | '\u{2507}' | '\u{250a}' | '\u{250b}'
        | '\u{254e}' | '\u{254f}' | '\u{2551}' | '\u{2595}' | '\u{258f}' => "|",
        '\u{250c}'..='\u{254b}' | '\u{2552}'..='\u{256c}' | '\u{256d}'..='\u{2570}' => "+",
        '\u{2571}' => "/",
        '\u{2572}' => "\\",
        '\u{2573}' | '\u{00d7}' | '\u{2715}' | '\u{2716}' | '\u{2717}' | '\u{2718}' => "x",
        '\u{2026}' => "...",
        '\u{2022}' | '\u{2023}' | '\u{2043}' | '\u{25aa}' | '\u{25cf}' | '\u{00b7}'
        | '\u{2219}' | '\u{25e6}' | '\u{25cb}' => "*",
        '\u{2190}' => "<-",
        '\u{2192}' | '\u{279c}' | '\u{27a4}' | '\u{25b6}' | '\u{25b8}' | '\u{276f}' => "->",
        '\u{2191}' => "^",
        '\u{2193}' => "v",
        '\u{21d2}' => "=>",
        '\u{2264}' => "<=",
        '\u{2265}' => ">=",
        '\u{2260}' => "!=",
        '\u{2248}' => "~",
        '\u{2713}' | '\u{2714}' | '\u{2705}' => "ok",
        '\u{00a9}' => "(c)",
        '\u{00ae}' => "(r)",
        '\u{2122}' => "(tm)",
        '\u{00b0}' => "deg",
        '\u{2588}'
        | '\u{2589}'..='\u{258e}'
        | '\u{2590}'
        | '\u{2580}'..='\u{2587}'
        | '\u{2591}'..='\u{2593}' => "#",
        '\u{00e0}'..='\u{00e5}' => "a",
        '\u{00c0}'..='\u{00c5}' => "A",
        '\u{00e8}'..='\u{00eb}' => "e",
        '\u{00c8}'..='\u{00cb}' => "E",
        '\u{00ec}'..='\u{00ef}' => "i",
        '\u{00cc}'..='\u{00cf}' => "I",
        '\u{00f2}'..='\u{00f6}' | '\u{00f8}' => "o",
        '\u{00d2}'..='\u{00d6}' | '\u{00d8}' => "O",
        '\u{00f9}'..='\u{00fc}' => "u",
        '\u{00d9}'..='\u{00dc}' => "U",
        '\u{00e7}' => "c",
        '\u{00c7}' => "C",
        '\u{00f1}' => "n",
        '\u{00d1}' => "N",
        '\u{00df}' => "ss",
        // Zero-width marks and joiners, and variation selectors, add nothing.
        '\u{200b}'..='\u{200f}'
        | '\u{2060}'
        | '\u{fe00}'..='\u{fe0f}'
        | '\u{0300}'..='\u{036f}' => "",
        c if c.is_control() => "",
        _ => "?",
    }
}

/// `text` with every character mapped to ASCII; newlines are kept.
#[must_use]
pub fn ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\n' {
            out.push('\n');
        } else {
            out.push_str(char_ascii(c));
        }
    }
    out
}

/// A model answer as plain ASCII prose: Markdown markers are removed,
/// links keep their address in parentheses, and code keeps its lines,
/// indented by two spaces.
#[must_use]
pub fn plain(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            out.push(format!("  {}", ascii(line)));
            continue;
        }
        let indent = line.len() - trimmed.len();
        let mut rest = trimmed;
        // Headings and quotes lose their markers.
        rest = rest.trim_start_matches('#').trim_start();
        while let Some(stripped) = rest.strip_prefix('>') {
            rest = stripped.trim_start();
        }
        // List markers become an indent.
        let mut lead = " ".repeat(indent.min(8));
        if let Some(item) = rest
            .strip_prefix("- ")
            .or_else(|| rest.strip_prefix("* "))
            .or_else(|| rest.strip_prefix("+ "))
        {
            lead.push_str("  ");
            rest = item;
        } else if let Some(dot) = rest.find(". ")
            && dot > 0
            && dot <= 3
            && rest[..dot].chars().all(|c| c.is_ascii_digit())
        {
            lead.push_str(&rest[..=dot]);
            lead.push(' ');
            rest = &rest[dot + 2..];
        }
        if rest
            .chars()
            .all(|c| matches!(c, '-' | '*' | '_' | '=' | ' '))
            && rest.len() >= 3
        {
            out.push(String::new());
            continue;
        }
        out.push(format!("{lead}{}", inline(rest)));
    }
    // At most one blank line between paragraphs, none at the ends.
    let mut text = String::new();
    let mut blank = true;
    for line in out {
        let line = line.trim_end().to_owned();
        if line.is_empty() {
            if !blank {
                text.push('\n');
            }
            blank = true;
        } else {
            text.push_str(&line);
            text.push('\n');
            blank = false;
        }
    }
    text.trim_end().to_owned()
}

/// Inline Markdown: emphasis and code markers go; `[text](url)` becomes
/// `text (url)`.
fn inline(line: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '[' {
            if let Some(close) = chars[i..].iter().position(|&c| c == ']').map(|p| p + i)
                && chars.get(close + 1) == Some(&'(')
                && let Some(end) = chars[close..]
                    .iter()
                    .position(|&c| c == ')')
                    .map(|p| p + close)
            {
                let label: String = chars[i + 1..close].iter().collect();
                let url: String = chars[close + 2..end].iter().collect();
                out.push_str(&inline(&label));
                if url != label {
                    out.push_str(" (");
                    out.push_str(&ascii(&url));
                    out.push(')');
                }
                i = end + 1;
                continue;
            }
        }
        if c == '`' {
            i += 1;
            continue;
        }
        if (c == '*' || c == '_')
            && (chars.get(i + 1) == Some(&c)
                || i == 0
                || !chars[i - 1].is_alphanumeric()
                || chars.get(i + 1).is_none_or(|n| !n.is_alphanumeric()))
        {
            // Emphasis markers, not an underscore inside a word.
            let inside_word = c == '_'
                && i > 0
                && chars[i - 1].is_alphanumeric()
                && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
            if !inside_word {
                i += 1;
                continue;
            }
        }
        out.push_str(char_ascii(c));
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_and_model_text_becomes_printable_ascii() {
        let text =
            ascii("café \u{2014} “quoted” \u{2026} \u{2714} done \u{1f680}\u{fe0f} │ ├─ 日本");
        assert!(text.chars().all(|c| c.is_ascii() && !c.is_control()));
        assert_eq!(text, "cafe - \"quoted\" ... ok done ? | +- ??");
        assert_eq!(ascii("a\tb\x07c\nd"), "a bc\nd");
    }

    #[test]
    fn markdown_answers_become_plain_prose() {
        let answer = "## Why it failed\n\nThe test **asserts** `2 + 2 == 5`, which is false.\n\n- Fix the *expected* value\n- See [the book](https://doc.rust-lang.org/book/)\n\n```rust\nassert_eq!(2 + 2, 4);\n```\n\n---\n1. Run `cargo test` again.";
        let text = plain(answer);
        assert_eq!(
            text,
            "Why it failed\n\nThe test asserts 2 + 2 == 5, which is false.\n\n  Fix the expected value\n  See the book (https://doc.rust-lang.org/book/)\n\n  assert_eq!(2 + 2, 4);\n\n1. Run cargo test again."
        );
        for marker in ["**", "`", "##", "](", "- "] {
            assert!(!text.contains(marker), "{marker}");
        }
        assert_eq!(plain("snake_case_name stays"), "snake_case_name stays");
    }
}
