//! Vim text objects

/// Text objects that can be selected with i/a prefix
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    /// Word (iw/aw)
    Word {
        /// If true, use WORD (whitespace-delimited)
        ignore_punctuation: bool,
    },
    /// Sentence (is/as)
    Sentence,
    /// Paragraph (ip/ap)
    Paragraph,

    // Quote objects
    /// Single quotes (i'/a')
    SingleQuotes,
    /// Double quotes (i"/a")
    DoubleQuotes,
    /// Backticks (i`/a`)
    BackQuotes,

    // Bracket objects
    /// Parentheses (i(/a( or ib/ab)
    Parentheses,
    /// Square brackets (i[/a[)
    SquareBrackets,
    /// Curly braces (i{/a{ or iB/aB)
    CurlyBrackets,
    /// Angle brackets (i</a<)
    AngleBrackets,

    // Special objects
    /// HTML/XML tag (it/at)
    Tag,
    /// Function argument (ia/aa) - requires treesitter or similar
    Argument,
    /// Entire buffer (ie/ae)
    EntireBuffer,
    /// Current line (il/al)
    Line,
}

impl Object {
    /// Get the opening delimiter for bracket/quote objects
    pub fn open_delimiter(&self) -> Option<char> {
        match self {
            Object::SingleQuotes => Some('\''),
            Object::DoubleQuotes => Some('"'),
            Object::BackQuotes => Some('`'),
            Object::Parentheses => Some('('),
            Object::SquareBrackets => Some('['),
            Object::CurlyBrackets => Some('{'),
            Object::AngleBrackets => Some('<'),
            _ => None,
        }
    }

    /// Get the closing delimiter for bracket/quote objects
    pub fn close_delimiter(&self) -> Option<char> {
        match self {
            Object::SingleQuotes => Some('\''),
            Object::DoubleQuotes => Some('"'),
            Object::BackQuotes => Some('`'),
            Object::Parentheses => Some(')'),
            Object::SquareBrackets => Some(']'),
            Object::CurlyBrackets => Some('}'),
            Object::AngleBrackets => Some('>'),
            _ => None,
        }
    }

    /// Check if this object uses matching delimiters
    pub fn is_paired(&self) -> bool {
        self.open_delimiter().is_some()
    }

    /// Create object from character input (for i/a prefix)
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'w' => Some(Object::Word {
                ignore_punctuation: false,
            }),
            'W' => Some(Object::Word {
                ignore_punctuation: true,
            }),
            's' => Some(Object::Sentence),
            'p' => Some(Object::Paragraph),
            '\'' => Some(Object::SingleQuotes),
            '"' => Some(Object::DoubleQuotes),
            '`' => Some(Object::BackQuotes),
            '(' | ')' | 'b' => Some(Object::Parentheses),
            '[' | ']' => Some(Object::SquareBrackets),
            '{' | '}' | 'B' => Some(Object::CurlyBrackets),
            '<' | '>' => Some(Object::AngleBrackets),
            't' => Some(Object::Tag),
            'a' => Some(Object::Argument),
            'e' => Some(Object::EntireBuffer),
            'l' => Some(Object::Line),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_from_char() {
        assert_eq!(
            Object::from_char('w'),
            Some(Object::Word {
                ignore_punctuation: false
            })
        );
        assert_eq!(Object::from_char('('), Some(Object::Parentheses));
        assert_eq!(Object::from_char('b'), Some(Object::Parentheses));
        assert_eq!(Object::from_char('"'), Some(Object::DoubleQuotes));
        assert_eq!(Object::from_char('z'), None);
    }

    #[test]
    fn test_delimiters() {
        assert_eq!(Object::Parentheses.open_delimiter(), Some('('));
        assert_eq!(Object::Parentheses.close_delimiter(), Some(')'));
        assert_eq!(Object::DoubleQuotes.open_delimiter(), Some('"'));
        assert_eq!(Object::DoubleQuotes.close_delimiter(), Some('"'));
    }
}
