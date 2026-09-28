//! `openagents playtest award`: sign a NIP-XP playtest award from an
//! accepted contribution in the triage log (`docs/game/playtest-triage.md`).
//!
//! It picks the acceptance (a filed report by its code or issue, a
//! verified fix, or a session), reads the `playtest` quest, the tester's
//! public playtest report (kind `3197`, found by the tester's key and the
//! accepted report's digest), any session record, and the referee's
//! published awards from the relay, then builds and checks the award with
//! [`playtest::award`]. Without `--publish` it prints the award; with it,
//! it publishes it. Every path first refuses unless the key is the
//! playtest referee this build trusts (`verse::xp::PLAYTEST_REFEREE`),
//! which is unset until the owner creates the key.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nostr::domain::{Event, RelaySigner};
use playtest::award;
use playtest::triage::Acceptance;
use serde_json::{Value, json};

use super::{DEFAULT_REPO, Failure, load, public_key, usage};
use crate::Args;

/// What selects the acceptance and shapes the award.
pub struct Request {
    pub code: Option<String>,
    pub issue: Option<u64>,
    pub verified: bool,
    pub script: Option<String>,
    pub tester: Option<String>,
    /// The quest's address, `<id>@<version>`.
    pub quest: String,
    pub session: Option<String>,
    pub triager: Option<String>,
    pub repo: String,
    pub commit: Option<String>,
}

/// The one acceptance `request` names.
fn select(acceptances: Vec<Acceptance>, request: &Request) -> Result<Acceptance, String> {
    let found: Vec<Acceptance> = acceptances
        .into_iter()
        .filter(|a| {
            let verified = a.contribution == "verified-fix";
            if let Some(script) = &request.script {
                a.script.as_ref() == Some(script) && request.tester.as_ref() == Some(&a.tester)
            } else if let Some(code) = &request.code {
                a.code.as_ref() == Some(code) && verified == request.verified && a.script.is_none()
            } else {
                request.issue.is_some()
                    && a.issue == request.issue
                    && verified == request.verified
                    && a.script.is_none()
            }
        })
        .collect();
    match <[Acceptance; 1]>::try_from(found) {
        Ok([one]) => Ok(one),
        Err(found) if found.is_empty() => {
            Err("no accepted contribution in the triage log matches; see `openagents playtest log --acceptances`".into())
        }
        Err(_) => Err("more than one accepted contribution matches; name it by its code".into()),
    }
}

/// Builds and signs the award for `request` as `referee`, reading events
/// with `fetch`. `expected` is the playtest referee the build trusts.
///
/// # Errors
///
/// The referee gate, a missing event, or a refusal from the playtest rule.
pub fn sign(
    home: &Path,
    request: &Request,
    referee: &RelaySigner,
    expected: Option<&str>,
    fetch: &mut dyn FnMut(Value) -> Result<Vec<Event>, String>,
) -> Result<(Event, String), String> {
    let me = referee.pubkey().to_owned();
    award::referee(&me, expected)?;
    let mut acceptance = select(load(home)?.acceptances(), request)?;
    if let Some(triager) = &request.triager {
        acceptance.triager = Some(triager.clone());
    }
    let quests = fetch(json!({
        "kinds": [nostr::kinds::XP_QUEST], "authors": [me], "#d": [request.quest],
    }))?;
    let quest = quests
        .iter()
        .find(|q| nostr::xp::parse_quest(q).is_ok())
        .ok_or_else(|| format!("no quest {} from this referee on the relay", request.quest))?;
    let reports = fetch(json!({
        "kinds": [nostr::kinds::XP_PLAYTEST_REPORT], "authors": [acceptance.tester],
    }))?;
    let report = award::find_report(&acceptance, &reports)
        .ok_or("the tester's public playtest report for this acceptance isn't on the relay yet")?;
    let session = match &request.session {
        Some(id) => Some(
            fetch(json!({"ids": [id], "kinds": [nostr::kinds::XP_PLAYTEST_SESSION]}))?
                .into_iter()
                .find(|e| &e.id == id)
                .ok_or_else(|| format!("session record {id} isn't on the relay"))?,
        ),
        None => None,
    };
    let plan = award::plan(
        &acceptance,
        quest,
        report,
        session.as_ref(),
        &request.repo,
        request.commit.as_deref(),
    )?;
    let existing = fetch(json!({
        "kinds": [nostr::kinds::XP_AWARD, nostr::kinds::XP_REVOCATION], "authors": [me],
    }))?;
    award::admit(&plan, &existing)?;
    let signed = referee.sign(
        super::now().max(acceptance.accepted_at),
        plan.unsigned.kind,
        plan.unsigned.tags,
        plan.unsigned.content,
    );
    Ok((signed, plan.key))
}

