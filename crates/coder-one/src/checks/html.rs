//! A small HTML tokenizer for the sanitizer scenarios: enough of the
//! HTML tokenization rules to find what a browser would run, and to
//! compare two documents after the normalization an HTML parser may do.
//!
//! It isn't a conforming parser. It reads start and end tags with their
//! attributes, comments, doctypes, and text; it reads the contents of
//! `script`, `style`, and the other raw-text elements as text up to their
//! end tag; and it decodes the character references attribute values and
//! text usually carry. A `<` that doesn't start a tag is text, as it is
//! for a browser.

/// One token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// A start tag: its lowercased name and its attributes in order, each
    /// with a lowercased name and a decoded value.
    Start {
        name: String,
        attrs: Vec<(String, String)>,
    },
    /// An end tag's lowercased name.
    End(String),
    Text(String),
    Comment(String),
    Doctype(String),
}

/// Elements whose contents are text up to their end tag.
const RAW_TEXT: &[&str] = &[
    "script", "style", "textarea", "title", "xmp", "iframe", "noembed", "noframes",
];

/// Elements without an end tag.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Attributes whose value is a URL a browser may load or navigate to.
const URL_ATTRS: &[&str] = &[
    "href",
    "src",
    "action",
    "formaction",
    "data",
    "xlink:href",
    "background",
    "poster",
    "lowsrc",
    "dynsrc",
    "codebase",
];

