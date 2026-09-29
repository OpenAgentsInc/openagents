//! A step's `reply` read while the model is still writing the step.
//!
//! A provider streams a step's next action as JSON text: Claude Code as the
//! `StructuredOutput` tool's `input_json_delta`s, Codex as output text
//! deltas. [`Tap`] reads that text as it arrives and decodes the top-level
//! `reply` member so far, so a host can show the user the reply's first
//! words before the model has written the rest of the action. [`settled`]
//! says how much of a reply still being written can be shown without
//! splitting a paragraph or a code fence.
//!
//! Nothing here decides anything: the parsed action at the end of the call
//! is still the step. A host that shows a streamed prefix must check that
//! the final reply begins with it.

/// Reads the top-level `reply` string of a next action from its JSON text,
/// fed in pieces as a provider streams it.
///
/// Only the outermost object's `reply` member is read; strings in nested
/// values and every other member are skipped. Text that is not a JSON
/// object leaves the reply empty. Escapes, including `\u` surrogate pairs,
/// are decoded exactly as a JSON parser decodes them.
#[derive(Clone, Debug, Default)]
pub struct Tap {
    /// Open objects and arrays outside strings.
    depth: usize,
    /// Inside a string.
    string: bool,
    /// The last character was a backslash inside a string.
    escaped: bool,
    /// `\u` digits read so far and their value.
    unicode: Option<(u8, u32)>,
    /// A high surrogate waiting for its low half.
    high: Option<u32>,
    /// At the outermost level, a `:` was read since the last `,`: the next
    /// string is a value, not a key.
    value: bool,
    /// The outermost key being read, or last read.
    key: String,
    /// Whether the current string is the outermost key.
    reading_key: bool,
    /// Whether the current string is the outermost `reply` value.
    reading_reply: bool,
    reply: String,
    /// The `reply` string has closed.
    complete: bool,
}

impl Tap {
    /// A tap that has read nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads `text`, the next piece of the action's JSON. Returns whether
    /// the reply grew or closed.
    pub fn feed(&mut self, text: &str) -> bool {
        let (before, was_complete) = (self.reply.len(), self.complete);
        for c in text.chars() {
            self.char(c);
        }
        self.reply.len() != before || self.complete != was_complete
    }

    /// The reply decoded so far.
    #[must_use]
    pub fn reply(&self) -> &str {
        &self.reply
    }

    /// Whether the reply's closing quote has been read, so [`Tap::reply`]
    /// is the whole reply.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.complete
    }

    fn char(&mut self, c: char) {
        if !self.string {
            match c {
                '"' => {
                    self.string = true;
                    let outer = self.depth == 1;
                    self.reading_key = outer && !self.value;
                    self.reading_reply =
                        outer && self.value && self.key == "reply" && !self.complete;
                    if self.reading_key {
                        self.key.clear();
                    }
                }
                '{' | '[' => self.depth += 1,
                '}' | ']' => self.depth = self.depth.saturating_sub(1),
                ':' if self.depth == 1 => self.value = true,
                ',' if self.depth == 1 => self.value = false,
                _ => {}
            }
            return;
        }
        if let Some((digits, value)) = self.unicode {
            let Some(digit) = c.to_digit(16) else {
                // Not a JSON string a parser accepts; stop reading it.
                self.unicode = None;
                return;
            };
            let value = value * 16 + digit;
            if digits + 1 < 4 {
                self.unicode = Some((digits + 1, value));
                return;
            }
            self.unicode = None;
            let decoded = match (self.high.take(), value) {
                (None, 0xD800..=0xDBFF) => {
                    self.high = Some(value);
                    return;
                }
                (Some(high), 0xDC00..=0xDFFF) => {
                    char::from_u32(0x10000 + ((high - 0xD800) << 10) + (value - 0xDC00))
                }
                (_, value) => char::from_u32(value),
            };
            self.push(decoded.unwrap_or(char::REPLACEMENT_CHARACTER));
            return;
        }
        if self.escaped {
            self.escaped = false;
            let decoded = match c {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                'b' => '\u{8}',
                'f' => '\u{c}',
                'u' => {
                    self.unicode = Some((0, 0));
                    return;
                }
                other => other,
            };
            self.high = None;
            self.push(decoded);
            return;
        }
        match c {
            '\\' => self.escaped = true,
            '"' => {
                self.string = false;
                self.high = None;
                if self.reading_reply {
                    self.complete = true;
                }
                self.reading_key = false;
                self.reading_reply = false;
            }
            other => {
                self.high = None;
                self.push(other);
            }
        }
    }

    fn push(&mut self, c: char) {
        if self.reading_key {
            self.key.push(c);
        } else if self.reading_reply {
            self.reply.push(c);
        }
    }
}

