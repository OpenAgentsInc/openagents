//! `openagents verse town`: author Everglade's townsfolk
//! (`docs/verse/generative-agents.md`, "Authoring townsfolk").
//!
//! The behavior lives in the `townsfolk` crate and the zone's
//! `townsfolk` module; this file parses words and prints. Anyone, a
//! workshop agent included, may validate, preview, and propose. Only the
//! owner admits or removes a villager, by typing its ID at a terminal: an
//! agent's task has no terminal on its standard input, and no charter
//! grants either command.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use serde_json::json;
use town_clock::TownTime;
use townsfolk::files::{self, Dir};
use townsfolk::routine::Villager;
use townsfolk::validate::{Checks, Router, Screen};
use townsfolk::{Problem, sim};
use verse::zones::everglade::townsfolk::{Routes, blockers_from_pack};
use verse::zones::everglade_pack::{PACK_DIRECTORY, PACK_EXTENSION, PACK_SHA256};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents verse town COMMAND [OPTIONS]
  list                      Every definition and roster entry and its state:
                            draft, proposed, refused, admitted, changed, or
                            missing.
  validate [ID...]          Check definitions (default: every one) against the
                            world tree, the zone's routes, the roster's budgets
                            and exclusive objects, and the secret screen; exit
                            1 when one fails.
  preview [ID] [--at HH:MM,...] [--day N]
                            The admitted town, with definition ID added or
                            replaced, at each time (default 05:30, 08:00,
                            12:30, 17:30, 22:00): where each villager stands or
                            walks, who stands together, ID's routine, and the
                            day's meetings.
  propose ID                Validate a definition with its routes and stage
                            proposals/ID.json for the owner. Anyone may propose.
  admit ID --owner          Add the proposed digest to town.json. The owner's
                            action: it asks you to type the ID at a terminal,
                            and no agent's charter grants it. Commit town.json
                            to ship it.
  remove ID --owner         Take a villager off the roster; its file stays.
Options: --dir PATH (default crates/verse-zone-everglade/townsfolk in this
checkout), --pack PATH (the pinned Everglade pack the routes come from;
default assets/verse/everglade in this checkout), --no-routes (validate and
preview without routing legs).";

const COMMANDS: &[&str] = &["list", "validate", "preview", "propose", "admit", "remove"];

/// The townsfolk directory under a checkout.
const DIR_IN_CHECKOUT: &str = "crates/verse-zone-everglade/townsfolk";

