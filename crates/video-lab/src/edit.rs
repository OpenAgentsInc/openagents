//! Changing a document: the edit pipeline of `lab.py tool_edit` and
//! `commit`, the times the chat passes, and voiceover caption pacing.

use lab_kit::Refusal;
use serde_json::{Value, json};

use crate::check::{Checked, check, plain};
use crate::fmt::clock;
use crate::measure::Measured;
use crate::patch::{apply_patch, operations};

fn refuse(message: impl Into<String>) -> Refusal {
    Refusal::new("video", message)
}

/// Python's `round(x, n)`: the correctly rounded decimal of the binary value.
pub fn round(x: f64, n: usize) -> f64 {
    format!("{x:.n$}").parse().unwrap_or(x)
}

/// `(name, start, end)` per shot.
pub fn timeline(data: &Value) -> Vec<(String, f64, f64)> {
    let mut t = 0.0;
    data["shots"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let dur = s.get("dur").and_then(Value::as_f64).unwrap_or(0.0);
            let row = (
                s["name"].as_str().unwrap_or_default().to_owned(),
                t,
                t + dur,
            );
            t += dur;
            row
        })
        .collect()
}

pub fn length(kind: &str, data: &Value) -> f64 {
    match kind {
        "cut.v1" => data["shots"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| s["dur"].as_f64().unwrap_or(0.0))
            .sum(),
        "scene.v1" => data["shot"]["duration"].as_f64().unwrap_or(0.0),
        _ => data.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
    }
}

/// Captions move with the shot they're under when shots change length.
pub fn ripple(old: &Value, new: &mut Value) -> usize {
    let before = timeline(old);
    let after = timeline(new);
    let mut moved = 0;
    if let Some(caps) = new.get_mut("captions").and_then(Value::as_array_mut) {
        for c in caps {
            let at = c["at"].as_f64().unwrap_or(0.0);
            let Some(under) = before
                .iter()
                .find(|(_, s, e)| s - 1e-6 <= at && at < e - 1e-6)
            else {
                continue;
            };
            let Some(now) = after.iter().find(|(n, ..)| n == &under.0) else {
                continue;
            };
            let shift = now.1 - under.1;
            if shift.abs() > 1e-6 {
                c["at"] = json!(round(at + shift, 3));
                moved += 1;
            }
        }
    }
    moved
}

/// When shots are removed, captions over them go too (unless the patch
/// handles captions itself) and the score stops marking them.
pub fn drop_removed(old: &Value, new: &mut Value, touched_caps: bool) -> Vec<String> {
    let names: Vec<String> = new["shots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let gone: Vec<(String, f64, f64)> = timeline(old)
        .into_iter()
        .filter(|(n, ..)| !names.contains(n))
        .collect();
    if gone.is_empty() {
        return Vec::new();
    }
    let mut said = Vec::new();
    if !touched_caps
        && let Some(caps) = new
            .get("captions")
            .and_then(Value::as_array)
            .filter(|c| !c.is_empty())
            .cloned()
    {
        let keep: Vec<Value> = caps
            .iter()
            .filter(|c| {
                let at = c["at"].as_f64().unwrap_or(0.0);
                !gone.iter().any(|(_, s, e)| s - 1e-6 <= at && at < e - 1e-6)
            })
            .cloned()
            .collect();
        if keep.len() != caps.len() {
            said.push(format!(
                "{} captions over the removed shots went with them.",
                caps.len() - keep.len()
            ));
            new["captions"] = Value::Array(keep);
        }
    }
    if let Some(score) = new
        .get_mut("score")
        .and_then(Value::as_object_mut)
        .filter(|s| !s.is_empty())
    {
        let gone_names: Vec<&str> = gone.iter().map(|(n, ..)| n.as_str()).collect();
        let before = score.get("cuts_at").cloned();
        let cuts: Vec<Value> = before
            .as_ref()
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|n| !n.as_str().is_some_and(|n| gone_names.contains(&n)))
            .cloned()
            .collect();
        if Some(Value::Array(cuts.clone())) != before {
            score.insert("cuts_at".into(), Value::Array(cuts));
            said.push("The music no longer marks the removed shots.".into());
        }
        if score
            .get("end_at")
            .and_then(Value::as_str)
            .is_some_and(|e| gone_names.contains(&e))
        {
            score.shift_remove("end_at");
        }
    }
    said
}