/// How much of `reply`, a reply still being written, can be shown now:
/// the end of its last whole paragraph (a blank line outside a code fence)
/// after byte `shown`, or `shown` when no paragraph after it has ended.
/// A `complete` reply is shown whole.
#[must_use]
pub fn settled(reply: &str, shown: usize, complete: bool) -> usize {
    if complete {
        return reply.len();
    }
    let mut end = shown.min(reply.len());
    let mut fenced = false;
    let mut at = 0;
    for line in reply.split_inclusive('\n') {
        let next = at + line.len();
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        // A blank line that ends: the paragraph before it is whole.
        if line.trim().is_empty()
            && line.ends_with('\n')
            && !fenced
            && next > end
            && !reply[end.min(at)..at].trim().is_empty()
        {
            end = next;
        }
        at = next;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tapped(pieces: &[&str]) -> Tap {
        let mut tap = Tap::new();
        for piece in pieces {
            tap.feed(piece);
        }
        tap
    }

    #[test]
    fn the_reply_is_decoded_as_it_streams_and_matches_the_parsed_action() {
        let json = r#"{"reply": "Hello \"there\"\n\nTab\there, é and 😀 done", "ask":"none", "rationale": "reply is \"not\" this", "commands": ["echo \"reply\""], "finished": true}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let expected = parsed["reply"].as_str().unwrap();
        // Every split point, including inside escapes and surrogate pairs.
        for split in 0..json.len() {
            if !json.is_char_boundary(split) {
                continue;
            }
            let tap = tapped(&[&json[..split], &json[split..]]);
            assert_eq!(tap.reply(), expected, "split at {split}");
            assert!(tap.complete());
        }
        // One character at a time, the reply only grows, as a prefix.
        let mut tap = Tap::new();
        let mut last = String::new();
        for c in json.chars() {
            tap.feed(&c.to_string());
            assert!(tap.reply().starts_with(&last));
            last = tap.reply().to_owned();
        }
        assert_eq!(last, expected);
    }

    #[test]
    fn only_the_outermost_reply_is_read() {
        let tap = tapped(&[
            r#"{"rationale":"x","nested":{"reply":"no"},"list":["reply",{"reply":"no"}],"reply":"yes","after":"reply"}"#,
        ]);
        assert_eq!(tap.reply(), "yes");
        assert!(tap.complete());
        let tap = tapped(&[r#"{"rationale":"say \"reply\": no","reply""#, r#": "ye"#]);
        assert_eq!(tap.reply(), "ye");
        assert!(!tap.complete());
        assert_eq!(tapped(&[r#"["reply","x"]"#]).reply(), "");
        assert_eq!(tapped(&[r#"{"replying":"x"}"#]).reply(), "");
    }

    #[test]
    fn a_paragraph_shows_when_it_ends_and_never_inside_a_fence() {
        assert_eq!(settled("One line", 0, false), 0);
        assert_eq!(settled("One line", 0, true), 8);
        let text = "First para.\n\nSecond";
        assert_eq!(settled(text, 0, false), "First para.\n\n".len());
        assert_eq!(settled(text, "First para.\n\n".len(), false), 13);
        let fenced = "Look:\n\n```\na\n\nb\n```\n\nAfter";
        let closed = "Look:\n\n```\na\n\nb\n```\n\n".len();
        assert_eq!(settled(fenced, 0, false), closed);
        assert_eq!(settled(fenced, "Look:\n\n".len(), false), closed);
        // A blank line inside an open fence ends nothing.
        let open = "Look:\n\n```\na\n\nb";
        assert_eq!(settled(open, 0, false), "Look:\n\n".len());
        assert_eq!(settled(open, "Look:\n\n".len(), false), "Look:\n\n".len());
        // Blank lines alone are not a paragraph.
        assert_eq!(settled("\n\n\n\nx", 0, false), 0);
    }
}
