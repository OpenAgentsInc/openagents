//! `openagents verse town rumor`: author Everglade's rumors
//! (`docs/verse/generative-agents.md`, "Authoring townsfolk").
//!
//! A rumor is `townsfolk/rumors/ID.json`. Anyone may validate, preview,
//! and propose one; `propose` sets its repeat score once, with Jev over
//! `questions/rumor-repeat.json` (or the prior with `--prior`), and writes
//! the score into the file. The owner admits or removes it with `openagents
//! verse town admit|remove ID --owner`, as a villager.

use std::sync::LazyLock;

use coder::questions::{Fill, Set};
use serde_json::json;
use townsfolk::files::{self, Dir};
use townsfolk::rumor::{self, Fixed, Repeat, Rumor, Scorer};
use townsfolk::{Npc, Problem, diffusion};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents verse town rumor COMMAND [OPTIONS]
  validate [ID...]          Check rumors (default: every one): a real quest
                            step, a fact within 200 characters that passes the
                            secret screen, a source villager standing at the
                            node when it starts, and the roster's
                            rumors_in_flight; exit 1 when one fails.
  preview ID [--days N]     Who learns rumor ID, when, where, and from whom,
                            over N town days (default: its own days).
  propose ID [--prior]      Validate, set the repeat score once (Jev over
                            questions/rumor-repeat.json, or 0.5 with --prior),
                            write it into the file, and stage
                            proposals/rumors/ID.json for the owner.
The owner admits or removes a rumor with `openagents verse town admit|remove
ID --owner`. Options: --dir PATH (default crates/verse-zone-everglade/townsfolk
in this checkout).";

const SET_JSON: &str = include_str!("../../../questions/rumor-repeat.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the rumor-repeat set parses");
    set.validate()
        .expect("the rumor-repeat set is one this host asks");
    set
});

/// Today's town day under the default clock.
pub(crate) fn today() -> i64 {
    town_clock::Clock::DEFAULT
        .at_unix(crate::verse_town::now_unix())
        .day
}

