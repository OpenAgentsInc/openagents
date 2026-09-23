//! Drafts for `coder-one ask --scope highlights`: short text a person may
//! post, written from claims `gym runs highlights` computed by fixed rules.
//!
//! The executor only words the claims. Code checks each draft before anyone
//! reads it, and refuses one that fails:
//!
//! - it names one of the chosen highlights by key;
//! - every run it cites is one that highlight cites, and the Gym has it
//!   (the ordinary citation check);
//! - every number it writes is a number the highlight writes, in its claim,
//!   its numbers, or its caveats, so no rounded, computed, or invented
//!   figure gets through;
//! - a highlight that rests on one run says so in the draft.
//!
//! Nothing posts. A refused draft is kept with its reasons.

use std::collections::BTreeSet;

use serde_json::Value;

use super::cite::Claim;

/// How many highlights an ask drafts when the operator names none.
pub const DEFAULT_CHOSEN: usize = 3;

/// Words that say a claim rests on one run.
pub const ONE_RUN: [&str; 7] = [
    "one run",
    "a single run",
    "single run",
    "n=1",
    "one attempt",
    "once",
    "anecdote",
];

/// The highlights to draft from `gym runs highlights --json`: the ones
/// `wanted` names by key, in that order, or the first [`DEFAULT_CHOSEN`].
/// Returns the chosen highlights and the wanted keys it didn't find.
#[must_use]
pub fn choose(highlights: &Value, wanted: &[String]) -> (Vec<Value>, Vec<String>) {
    let all: Vec<&Value> = highlights["highlights"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    if wanted.is_empty() {
        return (
            all.into_iter().take(DEFAULT_CHOSEN).cloned().collect(),
            Vec::new(),
        );
    }
    let mut chosen = Vec::new();
    let mut missing = Vec::new();
    for key in wanted {
        match all.iter().find(|h| h["key"].as_str() == Some(key.as_str())) {
            Some(found) => chosen.push((*found).clone()),
            None => missing.push(key.clone()),
        }
    }
    (chosen, missing)
}

/// The numbers in `text`, normalized: `$0.60`, `0.6`, and `60%` read as
/// `0.6`, `0.6`, and `60`. A digit inside a name, such as `tb4--x`,
/// `GPT-6`, or `demo__1`, isn't a number.
#[must_use]
pub fn numbers_in(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < chars.len()
            && (chars[j].is_ascii_digit()
                || (matches!(chars[j], '.' | ',')
                    && chars.get(j + 1).is_some_and(char::is_ascii_digit)))
        {
            j += 1;
        }
        let named_before = i > 0 && {
            let before = chars[i - 1];
            before.is_alphanumeric() || before == '_' || before == '-'
        };
        let named_after = chars.get(j).is_some_and(|after| {
            *after == '_'
                || (*after == '-' && chars.get(j + 1).is_some_and(|c| c.is_alphanumeric()))
        });
        if !named_before && !named_after {
            let digits: String = chars[i..j].iter().filter(|c| **c != ',').collect();
            if let Ok(value) = digits.parse::<f64>() {
                out.push(format!("{value}"));
            }
        }
        i = j;
    }
    out
}

/// Every number a highlight writes: in its claim, its numbers' texts, its
/// caveats, and its sample size.
#[must_use]
pub fn allowed(highlight: &Value) -> BTreeSet<String> {
    let mut texts: Vec<String> = vec![
        highlight["claim"].as_str().unwrap_or_default().to_string(),
        highlight["sample"].to_string(),
    ];
    for number in highlight["numbers"].as_array().into_iter().flatten() {
        texts.push(number["text"].as_str().unwrap_or_default().to_string());
    }
    for caveat in highlight["caveats"].as_array().into_iter().flatten() {
        texts.push(caveat.as_str().unwrap_or_default().to_string());
    }
    texts.iter().flat_map(|text| numbers_in(text)).collect()
}

