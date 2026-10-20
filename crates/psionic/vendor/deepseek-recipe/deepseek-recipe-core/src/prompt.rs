//! Prompt text that records the byte ranges of explicit special tokens.
//!
//! A rendered prompt consists of two kinds of content:
//!
//! - **Explicit special tokens**: control tokens inserted through
//!   [`Prompt::push_special`] or a [`Special`] fragment, such as `<｜User｜>`.
//!   Token encoding encodes each one as the token ID of its spelling.
//! - **Plain text**: all other text, including template text and text taken
//!   from the conversation, such as message content, tool results, and tool
//!   names. Token encoding encodes plain text without the tokenizer's added
//!   vocabulary, so a special-token spelling in it encodes as the token IDs of
//!   its characters.
//!
//! [`Prompt`] keeps the two apart while the prompt is assembled. Token encoding
//! reads [`Prompt::special_ranges`].

use std::ops::{Add, AddAssign, Range};

/// Prompt text with the byte ranges of its explicit special tokens.
///
/// Build one with [`prompt!`](macro@crate::prompt),
/// [`push_plain`](Self::push_plain), or [`push_special`](Self::push_special).
/// Ranges are ascending and disjoint; adjacent special tokens keep separate
/// ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prompt {
    text: String,
    special_ranges: Vec<Range<usize>>,
}

impl Prompt {
    /// Create an empty prompt.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a prompt from one explicit special token.
    pub fn special(token: &str) -> Self {
        let mut prompt = Self::new();
        prompt.push_special(token);
        prompt
    }

    /// Create a prompt from plain text.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            special_ranges: Vec::new(),
        }
    }

    /// Append `token` as an explicit special token. An empty `token` appends
    /// nothing.
    pub fn push_special(&mut self, token: &str) {
        if token.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(token);
        self.special_ranges.push(start..self.text.len());
    }

    /// Append plain text.
    pub fn push_plain(&mut self, text: impl AsRef<str>) {
        self.text.push_str(text.as_ref());
    }

    /// Split at byte offset `at`, returning the suffix and keeping the prefix.
    /// A special token split by `at` becomes plain text in both parts.
    ///
    /// # Panics
    ///
    /// Panics if `at` is out of bounds or not on a UTF-8 character boundary.
    pub fn split_off(&mut self, at: usize) -> Self {
        if at == 0 {
            return std::mem::take(self);
        }
        let text = self.text.split_off(at);
        let mut special_ranges = self.special_ranges.split_off(
            self.special_ranges
                .partition_point(|range| range.start < at),
        );
        for range in &mut special_ranges {
            range.start -= at;
            range.end -= at;
        }
        if self
            .special_ranges
            .last()
            .is_some_and(|range| range.end > at)
        {
            self.special_ranges.pop();
        }
        Self {
            text,
            special_ranges,
        }
    }

    /// Return the prompt text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Return whether the prompt has no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Return the ascending, disjoint byte ranges of explicit special tokens.
    pub fn special_ranges(&self) -> &[Range<usize>] {
        &self.special_ranges
    }

    /// Consume the prompt, returning its text and special token ranges.
    pub fn into_parts(self) -> (String, Vec<Range<usize>>) {
        (self.text, self.special_ranges)
    }

    /// Join prompts with a plain separator, keeping each prompt's special
    /// tokens.
    pub fn join(items: &[Self], sep: &str) -> Self {
        let mut result = Self::new();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                result.push_plain(sep);
            }
            result += item;
        }
        result
    }
}

/// A special token passed to [`prompt!`](macro@crate::prompt).
///
/// Strings are plain text. Wrap a special token's spelling in this type to
/// insert it as an explicit special token.
pub struct Special<T>(pub T);

/// A value that [`prompt!`](macro@crate::prompt) can append to a [`Prompt`].
///
/// - `str` and `String` append plain text.
/// - [`Special`] appends an explicit special token.
/// - [`Prompt`] splices in with its own special tokens.
/// - `&T` dispatches to `T`.
pub trait PromptFragment {
    /// Append this value to `prompt`.
    fn push_into(&self, prompt: &mut Prompt);
}

impl PromptFragment for str {
    fn push_into(&self, prompt: &mut Prompt) {
        prompt.push_plain(self);
    }
}

impl PromptFragment for String {
    fn push_into(&self, prompt: &mut Prompt) {
        prompt.push_plain(self);
    }
}

impl<T: AsRef<str>> PromptFragment for Special<T> {
    fn push_into(&self, prompt: &mut Prompt) {
        prompt.push_special(self.0.as_ref());
    }
}

impl PromptFragment for Prompt {
    fn push_into(&self, prompt: &mut Prompt) {
        *prompt += self;
    }
}

impl<T: PromptFragment + ?Sized> PromptFragment for &T {
    fn push_into(&self, prompt: &mut Prompt) {
        (**self).push_into(prompt);
    }
}

impl AddAssign<&Self> for Prompt {
    fn add_assign(&mut self, rhs: &Self) {
        let offset = self.text.len();
        self.text.push_str(&rhs.text);
        self.special_ranges.extend(
            rhs.special_ranges
                .iter()
                .map(|range| range.start + offset..range.end + offset),
        );
    }
}

