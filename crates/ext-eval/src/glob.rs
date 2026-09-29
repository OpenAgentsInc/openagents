//! The small glob language `--case` and `file_exists` use.
//!
//! `*` matches any run of characters except `/`, `**` matches any run
//! including `/`, `?` matches one character except `/`, and every other
//! character matches itself. There are no character classes and no escapes:
//! a case name and a created file's path are bounded, plain strings, and
//! this is deterministic matching over them, not routing.

/// Whether `text` matches `pattern` as a whole.
#[must_use]
pub fn matches(pattern: &str, text: &str) -> bool {
    let tokens = tokens(pattern);
    let text: Vec<char> = text.chars().collect();
    let mut memo = vec![None; (tokens.len() + 1) * (text.len() + 1)];
    step(&tokens, &text, 0, 0, &mut memo)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Token {
    /// `*`: any run of characters but `/`.
    Star,
    /// `**`: any run of characters.
    Any,
    /// `**/`: nothing, or any run of characters ending in `/`.
    AnyDirectory,
    /// `?`: one character but `/`.
    One,
    /// A character that matches itself.
    Literal(char),
}

fn tokens(pattern: &str) -> Vec<Token> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '*' if chars.get(index + 1) == Some(&'*') => {
                if chars.get(index + 2) == Some(&'/') {
                    out.push(Token::AnyDirectory);
                    index += 3;
                } else {
                    out.push(Token::Any);
                    index += 2;
                }
            }
            '*' => {
                out.push(Token::Star);
                index += 1;
            }
            '?' => {
                out.push(Token::One);
                index += 1;
            }
            literal => {
                out.push(Token::Literal(literal));
                index += 1;
            }
        }
    }
    out
}

fn step(tokens: &[Token], text: &[char], t: usize, j: usize, memo: &mut [Option<bool>]) -> bool {
    let slot = t * (text.len() + 1) + j;
    if let Some(known) = memo[slot] {
        return known;
    }
    let result = match tokens.get(t) {
        None => j == text.len(),
        Some(Token::Literal(c)) => text.get(j) == Some(c) && step(tokens, text, t + 1, j + 1, memo),
        Some(Token::One) => {
            text.get(j).is_some_and(|c| *c != '/') && step(tokens, text, t + 1, j + 1, memo)
        }
        Some(Token::Star) => {
            let mut end = j;
            loop {
                if step(tokens, text, t + 1, end, memo) {
                    break true;
                }
                if end == text.len() || text[end] == '/' {
                    break false;
                }
                end += 1;
            }
        }
        Some(Token::Any) => (j..=text.len()).any(|end| step(tokens, text, t + 1, end, memo)),
        Some(Token::AnyDirectory) => {
            step(tokens, text, t + 1, j, memo)
                || (j..text.len())
                    .filter(|end| text[*end] == '/')
                    .any(|end| step(tokens, text, t + 1, end + 1, memo))
        }
    };
    memo[slot] = Some(result);
    result
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn literals_and_single_characters() {
        assert!(matches("smoke", "smoke"));
        assert!(!matches("smoke", "smokes"));
        assert!(matches("sm?ke", "smoke"));
        assert!(!matches("sm?ke", "sm/ke"));
    }

    #[test]
    fn a_star_stays_inside_one_segment() {
        assert!(matches("find-*", "find-callers"));
        assert!(matches("*", ""));
        assert!(matches("*.md", "notes.md"));
        assert!(!matches("*.md", "docs/notes.md"));
        assert!(matches("docs/*.md", "docs/notes.md"));
    }

    #[test]
    fn a_double_star_crosses_segments() {
        assert!(matches("**/*.md", "docs/deep/notes.md"));
        assert!(matches("**/*.md", "notes.md"));
        assert!(matches("out/**", "out/a/b.txt"));
        assert!(!matches("out/**", "in/a.txt"));
        assert!(matches("**", "anything/at/all"));
    }
}
