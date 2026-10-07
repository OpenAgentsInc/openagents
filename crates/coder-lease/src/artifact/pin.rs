//! Rewriting the source lines that hold an artifact's pin.

use super::Pin;

/// `current` with each pin line replaced by the line with the same prefix
/// that `output`, the regenerate command's standard output, printed. When
/// the history's line changed, its earlier quoted value goes into the
/// history, unless it is already there.
///
/// # Errors
/// A sentence when `output` or `current` lacks a pin line, or `current`
/// lacks the history's marker.
pub fn apply_pin(pin: &Pin, current: &str, output: &str) -> Result<String, String> {
    let mut lines: Vec<String> = current.split('\n').map(str::to_owned).collect();
    let mut moved = None;
    for prefix in &pin.lines {
        let printed = output
            .lines()
            .map(str::trim_end)
            .find(|line| line.starts_with(prefix.as_str()))
            .ok_or_else(|| format!("the regenerate command printed no line starting `{prefix}`"))?;
        let at = lines
            .iter()
            .position(|line| line.starts_with(prefix.as_str()))
            .ok_or_else(|| format!("{} has no line starting `{prefix}`", pin.file))?;
        if lines[at] != printed {
            if pin
                .history
                .as_ref()
                .is_some_and(|history| &history.from == prefix)
            {
                moved = quoted(&lines[at]);
            }
            printed.clone_into(&mut lines[at]);
        }
    }
    if let (Some(history), Some(old)) = (&pin.history, moved) {
        let after = lines
            .iter()
            .position(|line| line == &history.after)
            .ok_or_else(|| format!("{} has no line `{}`", pin.file, history.after))?;
        let end = lines[after + 1..]
            .iter()
            .position(|line| line.trim_start().starts_with(history.end.as_str()))
            .map_or(lines.len(), |offset| after + 1 + offset);
        let entry = format!("\"{old}\",");
        if !lines[after + 1..end]
            .iter()
            .any(|line| line.trim() == entry)
        {
            let indent: String = history
                .after
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            lines.insert(after + 1, format!("{indent}{entry}"));
        }
    }
    Ok(lines.join("\n"))
}

/// `change` with its pin lines and history put back as `base` has them, so
/// a submitted change carries no repin of its own.
#[must_use]
pub fn reset_pin(pin: &Pin, change: &str, base: &str) -> String {
    let base_lines: Vec<&str> = base.split('\n').collect();
    let mut lines: Vec<String> = change.split('\n').map(str::to_owned).collect();
    for prefix in &pin.lines {
        let from_base = base_lines
            .iter()
            .find(|line| line.starts_with(prefix.as_str()));
        let at = lines
            .iter()
            .position(|line| line.starts_with(prefix.as_str()));
        if let (Some(from_base), Some(at)) = (from_base, at) {
            (*from_base).clone_into(&mut lines[at]);
        }
    }
    if let Some(history) = &pin.history
        && let Some(base_block) = block(&base_lines, &history.after, &history.end)
    {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        if let Some(range) = block_range(&refs, &history.after, &history.end) {
            let replacement: Vec<String> = base_block.iter().map(|s| (*s).to_owned()).collect();
            lines.splice(range, replacement);
        }
    }
    lines.join("\n")
}

fn block<'a>(lines: &[&'a str], after: &str, end: &str) -> Option<Vec<&'a str>> {
    block_range(lines, after, end).map(|range| lines[range].to_vec())
}

fn block_range(lines: &[&str], after: &str, end: &str) -> Option<std::ops::Range<usize>> {
    let start = lines.iter().position(|line| *line == after)? + 1;
    let stop = lines[start..]
        .iter()
        .position(|line| line.trim_start().starts_with(end))?;
    Some(start..start + stop)
}

/// The text between the first and the last double quote of `line`.
fn quoted(line: &str) -> Option<String> {
    let first = line.find('"')?;
    let last = line.rfind('"')?;
    (last > first).then(|| line[first + 1..last].to_owned())
}