/// What an accepted edit reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edited {
    pub content: Value,
    /// Lines after "Saved vN: summary": what moved or went, what got fixed,
    /// what is still open, and the new length.
    pub said: Vec<String>,
    pub notes: Vec<String>,
}

/// The first half of `lab_edit`, which needs no measurements: patch, then
/// keep captions with their shots and drop what went with removed shots.
pub fn prepare(
    kind: &str,
    old: &Value,
    patch: &Value,
    summary: &str,
    ripple_captions: bool,
) -> Result<(Value, Vec<String>), Refusal> {
    if summary.trim().is_empty() {
        return Err(refuse(
            "an edit needs a one-line summary of what it changes, in plain words",
        ));
    }
    let ops = operations(patch)?;
    let mut new = apply_patch(old, &Value::Array(ops.clone()))?;
    if &new == old {
        return Err(refuse("the patch changes nothing"));
    }
    let touched_caps = ops.iter().any(|op| {
        op.get("path")
            .and_then(Value::as_str)
            .is_some_and(|p| p.starts_with("/captions"))
    });
    let mut said = Vec::new();
    if kind == "cut.v1" {
        said.extend(drop_removed(old, &mut new, touched_caps));
        if ripple_captions && !touched_caps {
            if old
                .get("timeline")
                .and_then(|t| t.get("links"))
                .is_some_and(|l| !l.is_null())
            {
                new = crate::timeline::request(
                    &json!({"action": "follow", "document": old, "command": new}),
                )
                .map_err(refuse)?;
            } else {
                let moved = ripple(old, &mut new);
                if moved > 0 {
                    said.push(format!("{moved} captions moved with their shots."));
                }
            }
        }
    }
    Ok((new, said))
}

/// The whole of `lab_edit`, when one set of measurements covers both documents.
pub fn edit(
    kind: &str,
    old: &Value,
    patch: &Value,
    summary: &str,
    ripple_captions: bool,
    m: &Measured,
) -> Result<Edited, Refusal> {
    let (new, said) = prepare(kind, old, patch, summary, ripple_captions)?;
    commit(kind, old, new, said, m, m)
}