/// Decodes the character references in `text`: numeric ones, with or
/// without the semicolon, and the named ones scripts are hidden behind.
#[must_use]
pub fn decode(text: &str) -> String {
    let named: &[(&str, &str)] = &[
        ("amp", "&"),
        ("lt", "<"),
        ("gt", ">"),
        ("quot", "\""),
        ("apos", "'"),
        ("colon", ":"),
        ("tab", "\t"),
        ("newline", "\n"),
        ("nbsp", "\u{a0}"),
        ("lpar", "("),
        ("rpar", ")"),
        ("sol", "/"),
        ("copy", "\u{a9}"),
    ];
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        if let Some(numeric) = after.strip_prefix('#') {
            let (hex, digits) = match numeric.strip_prefix(['x', 'X']) {
                Some(h) => (true, h),
                None => (false, numeric),
            };
            let len = digits
                .char_indices()
                .take_while(|(_, c)| {
                    if hex {
                        c.is_ascii_hexdigit()
                    } else {
                        c.is_ascii_digit()
                    }
                })
                .count();
            let value = u32::from_str_radix(&digits[..len], if hex { 16 } else { 10 }).ok();
            if len > 0
                && let Some(c) = value.and_then(char::from_u32)
            {
                out.push(c);
                let consumed = usize::from(hex) + len;
                let tail = &numeric[consumed..];
                rest = tail.strip_prefix(';').unwrap_or(tail);
                continue;
            }
        } else if let Some((name, value)) = named.iter().find(|(name, _)| {
            after
                .get(..name.len())
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) {
            out.push_str(value);
            let tail = &after[name.len()..];
            rest = tail.strip_prefix(';').unwrap_or(tail);
            continue;
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn name_char(c: char) -> bool {
    !c.is_whitespace() && c != '/' && c != '>'
}

/// Reads one tag's attributes from `s`, which starts after its name; returns
/// them and the bytes consumed through the closing `>`, or the whole rest.
fn attributes(s: &str) -> (Vec<(String, String)>, usize) {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut attrs = Vec::new();
    loop {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() {
            return (attrs, i);
        }
        if bytes[i] == b'>' {
            return (attrs, i + 1);
        }
        let start = i;
        // An attribute name may start with `=`; after that it ends at `=`.
        i += s[i..].chars().next().map_or(1, char::len_utf8);
        while i < bytes.len() {
            let c = s[i..].chars().next().unwrap_or(' ');
            if !name_char(c) || c == '=' {
                break;
            }
            i += c.len_utf8();
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i] as char;
                let end = s[i + 1..].find(quote).map_or(s.len(), |e| i + 1 + e);
                value = decode(&s[i + 1..end]);
                i = (end + 1).min(s.len());
            } else {
                let start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                value = decode(&s[start..i]);
            }
        }
        if !attrs.iter().any(|(n, _): &(String, String)| *n == name) {
            attrs.push((name, value));
        }
    }
}

/// The tokens of `html`.
#[must_use]
pub fn tokenize(html: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    let flush = |text: &mut String, tokens: &mut Vec<Token>| {
        if !text.is_empty() {
            tokens.push(Token::Text(decode(text)));
            text.clear();
        }
    };
    while i < html.len() {
        let rest = &html[i..];
        let Some(c) = rest.chars().next() else { break };
        if c != '<' {
            text.push(c);
            i += c.len_utf8();
            continue;
        }
        let next = rest[1..].chars().next();
        if let Some(body) = rest.strip_prefix("<!--") {
            flush(&mut text, &mut tokens);
            let end = body.find("-->").map_or(body.len(), |e| e);
            tokens.push(Token::Comment(body[..end].to_string()));
            i += 4 + (end + 3).min(body.len());
        } else if next == Some('!') || next == Some('?') {
            flush(&mut text, &mut tokens);
            let end = rest.find('>').map_or(rest.len(), |e| e + 1);
            let inner = rest[2..end.saturating_sub(1).max(2)].to_string();
            if inner.to_ascii_lowercase().starts_with("doctype") {
                tokens.push(Token::Doctype(inner.to_ascii_lowercase()));
            } else {
                tokens.push(Token::Comment(inner));
            }
            i += end;
        } else if next == Some('/') && rest[2..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            flush(&mut text, &mut tokens);
            let end = rest.find('>').map_or(rest.len(), |e| e + 1);
            let name: String = rest[2..]
                .chars()
                .take_while(|c| name_char(*c))
                .collect::<String>()
                .to_ascii_lowercase();
            tokens.push(Token::End(name));
            i += end;
        } else if next.is_some_and(|c| c.is_ascii_alphabetic()) {
            flush(&mut text, &mut tokens);
            let name: String = rest[1..].chars().take_while(|c| name_char(*c)).collect();
            let after = 1 + name.len();
            let (attrs, used) = attributes(&rest[after..]);
            let name = name.to_ascii_lowercase();
            i += after + used;
            let raw = RAW_TEXT.contains(&name.as_str());
            tokens.push(Token::Start {
                name: name.clone(),
                attrs,
            });
            if raw {
                let lower = html[i..].to_ascii_lowercase();
                let close = format!("</{name}");
                let end = lower.find(&close).unwrap_or(lower.len());
                if end > 0 {
                    tokens.push(Token::Text(html[i..i + end].to_string()));
                }
                i += end;
            }
        } else {
            text.push('<');
            i += 1;
        }
    }
    flush(&mut text, &mut tokens);
    tokens
}

/// What in `html` a browser would run: script elements, event-handler
/// attributes, script URLs, and script in styles. Empty when nothing.
#[must_use]
pub fn scripts(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    let tokens = tokenize(html);
    let mut in_style = false;
    for token in &tokens {
        match token {
            Token::Start { name, attrs } => {
                in_style = name == "style";
                if name == "script" {
                    found.push("a script element".to_string());
                }
                for (attr, value) in attrs {
                    let squashed: String = value
                        .chars()
                        .filter(|c| !c.is_whitespace() && !c.is_control())
                        .collect::<String>()
                        .to_ascii_lowercase();
                    if attr.len() > 2 && attr.starts_with("on") {
                        found.push(format!("an event handler, {attr}, on <{name}>"));
                    } else if URL_ATTRS.contains(&attr.as_str())
                        && ["javascript:", "vbscript:", "data:text/html"]
                            .iter()
                            .any(|s| squashed.starts_with(s))
                    {
                        found.push(format!("a script URL in {attr} on <{name}>"));
                    } else if (attr == "style" || (name == "meta" && attr == "content"))
                        && (squashed.contains("javascript:") || squashed.contains("expression("))
                    {
                        found.push(format!("script in {attr} on <{name}>"));
                    }
                }
            }
            Token::Text(text) if in_style => {
                let lower = text.to_ascii_lowercase();
                if lower.contains("javascript:") || lower.contains("expression(") {
                    found.push("script in a style element".to_string());
                }
            }
            Token::End(_) => in_style = false,
            _ => {}
        }
    }
    found
}

/// `html` after the normalization an HTML parser may do: tag and attribute
/// names lowercased, attributes sorted with decoded values, character
/// references decoded, runs of whitespace collapsed, whitespace-only text
/// dropped, end tags of void elements dropped, and the `html`, `head`,
/// `body`, and `tbody` tags a parser may add or drop left out.
#[must_use]
pub fn normalized(html: &str) -> Vec<String> {
    let implied = ["html", "head", "body", "tbody"];
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    tokenize(html)
        .into_iter()
        .filter_map(|token| match token {
            Token::Start { name, .. } if implied.contains(&name.as_str()) => None,
            Token::End(name)
                if implied.contains(&name.as_str()) || VOID.contains(&name.as_str()) =>
            {
                None
            }
            Token::Start { name, mut attrs } => {
                attrs.sort();
                let attrs: Vec<String> = attrs
                    .into_iter()
                    .map(|(n, v)| format!("{n}={:?}", squash(&v)))
                    .collect();
                Some(format!("<{name} {}>", attrs.join(" ")))
            }
            Token::End(name) => Some(format!("</{name}>")),
            Token::Text(text) => {
                let text = squash(&text);
                (!text.is_empty()).then_some(text)
            }
            Token::Comment(text) => Some(format!("<!--{}-->", squash(&text))),
            Token::Doctype(text) => Some(format!("<!{}>", squash(&text))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_are_found_where_a_browser_would_run_them() {
        assert!(!scripts("<p>Hello</p><script>alert(1)</script>").is_empty());
        assert!(!scripts("<img src=x onerror=alert(1)>").is_empty());
        assert!(!scripts("<a href=\"jav&#x09;ascript:alert(1)\">x</a>").is_empty());
        assert!(!scripts("<IMG SRC=JaVaScRiPt:alert(1)>").is_empty());
        assert!(!scripts("<div style=\"background:url(javascript:alert(1))\">x</div>").is_empty());
        assert!(
            scripts("<p>Write &lt;script&gt; in the head.</p><a href=\"/docs\">x</a>").is_empty()
        );
        assert!(scripts("<p>3 < 5 and onclick=x is text</p>").is_empty());
        // A tag named `scr<script` isn't a script element.
        assert!(scripts("<scr<script>ipt>alert(1)").is_empty());
    }

    #[test]
    fn normalization_forgives_what_a_parser_changes() {
        assert_eq!(
            normalized("<HTML><body><P CLASS='a' id=b>x  y</P><br/></body></HTML>"),
            normalized("<p id=\"b\" class=\"a\">x y</p><br>")
        );
        assert_ne!(
            normalized("<!-- <div>kept</div> --><p>x</p>"),
            normalized("<p>x</p>")
        );
        assert_eq!(decode("&#106;&#x61;&colon;&amp;"), "ja:&");
    }
}