pub(crate) fn run(output: &Output, words: &[String]) -> u8 {
    let group = "verse town";
    let Some((command, rest)) = words.split_first() else {
        return output.usage(group, "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    if !COMMANDS.contains(&command.as_str()) {
        return output.usage(group, &format!("unknown command `{command}`"), USAGE);
    }
    let args = match Args::parse(rest, &["owner", "no-routes"]) {
        Ok(args) => args,
        Err(message) => return output.usage(group, &message, USAGE),
    };
    for name in args.option_names() {
        if !["dir", "pack", "at", "day"].contains(&name) {
            return output.usage(
                group,
                &format!("--{name} isn't an option of {group}"),
                USAGE,
            );
        }
    }
    let ids = args.positional();
    let (min, max) = match command.as_str() {
        "list" => (0, 0),
        "validate" => (0, usize::MAX),
        "preview" => (0, 1),
        _ => (1, 1),
    };
    if ids.len() < min || ids.len() > max {
        return output.usage(group, &format!("wrong number of IDs for {command}"), USAGE);
    }
    let dir = match directory(&args) {
        Ok(dir) => dir,
        Err(message) => return output.fail(group, &message),
    };
    let result = match command.as_str() {
        "list" => list(output, &dir),
        "validate" => validate(output, &dir, &args),
        "preview" => preview(output, &dir, &args),
        "propose" => propose(output, &dir, &args, &ids[0]),
        "admit" | "remove" => owner(output, &dir, &args, command, &ids[0]),
        _ => Ok(0),
    };
    match result {
        Ok(code) => code,
        Err(message) => output.fail(group, &message),
    }
}

/// `--dir`, or the townsfolk directory of the checkout this runs in.
fn directory(args: &Args) -> Result<Dir, String> {
    if let Some(dir) = args.option("dir") {
        return Ok(Dir::new(dir));
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    cwd.ancestors()
        .map(|a| a.join(DIR_IN_CHECKOUT))
        .find(|d| d.join(files::TOWN_FILE).is_file())
        .map(Dir::new)
        .ok_or_else(|| {
            format!("no {DIR_IN_CHECKOUT}/town.json above this directory; run in a checkout or pass --dir")
        })
}

/// The pinned pack: `--pack`, or the checkout's above the directory.
fn pack(args: &Args, dir: &Dir) -> Option<PathBuf> {
    if let Some(path) = args.option("pack") {
        return Some(PathBuf::from(path));
    }
    let name = format!("{PACK_SHA256}.{PACK_EXTENSION}");
    let root = dir
        .root()
        .canonicalize()
        .unwrap_or_else(|_| dir.root().to_path_buf());
    root.ancestors()
        .map(|a| a.join(PACK_DIRECTORY).join(&name))
        .find(|p| p.is_file())
}

/// The zone's blockers, unless `--no-routes`.
fn blockers(args: &Args, dir: &Dir) -> Result<Option<Vec<verse::controller::Footprint>>, String> {
    if args.switch("no-routes") {
        return Ok(None);
    }
    let path = pack(args, dir).ok_or_else(|| {
        format!(
            "routes need the pinned Everglade pack ({PACK_DIRECTORY}/{PACK_SHA256}.{PACK_EXTENSION} in a checkout); pass --pack PATH, or --no-routes"
        )
    })?;
    blockers_from_pack(&path).map(Some)
}

fn screen() -> impl Screen {
    let screen = secret_screen::Screen::host();
    move |text: &str| screen.check(text).err().map(|r| r.to_string())
}

fn problems_text(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|p| format!("  {p}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn list(output: &Output, dir: &Dir) -> Result<u8, String> {
    let rows = files::list(dir)?;
    let value = json!({ "dir": dir.root(), "villagers": rows });
    output.emit(&value, |_| {
        if rows.is_empty() {
            return "no townsfolk definitions".into();
        }
        let mut table = vec![vec![
            "ID".to_owned(),
            "STATE".to_owned(),
            "DIGEST".to_owned(),
        ]];
        for row in &rows {
            let digest = row
                .file_digest
                .as_deref()
                .or(row.admitted_digest.as_deref())
                .unwrap_or("");
            table.push(vec![
                row.id.clone(),
                row.state.to_owned(),
                digest.chars().take(19).collect(),
            ]);
        }
        crate::out::table(&table)
    });
    Ok(0)
}

fn validate(output: &Output, dir: &Dir, args: &Args) -> Result<u8, String> {
    let tree = world_tree::everglade();
    let blockers = blockers(args, dir)?;
    let routes = blockers
        .as_deref()
        .map(|blockers| Routes { tree, blockers });
    let screen = screen();
    let ids = if args.positional().is_empty() {
        dir.ids()?
    } else {
        args.positional().to_vec()
    };
    let mut rows = Vec::new();
    let mut failed = false;
    for id in &ids {
        let mut checks = Checks::new(tree, &screen);
        if let Some(routes) = &routes {
            checks = checks.with_router(routes as &dyn Router);
        }
        let (digest, problems) = match files::check(dir, id, checks) {
            Ok((npc, problems)) => (Some(npc.digest()), problems),
            Err(why) => (
                None,
                vec![Problem::new("file", townsfolk::Code::Schema, why)],
            ),
        };
        failed |= !problems.is_empty();
        rows.push(json!({
            "id": id,
            "digest": digest,
            "valid": problems.is_empty(),
            "problems": problems,
        }));
    }
    let (_, left_out) = dir.roster(tree)?;
    let value = json!({
        "routes_checked": routes.is_some(),
        "definitions": rows,
        "roster_left_out": left_out,
    });
    output.emit(&value, |_| {
        let mut out = Vec::new();
        for (id, row) in ids.iter().zip(&rows) {
            let problems: Vec<Problem> =
                serde_json::from_value(row["problems"].clone()).unwrap_or_default();
            if problems.is_empty() {
                out.push(format!("{id}: valid"));
            } else {
                out.push(format!(
                    "{id}: {} problems\n{}",
                    problems.len(),
                    problems_text(&problems)
                ));
            }
        }
        if routes.is_none() {
            out.push("routes not checked (--no-routes)".into());
        }
        if !left_out.is_empty() {
            out.push(format!(
                "the roster as a client loads it leaves out:\n{}",
                problems_text(&left_out)
            ));
        }
        out.join("\n")
    });
    Ok(if failed { crate::out::EXIT_FAILURE } else { 0 })
}

/// `--at`'s times, as hours.
fn hours(args: &Args) -> Result<Vec<f64>, String> {
    match args.option("at") {
        None => Ok(vec![5.5, 8.0, 12.5, 17.5, 22.0]),
        Some(text) => text.split(',').map(town_clock::parse_hour).collect(),
    }
}

fn preview(output: &Output, dir: &Dir, args: &Args) -> Result<u8, String> {
    let tree = world_tree::everglade();
    let day = args.number::<i64>("day", 0)?;
    let hours = hours(args)?;
    let (roster, _) = dir.roster(tree)?;
    let seed = roster.town.seed;
    let mut villagers: Vec<Villager> = roster.villagers;
    let focus = args.positional().first().cloned();
    let mut schedule = Vec::new();
    if let Some(id) = &focus {
        let npc = dir.npc(id)?;
        let villager = match Villager::compile(npc, tree) {
            Ok(v) => v,
            Err(problems) => {
                return Err(format!(
                    "{id} doesn't validate:\n{}",
                    problems_text(&problems)
                ));
            }
        };
        schedule = sim::schedule(&villager, tree);
        villagers.retain(|v| v.id() != id);
        villagers.push(villager);
    }
    let moments: Vec<sim::Moment> = hours
        .iter()
        .map(|&h| sim::moment(&villagers, tree, seed, TownTime::at_hour(day, h)))
        .collect();
    let meetings: Vec<sim::Meeting> = sim::meetings(&villagers, seed, day, 300)
        .into_iter()
        .filter(|m| focus.as_ref().is_none_or(|id| m.ids.contains(id)))
        .collect();
    let value = json!({
        "day": day,
        "seed": seed,
        "schedule": schedule,
        "moments": moments,
        "meetings": meetings,
    });
    output.emit(&value, |_| {
        let mut out = schedule.clone();
        for m in &moments {
            out.extend(sim::render(m, tree));
        }
        if !meetings.is_empty() {
            out.push(format!("meetings on day {day}:"));
            for m in &meetings {
                let place = tree
                    .node(&m.node)
                    .map_or(m.node.as_str(), |n| n.name.as_str());
                out.push(format!(
                    "  {}-{} {place}: {}",
                    m.from,
                    m.to,
                    m.ids.join(", ")
                ));
            }
        }
        out.join("\n")
    });
    Ok(0)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn propose(output: &Output, dir: &Dir, args: &Args, id: &str) -> Result<u8, String> {
    if args.switch("no-routes") {
        return Err("a proposal routes every leg; drop --no-routes".into());
    }
    let tree = world_tree::everglade();
    let blockers = blockers(args, dir)?.unwrap_or_default();
    let routes = Routes {
        tree,
        blockers: &blockers,
    };
    let screen = screen();
    let checks = Checks::new(tree, &screen).with_router(&routes);
    let proposal = files::propose(dir, id, checks, now_unix())?;
    let path = dir.proposal_path(id);
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

/// Asks the owner to type `id` at the terminal.
fn confirm(id: &str, what: &str) -> Result<(), String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(format!(
            "{what} asks the owner to confirm at a terminal, and standard input isn't one; \
             an agent's task can't {what}, and no charter grants it"
        ));
    }
    eprint!("Type {id} to {what} it: ");
    std::io::stderr().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.trim() != id {
        return Err(format!("not confirmed; {id} is unchanged"));
    }
    Ok(())
}

fn owner(output: &Output, dir: &Dir, args: &Args, command: &str, id: &str) -> Result<u8, String> {
    if !args.switch("owner") {
        return Err(format!(
            "{command} is the owner's action: pass --owner and confirm at a terminal"
        ));
    }
    if command == "admit" {
        let tree = world_tree::everglade();
        let proposal = dir
            .proposal(id)?
            .ok_or_else(|| format!("{id} has no proposal; run propose first"))?;
        for line in &proposal.day {
            eprintln!("{line}");
        }
        confirm(id, "admit")?;
        let entry = files::admit(dir, id, tree)?;
        let value =
            json!({ "admitted": entry.id, "digest": entry.digest, "town": dir.town_path() });
        output.emit(&value, |_| {
            format!(
                "admitted {id} ({}); commit {} to ship it",
                entry.digest,
                dir.town_path().display()
            )
        });
        return Ok(0);
    }
    confirm(id, "remove")?;
    let removed = files::remove(dir, id)?;
    let value = json!({ "removed": removed, "id": id });
    output.emit(&value, |_| {
        if removed {
            format!("removed {id} from the roster; its file stays")
        } else {
            format!("{id} wasn't on the roster")
        }
    });
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn admit_and_remove_need_the_owner_at_a_terminal() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("townsfolk");
        std::fs::create_dir_all(dir.join("npcs")).unwrap();
        Dir::new(&dir)
            .write_town(&townsfolk::Town::new("everglade", 1))
            .unwrap();
        let output = Output::new(true);
        let d = dir.display();
        assert_eq!(
            run(&output, &words(&format!("admit someone --dir {d}"))),
            crate::out::EXIT_FAILURE
        );
        // Under the test harness standard input isn't a terminal, as in an
        // agent's task: even --owner is refused.
        assert!(confirm("someone", "admit").is_err());
        assert_eq!(
            run(
                &output,
                &words(&format!("remove someone --owner --dir {d}"))
            ),
            crate::out::EXIT_FAILURE
        );
        assert_eq!(run(&output, &words(&format!("list --dir {d}"))), 0);
        assert_eq!(
            run(&output, &words(&format!("nope --dir {d}"))),
            crate::out::EXIT_USAGE
        );
    }
}
