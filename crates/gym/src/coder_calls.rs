//! Coder One's per-call accounting and episode deadline, as an attempt's
//! evidence records them: the `ledger` in `evaluation/usage.json` and the
//! `deadline` in the episode manifest.
//!
//! Every call carries one of three charges: `priced`, with a cost and its
//! provenance; `zero`, a known zero, such as a request never sent; or
//! `unknown`, a call that may have done billed work nobody reported. The
//! views keep the three apart and never show an unknown as zero.

use serde_json::Value;

const DASH: &str = "—";

fn usd(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| DASH.to_owned(), |usd| format!("${usd:.4}"))
}

fn seconds(value: &Value) -> String {
    value.as_u64().map_or_else(
        || DASH.to_owned(),
        |ms| format!("{:.1}s", ms as f64 / 1000.0),
    )
}

/// How many calls carry each charge: priced, zero, and unknown.
pub fn charges(ledger: &[Value]) -> (usize, usize, usize) {
    let count = |charge: &str| {
        ledger
            .iter()
            .filter(|row| row["charge"].as_str() == Some(charge))
            .count()
    };
    (count("priced"), count("zero"), count("unknown"))
}

/// The lines the attempt view shows: cost by call with provenance, the
/// unknown charges, and the deadline's use.
pub fn lines(ledger: &[Value], deadline: Option<&Value>) -> Vec<String> {
    let mut lines = Vec::new();
    if !ledger.is_empty() {
        let (priced, zero, unknown) = charges(ledger);
        lines.push(format!(
            "Calls: {} · {priced} priced · {zero} known zero · {unknown} unknown charge",
            ledger.len()
        ));
        for row in ledger {
            let units = &row["units"];
            let units = if units.is_object() {
                format!(
                    " · turns {} · model calls {} · items {}",
                    show(&units["native_turns"]),
                    show(&units["model_calls"]),
                    show(&units["completed_items"])
                )
            } else {
                String::new()
            };
            let bound = match (row["charge"].as_str(), row["cost_lower_bound_usd"].as_f64()) {
                (Some("unknown"), Some(bound)) if bound > 0.0 => {
                    format!(" (at least ${bound:.4})")
                }
                _ => String::new(),
            };
            lines.push(format!(
                "  {:<9} {:<16} {:<18} {:<8} {:>9}{bound} {} · {}{units}",
                row["component"].as_str().unwrap_or("?"),
                row["id"].as_str().unwrap_or("?"),
                row["model"].as_str().unwrap_or(DASH),
                row["charge"].as_str().unwrap_or("?"),
                usd(&row["cost_usd"]),
                row["provenance"].as_str().unwrap_or("?"),
                seconds(&row["milliseconds"]),
            ));
            if row["charge"].as_str() != Some("priced")
                && let Some(basis) = row["basis"].as_str()
            {
                lines.push(format!("            {basis}"));
            }
        }
    }
    if let Some(deadline) = deadline {
        match deadline["kind"].as_str() {
            Some("hard") => {
                let total = deadline["total_ms"].as_u64().unwrap_or(0);
                let used = deadline["elapsed_ms"].as_u64().unwrap_or(0);
                lines.push(format!(
                    "Deadline: {} hard, {} kept for checks and recording · {} used ({:.0}%)",
                    seconds(&deadline["total_ms"]),
                    seconds(&deadline["reserve_ms"]),
                    seconds(&deadline["elapsed_ms"]),
                    if total == 0 {
                        0.0
                    } else {
                        used as f64 * 100.0 / total as f64
                    }
                ));
            }
            _ => lines.push(format!(
                "Deadline: none; the harness timeout was the only limit · {} used",
                seconds(&deadline["elapsed_ms"])
            )),
        }
        for cut in deadline["cuts"].as_array().into_iter().flatten() {
            if cut["skipped"].as_bool() == Some(true) {
                lines.push(format!(
                    "  deadline cut: {} skipped at {} (it asked for {})",
                    cut["what"].as_str().unwrap_or("?"),
                    seconds(&cut["at_ms"]),
                    seconds(&cut["requested_ms"])
                ));
            } else {
                lines.push(format!(
                    "  deadline cut: {} at {}: asked for {}, given {}",
                    cut["what"].as_str().unwrap_or("?"),
                    seconds(&cut["at_ms"]),
                    seconds(&cut["requested_ms"]),
                    seconds(&cut["granted_ms"])
                ));
            }
        }
    }
    lines
}

fn show(value: &Value) -> String {
    match value {
        Value::Null => DASH.to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_charges_stay_apart_from_zero_and_cuts_are_named() {
        let ledger = vec![
            json!({"component":"jev","id":"jev-1","model":"jev-1.13.0","charge":"priced","cost_usd":0.001,"provenance":"price_estimate","milliseconds":40}),
            json!({"component":"jev","id":"jev-2","model":"jev-1.13.0","charge":"unknown","cost_usd":null,"provenance":"unknown","basis":"no response arrived","milliseconds":10000}),
            json!({"component":"jev","id":"jev-3","charge":"zero","cost_usd":0.0,"provenance":"none","basis":"not sent: the episode deadline left no time"}),
            json!({"component":"delegate","id":"delegate-1","model":"gpt-6-luna","charge":"unknown","cost_usd":null,"cost_lower_bound_usd":0.1,"provenance":"unknown","basis":"the deadline cut the session off","units":{"native_turns":1,"model_calls":null,"completed_items":2}}),
        ];
        let deadline = json!({
            "kind": "hard", "total_ms": 1_740_000, "reserve_ms": 30_000, "elapsed_ms": 870_000,
            "cuts": [
                {"what":"delegate-1","at_ms":300_000,"requested_ms":600_000,"granted_ms":410_000,"skipped":false},
                {"what":"jev_close","at_ms":1_710_000,"requested_ms":60_000,"granted_ms":0,"skipped":true},
            ],
        });
        let text = lines(&ledger, Some(&deadline)).join("\n");
        assert!(
            text.contains("4 · 1 priced · 1 known zero · 2 unknown charge"),
            "{text}"
        );
        assert!(text.contains("(at least $0.1000)"), "{text}");
        assert!(text.contains("items 2"), "{text}");
        assert!(
            text.contains("not sent: the episode deadline left no time"),
            "{text}"
        );
        assert!(text.contains("1740.0s hard"), "{text}");
        assert!(text.contains("(50%)"), "{text}");
        assert!(
            text.contains("delegate-1 at 300.0s: asked for 600.0s, given 410.0s"),
            "{text}"
        );
        assert!(text.contains("jev_close skipped"), "{text}");
        assert_eq!(charges(&ledger), (1, 1, 2));
    }
}