pub(crate) fn run(output: &Output, words: &[String], dir: &Dir) -> u8 {
    let group = "verse town rumor";
    let Some((command, rest)) = words.split_first() else {
        return output.usage(group, "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["prior"]) {
        Ok(args) => args,
        Err(message) => return output.usage(group, &message, USAGE),
    };
    for name in args.option_names() {
        if !["dir", "days"].contains(&name) {
            return output.usage(
                group,
                &format!("--{name} isn't an option of {group}"),
                USAGE,
            );
        }
    }
    let ids = args.positional();
    let result = match (command.as_str(), ids.len()) {
        ("validate", _) => validate(output, dir, ids),
        ("preview", 1) => preview(output, dir, &args, &ids[0]),
        ("propose", 1) => propose(output, dir, &args, &ids[0]),
        ("preview" | "propose", _) => {
            return output.usage(group, &format!("{command} takes one ID"), USAGE);
        }
        _ => return output.usage(group, &format!("unknown command `{command}`"), USAGE),
    };
    match result {
        Ok(code) => code,
        Err(message) => output.fail(group, &message),
    }
}

fn problems_text(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|p| format!("  {p}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate(output: &Output, dir: &Dir, ids: &[String]) -> Result<u8, String> {
    let tree = world_tree::everglade();
    let screen = crate::verse_town::screen();
    let ids = if ids.is_empty() {
        dir.rumor_ids()?
    } else {
        ids.to_vec()
    };
    let today = today();
    let mut rows = Vec::new();
    let mut failed = false;
    for id in &ids {
        let problems = match files::check_rumor(dir, id, tree, &screen, today) {
            Ok((_, problems)) => problems,
            Err(why) => vec![Problem::new("file", townsfolk::Code::Schema, why)],
        };
        failed |= !problems.is_empty();
        rows.push((id.clone(), problems));
    }
    let value = json!({
        "today": today,
        "rumors": rows.iter().map(|(id, p)| json!({"id": id, "valid": p.is_empty(), "problems": p})).collect::<Vec<_>>(),
    });
    output.emit(&value, |_| {
        let mut out = vec![format!("today is town day {today}")];
        if rows.is_empty() {
            out.push("no rumors".into());
        }
        out.extend(rows.iter().map(|(id, p)| {
            if p.is_empty() {
                format!("{id}: valid")
            } else {
                format!("{id}: {} problems\n{}", p.len(), problems_text(p))
            }
        }));
        out.join("\n")
    });
    Ok(if failed { crate::out::EXIT_FAILURE } else { 0 })
}

fn preview(output: &Output, dir: &Dir, args: &Args, id: &str) -> Result<u8, String> {
    let tree = world_tree::everglade();
    let rumor = dir.rumor(id)?;
    let days = args.number::<u32>("days", rumor.days)?.clamp(1, 30);
    let (roster, _) = dir.roster(tree)?;
    let spread = diffusion::diffusion(&roster.villagers, roster.town.seed, &rumor, days);
    let name = |id: &str| {
        roster
            .villager(id)
            .map_or(id.to_owned(), |v| v.npc.name.clone())
    };
    let mut lines = Vec::new();
    if rumor.repeat.is_none() {
        lines.push(format!(
            "not scored yet: the preview uses the prior, {}",
            rumor::PRIOR_REPEAT
        ));
    }
    lines.extend(spread.render(tree, name));
    let value = json!({ "rumor": rumor, "diffusion": spread, "share": spread.share() });
    output.emit(&value, |_| lines.join("\n"));
    Ok(0)
}

/// Jev over `questions/rumor-repeat.json`.
struct JevScorer {
    client: jev::Client,
}

fn place_name(id: &str) -> String {
    world_tree::everglade()
        .node(id)
        .map_or(id.to_owned(), |n| n.name.clone())
}

impl Scorer for JevScorer {
    fn score(&mut self, rumor: &Rumor, source: &Npc) -> Result<Repeat, String> {
        let state = json!({
            "rumor": {
                "fact": rumor.fact,
                "source": format!("{}, the {}", source.name, source.card.role),
                "place": place_name(&rumor.node),
            }
        });
        let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
        if let Some(max) = SET.policy.state_max_bytes
            && size as u64 > max
        {
            return Err(format!("the state is {size} bytes, over the set's {max}"));
        }
        let request = jev::SystemOneRequest::new(state, SET.build(&Fill::None)?);
        let client = self.client.clone();
        let response = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(client.system_one(request))
                .map_err(|e| format!("Jev: {e}"))
        })
        .join()
        .map_err(|_| "the Jev request stopped".to_owned())??;
        let levels = SET
            .questions
            .get(&SET.gate)
            .and_then(|q| q.get("criteria"))
            .and_then(serde_json::Value::as_array)
            .map_or(4, Vec::len);
        match response.answers.get(SET.gate.as_str()) {
            Some(jev::Answer::Score(answer)) => Ok(Repeat {
                probability: rumor::repeat_from(&answer.probabilities, answer.score, levels),
                basis: format!("jev {} {}", SET.id, response.model),
            }),
            _ => Err("Jev didn't answer the repeat question".into()),
        }
    }
}

fn propose(output: &Output, dir: &Dir, args: &Args, id: &str) -> Result<u8, String> {
    let tree = world_tree::everglade();
    let screen = crate::verse_town::screen();
    let mut prior = Fixed::PRIOR;
    let mut jev;
    let scorer: &mut dyn Scorer = if args.switch("prior") {
        &mut prior
    } else {
        let judge = crate::background::JevJudge::from_env()
            .ok_or_else(|| format!("{} Or pass --prior.", crate::background::NO_JEV))?;
        jev = JevScorer {
            client: judge.client(),
        };
        &mut jev
    };
    let proposal = files::propose_rumor(
        dir,
        id,
        tree,
        &screen,
        scorer,
        crate::verse_town::now_unix(),
        today(),
    )?;
    let path = dir.rumor_proposal_path(id);
    let value = json!({ "path": path, "proposal": proposal });
    output.emit(&value, |_| {
        let mut out = vec![format!(
            "staged {}: {}",
            path.display(),
            if proposal.valid { "valid" } else { "refused" }
        )];
        if proposal.valid {
            out.extend(proposal.day.iter().cloned());
            out.push(format!(
                "the owner admits it with: openagents verse town admit {id} --owner"
            ));
        } else {
            out.push(problems_text(&proposal.problems));
        }
        out.join("\n")
    });
    Ok(if proposal.valid {
        0
    } else {
        crate::out::EXIT_FAILURE
    })
}
