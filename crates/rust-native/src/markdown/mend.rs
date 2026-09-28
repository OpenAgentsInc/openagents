//! Display-only repair of the half-written syntax at a streaming tail.
//!
//! While a reply streams, `**bold` parses as literal text until the closing
//! `**` arrives; then the markers vanish and the line reflows. Closing the
//! open markers in a copy of the last block keeps the styling steady from the
//! first styled character. The repairs:
//!
//! - Close open strong, emphasis, and strikethrough, innermost first, and
//!   complete a half-written closer (`**a*` becomes `**a**`).
//! - Close an open inline code span.
//! - Show `[text](partial` as a link with an empty destination, so the text
//!   is styled and the unfinished destination never shows.
//! - Drop a trailing marker that has nothing after it yet (`Hello **`).
//! - In an unclosed fenced code block, drop a last line that is a partial
//!   closing fence, so the fence characters don't flash as code.
//!
//! The scan is deliberately approximate rather than a second CommonMark
//! delimiter algorithm: a misjudged repair lasts only until the next append,
//! and the canonical blocks never contain it. It reads only the last
//! top-level block, so its cost is bounded by the tail.

/// An open emphasis-family delimiter run.
struct Delim {
    mark: char,
    count: usize,
    /// Byte offset of the run's unmatched part.
    at: usize,
}

/// Mend `text`, the source of the last top-level block, for display.
/// `code` says whether the canonical last block is a code block. Returns
/// `None` when nothing needs repair.
pub(super) fn tail(text: &str, code: bool) -> Option<String> {
    let mut fence: Option<(u8, usize)> = None;
    // Where the open paragraph starts: after the last blank line or fence.
    let mut region = 0;
    let mut pos = 0;
    let mut last_line = 0;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        last_line = pos;
        match fence {
            Some((mark, len)) => {
                if let Some((m, n, rest)) = fence_run(strip_quotes(content))
                    && m == mark
                    && n >= len
                    && rest.trim().is_empty()
                {
                    fence = None;
                    region = pos + line.len();
                }
            }
            None => {
                let (body, _, _) = strip_markers(content);
                if let Some((m, n, rest)) = fence_run(body)
                    && n >= 3
                    && !(m == b'`' && rest.contains('`'))
                {
                    fence = Some((m, n));
                } else if content.trim().is_empty() {
                    region = pos + line.len();
                }
            }
        }
        pos += line.len();
    }
    if let Some((mark, _)) = fence {
        let last = &text[last_line..];
        let body = strip_quotes(last).trim();
        let partial =
            !last.ends_with(['\n', '\r']) && !body.is_empty() && body.bytes().all(|b| b == mark);
        return partial.then(|| text[..last_line].to_owned());
    }
    if code {
        return None;
    }
    inline(text, region)
}

/// A fence character, its run length, and the rest of the line, after up to
/// three spaces of indentation.
fn fence_run(line: &str) -> Option<(u8, usize, &str)> {
    let line = line.trim_start_matches(' ');
    let mark = *line.as_bytes().first()?;
    if mark != b'`' && mark != b'~' {
        return None;
    }
    let n = line.bytes().take_while(|&b| b == mark).count();
    Some((mark, n, &line[n..]))
}

fn strip_quotes(line: &str) -> &str {
    line.trim_start_matches([' ', '\t', '>'])
}

/// Strip the block markers at the start of a line: indentation, quote
/// markers, list markers, and a heading marker. Also report whether the line
/// starts a list item or is a heading, since inline markers can't cross into
/// a new block.
fn strip_markers(line: &str) -> (&str, bool, bool) {
    let mut rest = line;
    let mut item = false;
    loop {
        rest = rest.trim_start_matches([' ', '\t']);
        let bytes = rest.as_bytes();
        let spaced = |at: usize| bytes.get(at).is_none_or(|b| b" \t\r\n".contains(b));
        match bytes.first() {
            Some(b'>') => rest = &rest[1..],
            Some(b'-' | b'+' | b'*') if spaced(1) => {
                rest = &rest[1..];
                item = true;
            }
            Some(b'0'..=b'9') => {
                let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
                if digits <= 9
                    && matches!(bytes.get(digits), Some(b'.' | b')'))
                    && spaced(digits + 1)
                {
                    rest = &rest[digits + 1..];
                    item = true;
                } else {
                    return (rest, item, false);
                }
            }
            Some(b'#') => {
                let hashes = bytes.iter().take_while(|&&b| b == b'#').count();
                if hashes <= 6 && spaced(hashes) {
                    return (&rest[hashes..], item, true);
                }
                return (rest, item, false);
            }
            _ => return (rest, item, false),
        }
    }
}

