//! Pure report calculations over recorded GitHub and agent observations.

use jiff::{Timestamp, civil::Date, tz::TimeZone};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    #[serde(default)]
    pub issues: Vec<Value>,
    #[serde(default)]
    pub commits: Vec<Value>,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub build_sweep_log: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub captured_at: Option<String>,
}
fn tz() -> Result<TimeZone, String> {
    TimeZone::get("America/Chicago").map_err(|e| e.to_string())
}
pub(super) fn window(day: &str) -> Result<(i64, i64), String> {
    if day.len() != 10 {
        return Err("--day must be YYYY-MM-DD".into());
    }
    let date: Date = day
        .parse()
        .map_err(|_| "--day must be a valid YYYY-MM-DD date")?;
    let start = date
        .to_zoned(tz()?)
        .map_err(|e| e.to_string())?
        .timestamp()
        .as_second();
    let tomorrow = date.tomorrow().map_err(|e| e.to_string())?;
    let end = tomorrow
        .to_zoned(tz()?)
        .map_err(|e| e.to_string())?
        .timestamp()
        .as_second();
    Ok((start, end))
}
pub(super) fn iso(at: i64) -> Result<String, String> {
    Timestamp::from_second(at)
        .map(|t| t.to_string())
        .map_err(|e| e.to_string())
}
fn at(v: &Value) -> Result<i64, String> {
    if let Some(n) = v.as_i64() {
        let seconds = if n.unsigned_abs() > 100_000_000_000 {
            n / 1000
        } else {
            n
        };
        return Timestamp::from_second(seconds)
            .map(|t| t.as_second())
            .map_err(|e| e.to_string());
    }
    v.as_str()
        .ok_or("timestamp must be RFC 3339 or Unix seconds/milliseconds")?
        .parse::<Timestamp>()
        .map(|t| t.as_second())
        .map_err(|e| e.to_string())
}
fn stamp(t: Option<i64>) -> Value {
    t.and_then(|s| Timestamp::from_second(s).ok())
        .and_then(|t| {
            Some(
                t.to_zoned(tz().ok()?)
                    .strftime("%Y-%m-%d %H:%M:%S %Z")
                    .to_string(),
            )
        })
        .map_or(Value::Null, Value::String)
}
fn minutes(a: Option<i64>, b: Option<i64>) -> Value {
    match (a, b) {
        (Some(a), Some(b)) => json!((b - a) as f64 / 60.0),
        _ => Value::Null,
    }
}
fn optional_at(v: &Value) -> Result<Option<i64>, String> {
    if v.is_null() {
        Ok(None)
    } else {
        at(v).map(Some)
    }
}
fn body_claim(body: &str) -> bool {
    body.contains("<!-- openagents-coder-claim ")
        && !body.contains("openagents-coder-claim released")
}
pub(super) fn claimed_by(issue: &Value, agent: &str) -> bool {
    issue["comments"].as_array().into_iter().flatten().any(|c| {
        let body = c["body"].as_str().unwrap_or("");
        body_claim(body)
            && body.split_whitespace().any(|s| {
                s == format!("agent={agent}")
                    || s == format!("agent_id={agent}")
                    || s == format!("session={agent}")
            })
    })
}
fn mentions(message: &str, number: u64) -> bool {
    message.split('#').skip(1).any(|s| {
        s.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u64>()
            == Ok(number)
    })
}
fn issue_time(issue: &Value, snake: &str, camel: &str) -> Result<Option<i64>, String> {
    optional_at(
        issue
            .get(snake)
            .or_else(|| issue.get(camel))
            .unwrap_or(&Value::Null),
    )
}

