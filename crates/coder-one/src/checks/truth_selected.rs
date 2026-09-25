//! Attribute retained executor evidence to the host's submitted candidate.
//!
//! "Kept first" means the candidate immediately before the second executor,
//! which can already include escalation and repair. Persistence can replace it
//! afterward. Native stream order alone cannot recover these choices.

use super::truth_micro::Selected;
use serde_json::Value;
use std::path::Path;

fn missing(reason: &str) -> Selected {
    Selected {
        unavailable: Some(reason.into()),
        ..Selected::default()
    }
}

fn session(composition: &Value) -> Result<(&str, bool), &'static str> {
    if !composition["best_of"]["kept"].is_null() {
        return Err("Best-of-N report attribution requires a candidate-specific stream");
    }
    let branches = composition["branches"]
        .as_array()
        .ok_or("No executor branch identities")?;
    let mut chosen = branches
        .iter()
        .rev()
        .find(|b| matches!(b["role"].as_str(), Some("primary" | "escalation")))
        .and_then(|b| b["session_id"].as_str())
        .ok_or("No primary or escalated session identity")?;
    let mut repair = false;
    if composition["repair"]["ran"] == true {
        chosen = composition["repair"]["session"]["session_id"]
            .as_str()
            .ok_or("Repair has no session identity")?;
        repair = true;
    }
    let second = &composition["second"];
    if !second["restore_error"].is_null() {
        return Err("Second-candidate restoration failed");
    }
    match second["kept"].as_str() {
        Some("second") => {
            chosen = branches
                .iter()
                .find(|b| b["role"] == "second")
                .and_then(|b| b["session_id"].as_str())
                .ok_or("Second executor has no session identity")?;
            repair = false;
        }
        Some("first") | None => (),
        Some(_) => return Err("Unknown second-candidate selection"),
    }
    for round in composition["persist"]["rounds"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if !round["restore_error"].is_null() {
            return Err("Persistence restoration failed");
        }
        match round["kept"].as_bool() {
            Some(true) => {
                chosen = round["session_id"]
                    .as_str()
                    .ok_or("Kept persistence round has no session identity")?;
                repair = false;
            }
            Some(false) => (),
            None => return Err("Persistence round has no candidate selection"),
        }
    }
    Ok((chosen, repair))
}

/// Use recorded host selection and native session identities, never a grade.
pub(crate) fn select(episode: &Path, composition: &Value) -> Selected {
    let (id, repair) = match session(composition) {
        Ok(value) => value,
        Err(why) => return missing(why),
    };
    // A Microluna dispatch has its own explicit keep-best selection. Its final
    // native session identity need not name the candidate that was restored.
    if let Some(dispatch) = id
        .strip_prefix("microluna-")
        .and_then(|s| s.split('-').next())
        .filter(|s| s.parse::<usize>().is_ok())
    {
        let source = format!("artifacts/microluna-{dispatch}.json");
        return std::fs::read(episode.join(&source))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .map_or_else(
                || missing("Selected Microluna record is missing"),
                |v| super::truth_micro::select(&v, &source),
            );
    }
    let mut streams = Vec::new();
    for entry in std::fs::read_dir(episode.join("artifacts"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(base) = name
            .strip_prefix("delegate-")
            .and_then(|s| s.strip_suffix(".stream.jsonl"))
        else {
            continue;
        };
        let (dispatch, resume) = base.split_once(".resume-").unwrap_or((base, "0"));
        let (Ok(dispatch), Ok(resume)) = (dispatch.parse::<usize>(), resume.parse::<usize>())
        else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let matches = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .any(|event| event["session_id"] == id || event["thread_id"] == id);
        if matches {
            streams.push((dispatch, resume, name, text));
        }
    }
    streams.sort_by_key(|(dispatch, resume, _, _)| (*dispatch, *resume));
    if let Some((_, _, name, text)) = streams.last() {
        let report = super::replay::final_report(text);
        return Selected {
            unavailable: report
                .is_none()
                .then(|| "Selected native stream has no final report".into()),
            source: Some(format!("artifacts/{name}")),
            report,
            ..Selected::default()
        };
    }
    if repair
        && let Some(report) = composition["repair"]["session"]["result"]
            .as_str()
            .filter(|s| !s.is_empty())
    {
        return Selected {
            report: Some(format!(
                "Retained repair report excerpt; the full native stream is unavailable:\n{report}"
            )),
            source: Some("artifacts/composition.json#repair.session.result".into()),
            ..Selected::default()
        };
    }
    missing("No native stream matches the selected executor session")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn composition() -> Value {
        json!({"branches":[{"role":"primary","session_id":"initial"},{"role":"escalation","session_id":"escalated"},{"role":"second","session_id":"alternative"}],
            "repair":{"ran":true,"session":{"session_id":"repaired"}},"second":{"kept":"first"}})
    }
    #[test]
    fn keeping_first_preserves_escalation_and_repair() {
        let mut c = composition();
        assert_eq!(session(&c), Ok(("repaired", true)));
        c["repair"]["ran"] = json!(false);
        assert_eq!(session(&c), Ok(("escalated", false)));
    }
    #[test]
    fn later_kept_round_wins_but_a_discarded_round_does_not() {
        let mut c = composition();
        c["second"]["kept"] = json!("second");
        assert_eq!(session(&c), Ok(("alternative", false)));
        c["persist"] = json!({"rounds":[{"kept":true,"session_id":"kept"},{"kept":false,"session_id":"discarded"}]});
        assert_eq!(session(&c), Ok(("kept", false)));
        c["persist"]["rounds"][1]["restore_error"] = json!("partial restore");
        assert!(session(&c).is_err());
    }
    #[test]
    fn resumes_are_ordered_numerically_and_bound_to_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let artifacts = dir.path().join("artifacts");
        std::fs::create_dir(&artifacts).unwrap();
        for (name, id, report) in [
            ("delegate-2.resume-9.stream.jsonl", "repaired", "older"),
            ("delegate-2.resume-10.stream.jsonl", "repaired", "newer"),
            ("delegate-3.stream.jsonl", "alternative", "discarded"),
        ] {
            std::fs::write(
                artifacts.join(name),
                json!({"type":"result","session_id":id,"result":report}).to_string(),
            )
            .unwrap();
        }
        let got = select(dir.path(), &composition());
        assert_eq!(got.report.as_deref(), Some("newer"));
        assert_eq!(
            got.source.as_deref(),
            Some("artifacts/delegate-2.resume-10.stream.jsonl")
        );
    }
    #[test]
    fn missing_selected_stream_does_not_fall_back_to_initial_failure() {
        let dir = tempfile::tempdir().unwrap();
        let artifacts = dir.path().join("artifacts");
        std::fs::create_dir(&artifacts).unwrap();
        std::fs::write(
            artifacts.join("delegate-1.stream.jsonl"),
            json!({"type":"result","session_id":"initial","result":"broken"}).to_string(),
        )
        .unwrap();
        let mut c = composition();
        assert!(select(dir.path(), &c).report.is_none());
        c["repair"]["session"]["result"] = json!("Both commands now pass");
        let got = select(dir.path(), &c);
        assert!(got.report.unwrap().contains("excerpt"));
        assert!(got.source.unwrap().ends_with("#repair.session.result"));
    }
}
