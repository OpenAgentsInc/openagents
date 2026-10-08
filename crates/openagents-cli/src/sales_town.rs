//! The floor's bodies in the Agora: the pinned station table and where Paul
//! and the admitted hires stand with their real work. Reads only.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::agent_hiring::Book;
use coder::task::sales::town::{self, Lifecycle, Member, Role, Table};
use serde_json::Value;
use std::path::Path;

const USAGE: &str = "usage: openagents sales town COMMAND [--json]
  table                              The pinned Agora station table, its digest, and problems against the world tree.
  bodies --root DIR --credential FILE [--hires FILE] [--member NAME:ROLE:STATE]...
                                     Paul and the admitted hires placed in the Agora, each with its current canonical work.
--hires is the JSON `openagents agent hire list` prints; --member admits a fixture body
(ROLE leader|hire, STATE active|paused|retired). With neither, Paul stands alone.";

pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(v) => v,
        Err(e) => return output.usage("sales town", &e, USAGE),
    };
    match execute(&args) {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales town", &e),
    }
}

fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 || !matches!(args.positional()[0].as_str(), "table" | "bodies")
    {
        return Err(USAGE.into());
    }
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential" | "hires" | "member"))
    {
        return Err("unknown sales town option".into());
    }
    if args.positional()[0] == "bodies" {
        required(&args, "root")?;
        required(&args, "credential")?;
    }
    Ok(args)
}

fn member(spec: &str) -> Result<Member, String> {
    let parts: Vec<&str> = spec.split(':').collect();
    let [name, role, state] = parts[..] else {
        return Err(format!("--member {spec}: expected NAME:ROLE:STATE"));
    };
    let role = match role {
        "leader" => Role::Leader,
        "hire" => Role::Hire,
        _ => return Err(format!("--member {spec}: ROLE is leader or hire")),
    };
    let lifecycle = match state {
        "active" => Lifecycle::Active,
        "paused" => Lifecycle::Paused,
        "retired" => Lifecycle::Retired,
        _ => {
            return Err(format!(
                "--member {spec}: STATE is active, paused, or retired"
            ));
        }
    };
    if name.is_empty() {
        return Err(format!("--member {spec}: NAME is empty"));
    }
    Ok(Member {
        name: name.to_string(),
        pubkey: String::new(),
        role,
        lifecycle,
    })
}

fn execute(args: &Args) -> Result<Value, String> {
    let table = Table::agora();
    if args.positional()[0] == "table" {
        return Ok(serde_json::json!({
            "table": table,
            "digest": table.digest(),
            "problems": table.validate(world_tree::everglade()),
        }));
    }
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let access = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    let (mut roster, pending) = match args.option("hires") {
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("--hires {path}: {e}"))?;
            let book: Book = serde_json::from_slice(&bytes)
                .map_err(|e| format!("--hires {path}: not a hiring book: {e}"))?;
            town::roster_from_book(&book)
        }
        None => (
            town::roster_from_book(&Book {
                schema: coder::task::agent_hiring::SCHEMA.into(),
                entries: Default::default(),
            })
            .0,
            Vec::new(),
        ),
    };
    for spec in args.options("member") {
        let m = member(spec)?;
        roster.retain(|r| r.name != m.name);
        roster.push(m);
    }
    serde_json::to_value(store.town_bodies(&access, &roster, &pending)?)
        .map_err(|_| "town bodies serialization failed".to_string())
}