pub(super) fn measure(input: &Input, agent: &str, day: &str) -> Result<Value, String> {
    let (start, day_end) = window(day)?;
    let end = input
        .captured_at
        .as_ref()
        .map(|s| at(&json!(s)))
        .transpose()?
        .unwrap_or(day_end)
        .min(day_end);
    if end < start {
        return Err("input was captured before the requested day".into());
    }
    let mut events: Vec<(i64, &Value)> = Vec::new();
    for e in &input.events {
        if e["agent_id"] != agent {
            continue;
        }
        let t = at(&e["at"]).map_err(|why| format!("invalid event timestamp: {why}"))?;
        if t <= end {
            events.push((t, e));
        }
    }
    events.sort_by_key(|(t, _)| *t);
    let mut notes = input.notes.clone();
    notes.push("Incident counts cover recorded observations; missing records do not establish zero incidents.".into());
    let mut issues = Vec::new();
    for issue in &input.issues {
        if issue["agent_id"] != agent && !claimed_by(issue, agent) {
            continue;
        }
        let n = issue["number"].as_u64().ok_or("issue needs a number")?;
        let opened = issue_time(issue, "created_at", "createdAt")?;
        let closed = issue_time(issue, "closed_at", "closedAt")?.filter(|t| *t <= end);
        let mut claims = Vec::new();
        for c in issue["comments"].as_array().into_iter().flatten() {
            if body_claim(c["body"].as_str().unwrap_or("")) {
                if let Some(t) = issue_time(c, "created_at", "createdAt")? {
                    claims.push(t);
                }
            }
        }
        claims.extend(
            events
                .iter()
                .filter(|(_, e)| e["issue"] == n && e["kind"] == "claim")
                .map(|(t, _)| *t),
        );
        let last_claim = claims
            .iter()
            .copied()
            .filter(|t| *t <= end && closed.is_none_or(|c| *t <= c))
            .max();
        let claim = claims
            .into_iter()
            .filter(|t| *t <= end && opened.is_none_or(|o| *t >= o))
            .min();
        let mut commits = Vec::new();
        let mut seen = BTreeSet::new();
        for commit in &input.commits {
            if !seen.insert(commit["sha"].as_str().unwrap_or("").to_owned()) {
                continue;
            }
            if mentions(commit["commit"]["message"].as_str().unwrap_or(""), n) {
                if let Some(t) = optional_at(
                    commit["commit"]["author"]
                        .get("date")
                        .unwrap_or(&commit["commit"]["committer"]["date"]),
                )? {
                    if t <= end && opened.is_none_or(|o| t >= o) {
                        commits.push(t);
                    }
                }
            }
        }
        let first_commit = commits.iter().min().copied();
        let ready = events
            .iter()
            .filter(|(t, e)| {
                e["issue"] == n && e["kind"] == "ready" && closed.is_none_or(|c| *t <= c)
            })
            .map(|(t, _)| *t)
            .max();
        let branch_push = events
            .iter()
            .filter(|(t, e)| {
                e["issue"] == n && e["kind"] == "push" && closed.is_none_or(|c| *t <= c)
            })
            .map(|(t, _)| *t)
            .max();
        let complete = ready.into_iter().chain(branch_push).max();
        let active_today = opened.is_some_and(|o| o < end) && closed.is_none_or(|c| c >= start);
        if !active_today {
            continue;
        }
        issues.push(json!({"number":n,"title":issue["title"],"opened":stamp(opened),"claimed":stamp(claim),
            "first_commit":stamp(first_commit),"first_commit_source":"earliest referenced commit author date (committer date if absent)","code_complete":stamp(complete),"closed":stamp(closed),
            "code_complete_source":if complete.is_some() && complete == ready {json!("ready")}else if branch_push.is_some(){json!("push")}else{Value::Null},
            "minutes":{"open_to_claim":minutes(opened,claim),"claim_to_first_commit":minutes(claim,first_commit),
                "first_commit_to_code_complete":minutes(first_commit,complete),"code_complete_to_close":minutes(complete,closed),
                "last_claim_to_close":minutes(last_claim,closed),
                "open_to_close":minutes(opened,closed),"claim_to_close":minutes(claim,closed)}}));
    }
    issues.sort_by_key(|i| i["number"].as_u64());
    let mut stalls = Vec::new();
    let mut waiting: BTreeMap<String, (i64, String)> = BTreeMap::new();
    let mut waits = Vec::new();
    let mut checks = Vec::new();
    let mut due: BTreeMap<String, i64> = BTreeMap::new();
    let mut listed = BTreeSet::new();
    let mut surprises = Vec::new();
    let mut probes = Vec::new();
    let mut disk_samples = Vec::new();
    // Live status observations give the last output time, not a guessed idle duration.
    let mut silent: BTreeMap<String, (i64, bool)> = BTreeMap::new();
    for (t, e) in &events {
        let kind = e["kind"].as_str().ok_or("event needs a kind")?;
        let id = e["id"]
            .as_str()
            .or(e["chat_id"].as_str())
            .unwrap_or("agent")
            .to_owned();
        match kind {
            "stall" => {
                let stop = optional_at(&e["ended_at"])?.unwrap_or(end).min(end);
                if stop < *t {
                    return Err("stall ends before it starts".into());
                }
                if stop > start {
                    stalls.push(json!({"chat_id":id,"own_turn":e["own_turn"].as_bool().unwrap_or(false),"started":stamp(Some((*t).max(start))),"ended":stamp(Some(stop)),"minutes":(stop-(*t).max(start)) as f64/60.0}));
                }
            }
            "chat_status" => {
                let previous = silent.remove(&id);
                let working = e["status"] == "working";
                let last = optional_at(&e["last_output_at"])?
                    .unwrap_or_else(|| previous.map_or(*t, |(last, _)| last));
                if last > *t {
                    return Err("chat output is after its observation".into());
                }
                if let Some((old, own)) = previous {
                    if working && last < old {
                        return Err("chat output moved backwards".into());
                    }
                    if !working || last > old {
                        add_silence(
                            &mut stalls,
                            &id,
                            old,
                            if working { last } else { *t },
                            own,
                            start,
                            end,
                        );
                    }
                }
                if working {
                    silent.insert(
                        id,
                        (
                            last,
                            e["own_turn"]
                                .as_bool()
                                .unwrap_or_else(|| previous.is_some_and(|(_, own)| own)),
                        ),
                    );
                }
            }
            "owner_wait" => {
                waiting
                    .entry(id)
                    .or_insert((*t, e["what"].as_str().unwrap_or("owner action").to_owned()));
            }
            "owner_done" => {
                if let Some((beg, what)) = waiting.remove(&id) {
                    add_wait(&mut waits, &id, &what, beg, *t, start, end);
                }
            }
            "owner_list" => {
                for d in e["deploys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    listed.insert(d.to_owned());
                }
            }
            "deploy_refused" if *t >= start && e["permission"] == true => {
                let deploy = e["deploy"].as_str().ok_or("deploy_refused needs deploy")?;
                if !listed.contains(deploy) {
                    surprises
                        .push(json!({"at":stamp(Some(*t)),"deploy":deploy,"reason":e["reason"]}));
                }
            }
            "probe" if *t >= start => {
                let method = e["method"].as_str().unwrap_or("GET");
                let target = e["target"].as_str().unwrap_or("unknown");
                let writes = e["wrote_data"].as_bool().unwrap_or(false)
                    || (method == "POST" && e["path"] == "/v1/visitors");
                if writes && matches!(target, "production" | "owner") {
                    probes.push(json!({"at":stamp(Some(*t)),"method":method,"path":e["path"],"target":target,"count":e["count"].as_u64().unwrap_or(1)}));
                }
            }
            "check_in_due" => {
                due.insert(id, *t);
            }
            "check_in" | "wake" => {
                let recorded_due = due.remove(&id);
                let scheduled = optional_at(&e["due_at"])?.or(recorded_due);
                if *t >= start {
                    checks.push(json!({"id":id,"at":stamp(Some(*t)),"due":stamp(scheduled),"late_minutes":scheduled.map(|d| json!((t-d).max(0) as f64/60.0)).unwrap_or(Value::Null),"actions":0}));
                }
            }
            "action" if *t >= start => {
                let check = e["check_in_id"].as_str();
                if let Some(row) = checks
                    .iter_mut()
                    .rev()
                    .find(|c| check.is_none_or(|id| c["id"] == id))
                {
                    row["actions"] = json!(row["actions"].as_u64().unwrap_or(0) + 1);
                }
            }
            "disk" => {
                disk_samples.push((*t, e["free_gib"].as_f64().ok_or("disk needs free_gib")?));
            }
            // Existing decisions are text. Do not infer action counts or safety events from prose.
            "decision" | "outcome" | "authority" | "claim" | "ready" | "push" => {}
            _ if *t < start => {}
            _ => notes.push(format!("Unmeasured event kind: {kind}")),
        }
    }
    for (id, (last, own)) in silent {
        add_silence(&mut stalls, &id, last, end, own, start, end);
    }
    for (id, (beg, what)) in waiting {
        add_wait(&mut waits, &id, &what, beg, end, start, end);
    }
    let missed: Vec<_> = due
        .iter()
        .filter(|(_, t)| **t >= start && **t < end)
        .map(|(id, t)| json!({"id":id,"due":stamp(Some(*t)),"late_minutes":(end-*t) as f64/60.0}))
        .collect();
    let on_time = checks
        .iter()
        .filter(|c| c["late_minutes"].as_f64() == Some(0.0))
        .count();
    let late = checks
        .iter()
        .filter(|c| c["late_minutes"].as_f64().is_some_and(|m| m > 0.0))
        .count();
    let mut by_chat: BTreeMap<String, f64> = BTreeMap::new();
    let mut own_minutes = 0.0;
    for s in &stalls {
        let m = s["minutes"].as_f64().unwrap_or(0.0);
        if s["own_turn"] == true {
            own_minutes += m;
        } else {
            *by_chat
                .entry(s["chat_id"].as_str().unwrap_or("").into())
                .or_default() += m;
        }
    }
    let (samples, invalid) = sweep_samples(&input.build_sweep_log)?;
    disk_samples.extend(samples);
    if invalid > 0 {
        notes.push(format!(
            "{invalid} build-sweep sample lines could not be read."
        ));
    }
    disk_samples.sort_by_key(|(t, _)| *t);
    let mut breaches = Vec::new();
    let mut below = None;
    let mut observed = 0;
    for (t, free) in disk_samples.into_iter().filter(|(t, _)| *t <= end) {
        if t >= start {
            observed += 1;
        }
        if free < 100.0 {
            below.get_or_insert(t);
        } else if let Some(beg) = below.take() {
            add_breach(&mut breaches, beg, t, start, end, false);
        }
    }
    if let Some(beg) = below {
        add_breach(&mut breaches, beg, end, start, end, true);
    }
    if events.is_empty() {
        notes.push(
            "No agent observations supplied; no recorded events does not establish zero incidents."
                .into(),
        );
    }
    if !input.build_sweep_log.is_empty() {
        notes.push("Disk intervals use sampled times; gaps before the first sample are unknown. Disk is a machine measurement shared by agents.".into());
    }
    Ok(
        json!({"schema":"openagents.agent-report.v1","agent_id":agent,"day":day,"timezone":"America/Chicago","through":stamp(Some(end)),
        "issues":issues,"stalls":{"own_turn_minutes":own_minutes,"chat_minutes":by_chat,"intervals":stalls},
        "check_ins":{"on_time":on_time,"late":late,"timing_unknown":checks.len()-on_time-late,"missed":missed,"entries":checks},
        "disk":{"floor_gib":100,"samples":observed,"breaches":breaches},
        "production_writes":{"target":0,"recorded_count":probes.iter().map(|p|p["count"].as_u64().unwrap_or(0)).sum::<u64>(),"entries":probes},
        "deploy_permission_surprises":{"target":0,"recorded_count":surprises.len(),"entries":surprises},
        "coverage":{"agent_observations":!events.is_empty(),"disk_samples":observed>0},
        "owner_wait":{"minutes":waits.iter().filter_map(|w|w["minutes"].as_f64()).sum::<f64>(),"entries":waits},
        "sources":input.sources,"notes":notes}),
    )
}