/// `lab.py commit`'s decision: compare the checks before and after.
pub fn commit(
    kind: &str,
    old: &Value,
    new: Value,
    mut said: Vec<String>,
    m_old: &Measured,
    m_new: &Measured,
) -> Result<Edited, Refusal> {
    let Checked {
        problems: before, ..
    } = check(kind, old, m_old);
    let Checked {
        problems: after,
        notes,
    } = check(kind, &new, m_new);
    let added: Vec<String> = after
        .iter()
        .filter(|p| !before.contains(p))
        .cloned()
        .collect();
    let shape = |p: &str| -> String {
        let mut out = String::new();
        let mut in_num = false;
        for c in p.chars() {
            if c.is_ascii_digit() || c == '.' {
                if !in_num {
                    out.push('#');
                    in_num = true;
                }
            } else {
                in_num = false;
                out.push(c);
            }
        }
        out
    };
    let after_shapes: Vec<String> = after.iter().map(|q| shape(q)).collect();
    let fixed: Vec<String> = before
        .iter()
        .filter(|p| !after_shapes.contains(&shape(p)))
        .cloned()
        .collect();
    if !added.is_empty() {
        let mut why = String::from("Not saved. ");
        why.push_str(
            &plain(&added)
                .iter()
                .map(|p| {
                    if p.ends_with('.') {
                        p.clone()
                    } else {
                        format!("{p}.")
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
        if !fixed.is_empty() {
            why.push_str(&format!(" (It would have fixed: {}.)", fixed.join("; ")));
        }
        return Err(Refusal::new("check", why));
    }
    if !fixed.is_empty() {
        said.push(format!("Fixed: {}", fixed.join("; ")));
    }
    if !after.is_empty() {
        said.push(format!("Still open from before: {}", after.join("; ")));
    }
    if new
        .get("shots")
        .is_some_and(|s| s.as_array().is_some_and(|a| !a.is_empty()))
        && old
            .get("shots")
            .is_some_and(|s| s.as_array().is_some_and(|a| !a.is_empty()))
    {
        said.push(format!(
            "Length {} -> {}.",
            clock(length("cut.v1", old)),
            clock(length("cut.v1", &new))
        ));
    }
    if new
        .get("score")
        .is_some_and(|s| s.as_object().is_some_and(|o| !o.is_empty()))
        && length(kind, &new) != length(kind, old)
    {
        said.push("The music is remade to the new length; its hits stay on their shots.".into());
    }
    Ok(Edited {
        content: new,
        said,
        notes,
    })
}

/// A time from what the chat passed: seconds, `m:ss`, `h:mm:ss`, or a shot's
/// name (its start).
pub fn seconds(at: &Value, data: Option<&Value>) -> Result<f64, Refusal> {
    if let Some(x) = at.as_f64().filter(|_| at.is_number()) {
        return Ok(x);
    }
    let Some(text) = at.as_str().filter(|t| !t.trim().is_empty()) else {
        return Err(refuse("say a time, like 1:26 or 86"));
    };
    let s = text.trim().trim_end_matches('s');
    let parts: Vec<&str> = s.split(':').collect();
    let whole = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let decimal = |p: &str| {
        let (a, b) = p.split_once('.').unwrap_or((p, "0"));
        whole(a) && (b.is_empty() || whole(b)) && !p.ends_with('.')
    };
    if (parts.len() == 2 || parts.len() == 3)
        && parts[..parts.len() - 1].iter().all(|p| whole(p))
        && decimal(parts[parts.len() - 1])
    {
        let n: Vec<f64> = parts.iter().map(|p| p.parse().unwrap_or(0.0)).collect();
        return Ok(if n.len() == 3 {
            n[0] * 3600.0 + n[1] * 60.0 + n[2]
        } else {
            n[0] * 60.0 + n[1]
        });
    }
    if let Ok(x) = s.parse::<f64>()
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
    {
        return Ok(x);
    }
    if let Some(data) = data
        && let Some((_, start, _)) = timeline(data).into_iter().find(|(n, ..)| n == text.trim())
    {
        return Ok(start);
    }
    Err(refuse(format!(
        "'{text}' isn't a time; say 1:26, 86, or a shot's name"
    )))
}

/// About how many words a line takes to say: numbers take longer to say
/// than to write ("284" is "two hundred eighty-four").
pub fn spoken_words(text: &str) -> f64 {
    text.split_whitespace()
        .map(|tok| {
            let digits: String = tok.chars().filter(char::is_ascii_digit).collect();
            if digits.is_empty() {
                1.0 + 0.5 * tok.matches('/').count() as f64
            } else {
                let significant = digits.trim_start_matches('0');
                let n = if significant.is_empty() {
                    1
                } else {
                    significant.chars().count()
                };
                1.0 + n.min(4) as f64 * 0.6
            }
        })
        .sum()
}

/// A voiceover passage as caption-sized pieces: sentences, a long one split
/// at its last comma, semicolon or colon that fits.
pub fn vo_chunks(text: &str, max_chars: usize) -> Vec<(String, bool)> {
    let normal = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut sentences = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = normal.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == ' ' && i > 0 && matches!(chars[i - 1], '.' | '?' | '!') {
            sentences.push(std::mem::take(&mut cur));
        } else {
            cur.push(*c);
        }
    }
    sentences.push(cur);
    let mut out = Vec::new();
    for sent in sentences {
        let mut rest: Vec<char> = sent.chars().collect();
        while rest.len() > max_chars {
            let window = &rest[..(max_chars + 1).min(rest.len())];
            let mut cut = 0;
            for i in 0..window.len().saturating_sub(1) {
                if matches!(window[i], ',' | ';' | ':') && window[i + 1].is_whitespace() {
                    cut = i + 2;
                }
            }
            if cut == 0 {
                cut = rest[..max_chars]
                    .iter()
                    .rposition(|c| *c == ' ')
                    .map(|i| i + 1)
                    .unwrap_or(max_chars);
            }
            let piece: String = rest[..cut].iter().collect();
            out.push((piece.trim().to_owned(), false));
            rest = rest[cut..]
                .iter()
                .collect::<String>()
                .trim()
                .chars()
                .collect();
        }
        if !rest.is_empty() {
            out.push((rest.into_iter().collect(), true));
        }
    }
    out
}

pub fn vo_captions(text: &str, start: f64, wpm: f64) -> Vec<Value> {
    let mut t = start;
    vo_chunks(text, 92)
        .into_iter()
        .map(|(piece, ends)| {
            let dur = (spoken_words(&piece) * 60.0 / wpm).max(1.2);
            let cap = json!({"text": piece, "at": round(t, 3), "dur": round(dur, 3), "vo": true});
            t += dur + if ends { 0.3 } else { 0.12 };
            cap
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_like_the_lab() {
        let cut = json!({"shots": [{"name": "s1", "dur": 4}, {"name": "s2", "dur": 6}]});
        for (at, want) in [
            (json!(86), 86.0),
            (json!("1:26"), 86.0),
            (json!("1:26.5"), 86.5),
            (json!("0:01:26"), 86.0),
            (json!("86s"), 86.0),
            (json!("12.5"), 12.5),
            (json!("s2"), 4.0),
        ] {
            assert_eq!(seconds(&at, Some(&cut)).unwrap(), want, "{at}");
        }
        assert_eq!(
            seconds(&json!("soon"), Some(&cut)).unwrap_err().message,
            "'soon' isn't a time; say 1:26, 86, or a shot's name"
        );
        assert_eq!(
            seconds(&json!(""), None).unwrap_err().message,
            "say a time, like 1:26 or 86"
        );
    }

    #[test]
    fn captions_ripple_with_their_shots() {
        let old = json!({"shots": [{"name": "a", "dur": 4}, {"name": "b", "dur": 4}], "captions": [{"text": "x", "at": 5.0, "dur": 1}]});
        let mut new = json!({"shots": [{"name": "a", "dur": 6}, {"name": "b", "dur": 4}], "captions": [{"text": "x", "at": 5.0, "dur": 1}]});
        assert_eq!(ripple(&old, &mut new), 1);
        assert_eq!(new["captions"][0]["at"], 7.0);
    }

    #[test]
    fn removed_shots_take_their_captions_and_music_hits() {
        let old = json!({"shots": [{"name": "a", "dur": 4}, {"name": "b", "dur": 4}], "captions": [{"text": "x", "at": 5.0, "dur": 1}],
            "score": {"cuts_at": ["b"], "end_at": "b"}});
        let mut new = json!({"shots": [{"name": "a", "dur": 4}], "captions": [{"text": "x", "at": 5.0, "dur": 1}], "score": {"cuts_at": ["b"], "end_at": "b"}});
        let said = drop_removed(&old, &mut new, false);
        assert_eq!(
            said,
            [
                "1 captions over the removed shots went with them.",
                "The music no longer marks the removed shots."
            ]
        );
        assert_eq!(new["captions"], json!([]));
        assert!(new["score"].get("end_at").is_none());
    }

    #[test]
    fn voiceover_lines_pace_like_the_lab() {
        assert!((spoken_words("It costs $4.00 for 284 people") - 9.6).abs() < 1e-9);
        let caps = vo_captions(
            "Coder 1.0 is out today. It runs every harness you already use, side by side, and it keeps going.",
            2.0,
            150.0,
        );
        assert_eq!(caps[0]["text"], "Coder 1.0 is out today.");
        assert_eq!(caps[0]["at"], 2.0);
        assert!(caps[1]["at"].as_f64().unwrap() > 2.0);
    }
}