fn referee_key(args: &Args) -> Result<RelaySigner, Failure> {
    let path = args.option("referee-key").map(PathBuf::from).or_else(|| {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".openagents/nostr/playtest-referee-key"))
    });
    let path = path.ok_or_else(|| usage("--referee-key PATH is required"))?;
    if !path.exists() {
        return Err(Failure::Failed(format!(
            "there's no playtest referee key at {}; the owner creates it once with `microcoder xp playtest-keygen`",
            path.display()
        )));
    }
    let identity = coder::relay::Identity::load_from(&path).map_err(Failure::Failed)?;
    Ok(identity.signer().clone())
}

pub fn run(home: &Path, args: &Args) -> Result<Value, Failure> {
    // The production path is off until the owner creates the key.
    if verse::xp::PLAYTEST_REFEREE.is_none() {
        return Err(Failure::Failed(award::NO_REFEREE.into()));
    }
    let number = |name: &str| {
        args.option(name)
            .map(|v| {
                v.trim_start_matches('#')
                    .parse::<u64>()
                    .map_err(|_| usage(format!("--{name} takes a number")))
            })
            .transpose()
    };
    let request = Request {
        code: args.positional().first().map(|c| c.to_ascii_uppercase()),
        issue: number("issue")?,
        verified: args.switch("verified"),
        script: args.option("script").map(str::to_owned),
        tester: args.option("tester").map(public_key).transpose()?,
        quest: args
            .option("quest")
            .ok_or_else(|| usage("--quest ID@VERSION is required"))?
            .to_owned(),
        session: args.option("session").map(str::to_owned),
        triager: args.option("triager").map(public_key).transpose()?,
        repo: args.option("repo").unwrap_or(DEFAULT_REPO).to_owned(),
        commit: args.option("commit").map(str::to_owned),
    };
    if request.code.is_none() && request.issue.is_none() && request.script.is_none() {
        return Err(usage(
            "name the acceptance: a report CODE, --issue N, or --script NAME --tester KEY",
        ));
    }
    let signer = referee_key(args)?;
    let relay = args.option("relay").unwrap_or(playtest::RELAY).to_owned();
    let timeout = Duration::from_secs(args.number("timeout", 20u64).map_err(usage)?);
    let mut client = crate::relay::Client::connect(&relay, signer.clone());
    let mut fetch = |filter: Value| -> Result<Vec<Event>, String> {
        let mut events = Vec::new();
        let read = |client: &mut crate::relay::Client, events: &mut Vec<Event>| {
            client.subscribe(vec![filter.clone()], false, timeout, |e| {
                events.push(e.clone());
            })
        };
        if let Err(error) = read(&mut client, &mut events) {
            if !error.contains("auth-required") || !client.authenticate(Instant::now() + timeout) {
                return Err(error);
            }
            read(&mut client, &mut events)?;
        }
        Ok(events)
    };
    let result = sign(
        home,
        &request,
        &signer,
        verse::xp::PLAYTEST_REFEREE,
        &mut fetch,
    );
    let (signed, key) = match result {
        Ok(done) => done,
        Err(error) => {
            client.close();
            return Err(Failure::Failed(error));
        }
    };
    if !args.switch("publish") {
        client.close();
        return Ok(json!({
            "dry_run": true, "key": key, "award": signed,
            "text": format!("Would publish award {} for {key} on {relay}. Nothing was published; re-run with --publish.", signed.id),
        }));
    }
    let published = client.publish(signed.clone(), timeout);
    client.close();
    let published = published.map_err(Failure::Failed)?;
    if !published.accepted {
        return Err(Failure::Failed(format!(
            "{relay} refused the award: {}",
            published.message
        )));
    }
    Ok(json!({
        "key": key, "award": signed.id, "relay": relay,
        "text": format!("Published award {} for {key} on {relay}.", signed.id),
    }))
}

#[cfg(test)]
mod tests;