/// Close the inline markers left open in the paragraph at `text[region..]`.
fn inline(text: &str, region: usize) -> Option<String> {
    // The paragraph's characters with their byte offsets in `text`, block
    // markers removed, and the indexes where a new block starts.
    let mut chars: Vec<(usize, char)> = Vec::new();
    let mut resets: Vec<usize> = Vec::new();
    let mut pos = region;
    for line in text[region..].split_inclusive('\n') {
        let (body, item, heading) = strip_markers(line);
        if item || heading {
            resets.push(chars.len());
        }
        let start = pos + line.len() - body.len();
        chars.extend(body.char_indices().map(|(at, c)| (start + at, c)));
        if heading {
            resets.push(chars.len());
        }
        pos += line.len();
    }
    if chars.iter().all(|(_, c)| c.is_whitespace()) {
        return None;
    }

    let mut stack: Vec<Delim> = Vec::new();
    // An open code span: its backtick run length and offset.
    let mut code: Option<(usize, usize)> = None;
    // Open `[`s, each with the delimiter stack depth where it opened.
    let mut brackets: Vec<usize> = Vec::new();
    // A link destination still streaming: the offset of its `(` and paren
    // depth.
    let mut dest: Option<(usize, usize)> = None;
    // A trailing marker with nothing after it.
    let mut dangling: Option<usize> = None;
    let run = |i: usize| chars[i..].iter().take_while(|x| x.1 == chars[i].1).count();

    let mut next_reset = 0;
    let mut i = 0;
    while i < chars.len() {
        while next_reset < resets.len() && resets[next_reset] <= i {
            next_reset += 1;
            stack.clear();
            brackets.clear();
            code = None;
            dest = None;
            dangling = None;
        }
        let c = chars[i].1;
        if let Some((_, depth)) = &mut dest {
            match c {
                '\\' => i += 1,
                '(' => *depth += 1,
                ')' if *depth == 0 => dest = None,
                ')' => *depth -= 1,
                _ => {}
            }
            i += 1;
            continue;
        }
        if let Some((open, _)) = code {
            if c == '`' {
                let k = run(i);
                if k == open {
                    code = None;
                }
                i += k;
            } else {
                i += 1;
            }
            continue;
        }
        match c {
            '\\' => i += 2,
            '`' => {
                let k = run(i);
                code = Some((k, chars[i].0));
                i += k;
            }
            '*' | '_' | '~' => {
                let k = run(i);
                let prev = i.checked_sub(1).map(|j| chars[j].1);
                let after = chars.get(i + k).map(|x| x.1);
                let mut open = after.is_some_and(|n| !n.is_whitespace());
                let mut close = prev.is_some_and(|p| !p.is_whitespace());
                if c == '_' && open && close {
                    // An intraword underscore is literal.
                    (open, close) = (false, false);
                }
                let mut left = k;
                if close {
                    while left > 0 {
                        let Some(j) = stack.iter().rposition(|d| d.mark == c) else {
                            break;
                        };
                        stack.truncate(j + 1);
                        let top = &mut stack[j];
                        if top.count <= left {
                            left -= top.count;
                            stack.pop();
                        } else {
                            top.count -= left;
                            left = 0;
                        }
                    }
                }
                if left > 0 && open {
                    stack.push(Delim {
                        mark: c,
                        count: left,
                        at: chars[i + k - left].0,
                    });
                } else if left == k && chars[i + k..].iter().all(|x| x.1.is_whitespace()) {
                    dangling = Some(chars[i].0);
                }
                i += k;
            }
            '[' => {
                brackets.push(stack.len());
                i += 1;
            }
            ']' => {
                if let Some(depth) = brackets.pop()
                    && chars.get(i + 1).is_some_and(|x| x.1 == '(')
                {
                    // Emphasis opened inside the link text and never closed
                    // there stays literal.
                    stack.truncate(depth);
                    dest = Some((chars[i + 1].0, 0));
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    let mut end = text.len();
    let mut closers = String::new();
    let mut link = false;
    if let Some((paren, _)) = dest {
        end = paren;
        link = true;
    } else if let Some((open, at)) = code {
        if text[at + open..].trim().is_empty() {
            end = at;
        } else {
            closers.push_str(&"`".repeat(open));
        }
    } else if let Some(at) = dangling {
        end = at;
    }
    for delim in stack.iter().rev() {
        let from = delim.at + delim.count;
        if from < end && !text[from..end].trim().is_empty() {
            closers.extend(std::iter::repeat_n(delim.mark, delim.count));
        }
    }
    if end == text.len() && closers.is_empty() && !link {
        return None;
    }
    let mut out = text[..end].to_owned();
    if link {
        out.push_str("()");
    }
    if !closers.is_empty() {
        // A closer after whitespace can't close.
        out.truncate(out.trim_end().len());
        out.push_str(&closers);
    }
    Some(out)
}