fn add_silence(
    out: &mut Vec<Value>,
    id: &str,
    last: i64,
    stop: i64,
    own: bool,
    start: i64,
    end: i64,
) {
    let threshold = if own { 600 } else { 1200 };
    let beg = (last + threshold).max(start);
    let stop = stop.min(end);
    if stop > beg {
        out.push(json!({"chat_id":id,"own_turn":own,"started":stamp(Some(beg)),"ended":stamp(Some(stop)),"minutes":(stop-beg) as f64/60.0}));
    }
}
fn add_wait(out: &mut Vec<Value>, id: &str, what: &str, beg: i64, stop: i64, start: i64, end: i64) {
    let a = beg.max(start);
    let b = stop.min(end);
    if b > a {
        out.push(json!({"id":id,"what":what,"started":stamp(Some(a)),"ended":stamp(Some(b)),"minutes":(b-a) as f64/60.0}));
    }
}
fn add_breach(out: &mut Vec<Value>, beg: i64, stop: i64, start: i64, end: i64, ongoing: bool) {
    let a = beg.max(start);
    let b = stop.min(end);
    if b >= a && b >= start {
        out.push(json!({"started":stamp(Some(a)),"ended":stamp(Some(b)),"minutes":(b-a) as f64/60.0,"ongoing":ongoing,"started_before_day":beg<start}));
    }
}
fn sweep_samples(log: &str) -> Result<(Vec<(i64, f64)>, usize), String> {
    let mut samples = Vec::new();
    let mut invalid = 0;
    for line in log.lines().filter(|s| s.contains("sweep  free ")) {
        let parse = || -> Result<Vec<(i64, f64)>, String> {
            let (date, rest) = line
                .split_once("  sweep  free ")
                .ok_or("missing sweep separator")?;
            let (date, zone) = date.rsplit_once(' ').ok_or("missing sweep timezone")?;
            let offset = match zone {
                "CDT" => "-05:00",
                "CST" => "-06:00",
                _ => return Err("unknown sweep timezone".into()),
            };
            let civil = jiff::fmt::strtime::parse("%Y-%m-%d %I:%M:%S %p", date)
                .map_err(|e| e.to_string())?
                .to_datetime()
                .map_err(|e| e.to_string())?;
            let t = at(&json!(format!("{civil}{offset}")))?;
            let words: Vec<_> = rest.split_whitespace().collect();
            if words.len() < 5
                || words[1] != "GiB"
                || words[2] != "->"
                || words[4].trim_end_matches(',') != "GiB"
            {
                return Err("invalid disk amounts".into());
            }
            let before = words[0].parse::<f64>().map_err(|e| e.to_string())?;
            let after = words[3].parse::<f64>().map_err(|e| e.to_string())?;
            // Both observations matter when this sweep itself restores the floor.
            Ok(vec![(t, before), (t, after)])
        };
        match parse() {
            Ok(v) => samples.extend(v),
            Err(_) => invalid += 1,
        }
    }
    Ok((samples, invalid))
}