/// Checks each draft against the highlight it names, adding a problem for
/// each failure. Run it after the citation check: a draft is refused when
/// its claim isn't verified.
pub fn check(claims: &mut [Claim], chosen: &[Value]) {
    for claim in claims.iter_mut() {
        let Some(key) = claim.highlight.clone() else {
            claim
                .problems
                .push("the draft names no highlight".to_string());
            continue;
        };
        let Some(highlight) = chosen.iter().find(|h| h["key"].as_str() == Some(&key)) else {
            claim
                .problems
                .push(format!("{key} isn't one of the chosen highlights"));
            continue;
        };
        let cited: Vec<&str> = highlight["runs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for run in &claim.runs {
            if !cited.contains(&run.as_str()) {
                claim
                    .problems
                    .push(format!("{run} isn't a run {key} cites"));
            }
        }
        let allowed = allowed(highlight);
        let mut stray: Vec<String> = numbers_in(&claim.text)
            .into_iter()
            .filter(|number| !allowed.contains(number))
            .collect();
        stray.dedup();
        for number in stray {
            claim
                .problems
                .push(format!("{number} isn't a number {key} gives"));
        }
        let lower = claim.text.to_lowercase();
        if highlight["n1"] == true && !ONE_RUN.iter().any(|words| lower.contains(words)) {
            claim.problems.push(format!(
                "{key} rests on one run, and the draft doesn't say so"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn highlights() -> Value {
        json!({"highlights": [
            {
                "key": "cost-aaaa", "rule": "cost",
                "claim": "Coder One · tunable-luna passed fix-git for 3% of what Claude Code · Opus 5.5 spent: $0.003 against $0.12 a passing run on average, 44.0 times as much for the second, over 3 and 3 runs.",
                "runs": ["panel--a/fix-git__1", "panel--b/fix-git__2"],
                "numbers": [{"label": "share_percent", "value": 3.0, "text": "3%"}, {"label": "ratio", "value": 44.0, "text": "44.0"}],
                "sample": 3, "n1": false,
                "caveats": ["The arms ran in different batches (tb4 and panel), not side by side."],
            },
            {
                "key": "leaderboard-bbbb", "rule": "leaderboard",
                "claim": "Coder One · tunable-v3 passed vba-userform-port in 1 of 1 graded runs here; the leaderboard's top 5 rows passed it 2 of 25 trials (8%).",
                "runs": ["tb4--c/vba__1"],
                "numbers": [], "sample": 1, "n1": true, "caveats": [],
            },
            {"key": "surprise-cccc", "runs": [], "numbers": [], "sample": 1, "n1": true, "caveats": []},
            {"key": "time-dddd", "runs": [], "numbers": [], "sample": 2, "n1": false, "caveats": []},
        ]})
    }

    fn draft(text: &str, key: Option<&str>, runs: &[&str]) -> Claim {
        Claim {
            text: text.to_string(),
            runs: runs.iter().map(|r| (*r).to_string()).collect(),
            steps: Vec::new(),
            judgments: Vec::new(),
            files: Vec::new(),
            marks: Vec::new(),
            highlight: key.map(str::to_string),
            problems: Vec::new(),
            citations: runs.len(),
            valid: runs.len(),
        }
    }

    #[test]
    fn numbers_are_read_and_names_are_not() {
        assert_eq!(
            numbers_in(
                "It cost $0.60, 20% of $3.00, over 1,204 s on tb4--x/demo__1 with GPT-6 Luna and Opus 5.5 in 14m 05s."
            ),
            vec!["0.6", "20", "3", "1204", "5.5", "14", "5"]
        );
        assert!(numbers_in("no digits").is_empty());
    }

    #[test]
    fn a_draft_is_refused_for_a_stray_number_a_foreign_run_or_an_unmarked_n1() {
        let (chosen, missing) = choose(
            &highlights(),
            &[
                "cost-aaaa".to_string(),
                "leaderboard-bbbb".to_string(),
                "nope".to_string(),
            ],
        );
        assert_eq!(chosen.len(), 2);
        assert_eq!(missing, vec!["nope"]);
        assert_eq!(choose(&highlights(), &[]).0.len(), DEFAULT_CHOSEN);
        let mut drafts = vec![
            // Passes: every number is the claim's, Opus 5.5 included.
            draft(
                "On fix-git, Coder One on Luna passed for $0.003 a run against $0.12 for Claude Code on Opus 5.5: 3% of the cost, over 3 runs each.",
                Some("cost-aaaa"),
                &["panel--a/fix-git__1"],
            ),
            // A rounded figure the claim doesn't give.
            draft(
                "Coder One passed fix-git for about a 40th of Claude Code's cost.",
                Some("cost-aaaa"),
                &["panel--a/fix-git__1"],
            ),
            // A run the highlight doesn't cite.
            draft(
                "Coder One passed fix-git for 3% of the cost.",
                Some("cost-aaaa"),
                &["tb4--z/zz__1"],
            ),
            // An n=1 claim phrased as a result.
            draft(
                "Coder One passes vba-userform-port, which the top 5 rows pass 8% of the time.",
                Some("leaderboard-bbbb"),
                &["tb4--c/vba__1"],
            ),
            // The same, saying it's one run.
            draft(
                "In one run, Coder One passed vba-userform-port, which the leaderboard's top 5 rows passed in 2 of 25 trials.",
                Some("leaderboard-bbbb"),
                &["tb4--c/vba__1"],
            ),
            draft("No key.", None, &["tb4--c/vba__1"]),
            draft("Another key.", Some("time-dddd"), &[]),
        ];
        check(&mut drafts, &chosen);
        let ok: Vec<bool> = drafts.iter().map(Claim::verified).collect();
        assert_eq!(
            ok,
            vec![true, false, false, false, true, false, false],
            "{drafts:#?}"
        );
        assert_eq!(
            drafts[1].problems,
            vec!["40 isn't a number cost-aaaa gives"]
        );
        assert_eq!(
            drafts[2].problems,
            vec!["tb4--z/zz__1 isn't a run cost-aaaa cites"]
        );
        assert_eq!(
            drafts[3].problems,
            vec!["leaderboard-bbbb rests on one run, and the draft doesn't say so"]
        );
        assert_eq!(drafts[5].problems, vec!["the draft names no highlight"]);
        assert_eq!(
            drafts[6].problems,
            vec!["time-dddd isn't one of the chosen highlights"]
        );
    }
}