impl AddAssign<Self> for Prompt {
    fn add_assign(&mut self, rhs: Self) {
        *self += &rhs;
    }
}

impl Add<&Self> for Prompt {
    type Output = Self;

    fn add(mut self, rhs: &Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::fmt::Display for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::prompt;

    const USER: &str = "<｜User｜>";
    const ASSISTANT: &str = "<｜Assistant｜>";

    fn special_texts(prompt: &Prompt) -> Vec<&str> {
        prompt
            .special_ranges()
            .iter()
            .map(|range| &prompt.as_str()[range.clone()])
            .collect()
    }

    #[test]
    fn special_ranges_align_with_bytes() {
        let mut prompt = Prompt::special(USER);
        prompt.push_plain("你");
        prompt += Prompt::special(ASSISTANT) + &Prompt::plain("d");
        assert_eq!(prompt.as_str(), format!("{USER}你{ASSISTANT}d"));
        assert_eq!(
            prompt.special_ranges(),
            &[
                0..USER.len(),
                USER.len() + 3..USER.len() + 3 + ASSISTANT.len()
            ]
        );
    }

    #[test]
    fn split_off_preserves_complete_special_tokens() {
        let original = prompt!("{}你{}", Special(USER), Special(ASSISTANT));
        for (at, prefix, suffix) in [
            (0, Prompt::new(), original.clone()),
            (
                USER.len(),
                Prompt::special(USER),
                prompt!("你{}", Special(ASSISTANT)),
            ),
            (
                USER.len() + "你".len(),
                prompt!("{}你", Special(USER)),
                Prompt::special(ASSISTANT),
            ),
            (original.as_str().len(), original.clone(), Prompt::new()),
            (
                1,
                Prompt::plain("<"),
                prompt!("{}你{}", &USER[1..], Special(ASSISTANT)),
            ),
        ] {
            let mut prompt = original.clone();
            assert_eq!(prompt.split_off(at), suffix);
            assert_eq!(prompt, prefix);
        }
    }

    #[test]
    fn template_strings_are_plain() {
        const TAG: &str = "invoke";
        let key = "user_key";
        let value = "user_value".to_owned();
        let prompt = prompt!(
            "<{}{} name=\"{}\">{}</{}{}>",
            Special("dsml"),
            TAG,
            key,
            value,
            Special("dsml"),
            TAG
        );
        assert_eq!(
            prompt.as_str(),
            "<dsmlinvoke name=\"user_key\">user_value</dsmlinvoke>"
        );
        assert_eq!(special_texts(&prompt), ["dsml", "dsml"]);
    }

    #[test]
    fn literal_special_token_text_is_plain() {
        let content = USER;
        let user = Special(USER);
        let prompt = prompt!("{user}{content}<｜Assistant｜>");
        assert_eq!(prompt.as_str(), format!("{USER}{USER}{ASSISTANT}"));
        assert_eq!(special_texts(&prompt), [USER]);
    }

    #[test]
    fn template_literal_only_and_trailing_comma() {
        let prompt = prompt!("no placeholders");
        assert_eq!(prompt.as_str(), "no placeholders");
        assert!(prompt.special_ranges().is_empty());

        let prompt = prompt!("a{}c", "b",);
        assert_eq!(prompt.as_str(), "abc");
        assert!(prompt.special_ranges().is_empty());
    }

    #[test]
    fn template_splices_prompt_fragments() {
        let mut sub = Prompt::special(USER);
        sub.push_plain("mid");
        let prompt = prompt!("head {}{}", sub, &Prompt::special(ASSISTANT));
        assert_eq!(prompt.as_str(), format!("head {USER}mid{ASSISTANT}"));
        assert_eq!(special_texts(&prompt), [USER, ASSISTANT]);
    }

    #[test]
    fn template_captures_and_named_arguments() {
        let value = "user_value".to_owned();
        let prompt = prompt!("{}|{}", "head", arg = &value);
        assert_eq!(prompt.as_str(), "head|user_value");
        assert!(prompt.special_ranges().is_empty());

        let role = Special(USER);
        let prompt = prompt!("{role}{content}", content = &value);
        assert_eq!(prompt.as_str(), format!("{USER}user_value"));
        assert_eq!(special_texts(&prompt), [USER]);
    }

    #[test]
    fn template_evaluates_each_argument_once_in_order() {
        let calls = Cell::new(0);
        let next = || {
            calls.set(calls.get() + 1);
            calls.get().to_string()
        };
        // `{}` takes the positional argument; `a` and `b` are each used twice.
        let prompt = prompt!("{a}{a}{}{b}{b}", next(), a = next(), b = next());
        assert_eq!(prompt.as_str(), "22133");
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn template_escapes_braces() {
        let prompt = prompt!("{{\"tag\": {}}}", "x");
        assert_eq!(prompt.as_str(), "{\"tag\": x}");
        assert!(prompt.special_ranges().is_empty());
    }
}