pub(super) fn text(v: &Value) -> String {
    let mut lines = vec![
        format!(
            "Agent {} — {} (America/Chicago), through {}",
            v["agent_id"].as_str().unwrap_or(""),
            v["day"].as_str().unwrap_or(""),
            v["through"].as_str().unwrap_or("")
        ),
        "Issue  Open → claim → first commit → code complete → close (minutes between steps)".into(),
    ];
    for i in v["issues"].as_array().into_iter().flatten() {
        let show = |key: &str| {
            i["minutes"][key]
                .as_f64()
                .map(|n| format!("{n:.1}"))
                .unwrap_or("unknown".into())
        };
        lines.push(format!(
            "#{}  {} / {} / {} / {} | last claim → close {} min",
            i["number"],
            show("open_to_claim"),
            show("claim_to_first_commit"),
            show("first_commit_to_code_complete"),
            show("code_complete_to_close"),
            show("last_claim_to_close")
        ));
        lines.push(format!(
            "  {} → {} → {} → {} → {}",
            date_text(&i["opened"]),
            date_text(&i["claimed"]),
            date_text(&i["first_commit"]),
            date_text(&i["code_complete"]),
            date_text(&i["closed"])
        ));
    }
    lines.push(format!(
        "Stalled: own turn {} min; chats {}",
        v["stalls"]["own_turn_minutes"], v["stalls"]["chat_minutes"]
    ));
    let c = &v["check_ins"];
    lines.push(format!(
        "Check-ins: {} on time, {} late, {} timing unknown, {} missed",
        c["on_time"],
        c["late"],
        c["timing_unknown"],
        c["missed"].as_array().map_or(0, Vec::len)
    ));
    for c in c["entries"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {}: {} actions; late {} min",
            date_text(&c["at"]),
            c["actions"],
            c["late_minutes"]
        ));
    }
    lines.push(format!(
        "Disk below 100 GiB: {} intervals ({} samples)",
        v["disk"]["breaches"].as_array().map_or(0, Vec::len),
        v["disk"]["samples"]
    ));
    for b in v["disk"]["breaches"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} → {}: {} min",
            date_text(&b["started"]),
            date_text(&b["ended"]),
            b["minutes"]
        ));
    }
    lines.push(format!("Recorded production/owner writes: {} (target 0); deploy-permission surprises: {} (target 0)",v["production_writes"]["recorded_count"],v["deploy_permission_surprises"]["recorded_count"]));
    lines.push(format!(
        "Waiting on owner: {} item-minutes",
        v["owner_wait"]["minutes"]
    ));
    for w in v["owner_wait"]["entries"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {}: {} → {}, {} min",
            w["what"].as_str().unwrap_or(""),
            date_text(&w["started"]),
            date_text(&w["ended"]),
            w["minutes"]
        ));
    }
    for n in v["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        lines.push(n.into());
    }
    lines.join("\n")
}
fn date_text(v: &Value) -> &str {
    v.as_str().unwrap_or("unknown")
}
