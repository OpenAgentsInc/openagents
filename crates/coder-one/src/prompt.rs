//! `coder-one prompt …`: the system prompt section library, and captures
//! of what each executor sends for a variant.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use crate::capture::{self, Plan};
use crate::delegate::Agent;
use crate::policy::{Manifest, REFERENCE};
use crate::system::{self, Policy, Variant};

/// The prompt commands' usage.
pub const USAGE: &str = "usage: coder-one prompt list [--json]
       coder-one prompt show AGENT VARIANT [--select IDS]
       coder-one prompt capture [--agent claude-code|codex] [--variant V]... [--select IDS]
                                [--out DIR] [--json]

AGENT is claude-code or codex. VARIANT is default (the CLI's own prompt), core,
core-select, or a JSON executor.system object. --select names the optional
sections to include as if Jev had selected them: a comma-separated list, or all.

capture runs the real CLI against a local server that records the first request
and answers with an error: no inference, a dummy credential, a scratch home, and
no header recorded. Each agent runs with its reference manifest's settings
(claude-code: jevprobe2-opus-lean-low-5m; codex: jevprobe3-luna). Codex reads
the model list from ~/.codex/models_cache.json and nothing else from ~/.codex.
Without --variant, capture measures default, core, and core with every optional
section. Results go under ~/.openagents/coder-one/prompts unless --out names a
directory.";

struct Flags {
    positional: Vec<String>,
    agents: Vec<Agent>,
    variants: Vec<String>,
    select: Option<String>,
    out: Option<PathBuf>,
    json: bool,
}

fn parse(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags {
        positional: Vec::new(),
        agents: Vec::new(),
        variants: Vec::new(),
        select: None,
        out: None,
        json: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--agent" => flags.agents.push(Agent::parse(&value("--agent")?)?),
            "--variant" => flags.variants.push(value("--variant")?),
            "--select" => flags.select = Some(value("--select")?),
            "--out" => flags.out = Some(value("--out")?.into()),
            "--json" => flags.json = true,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => flags.positional.push(other.to_string()),
        }
    }
    Ok(flags)
}

/// The variant a name or JSON object names for `agent`; `None` is the
/// CLI's default.
///
/// # Errors
///
/// Returns a message for an unknown name, invalid JSON, or a policy the
/// agent can't run.
pub fn variant(agent: Agent, name: &str, select: Option<&str>) -> Result<Option<Variant>, String> {
    if name == "default" {
        return Ok(None);
    }
    let policy = if name.starts_with('{') {
        serde_json::from_str::<Policy>(name).map_err(|error| format!("invalid variant: {error}"))?
    } else {
        Policy::preset(name).ok_or_else(|| {
            format!("unknown variant {name}: use default, core, core-select, or JSON")
        })?
    };
    let problems = policy.validate(agent);
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let mut variant = Variant::new(agent, policy);
    if let Some(select) = select {
        let ids: Vec<String> = if select == "all" {
            system::OPTIONAL
                .iter()
                .map(|id| (*id).to_string())
                .collect()
        } else {
            select.split(',').map(|id| id.trim().to_string()).collect()
        };
        for id in &ids {
            if !system::OPTIONAL.contains(&id.as_str()) {
                return Err(format!("{id} is not an optional section"));
            }
        }
        variant.select(ids.into_iter().map(|id| (id, Some(1.0))).collect());
    }
    Ok(Some(variant))
}

/// Runs a prompt command and returns its exit code.
///
/// # Errors
///
/// Returns a message for a usage error or a failed capture.
pub fn command(args: &[String]) -> Result<i32, String> {
    let Some(sub) = args.first() else {
        println!("{USAGE}");
        return Ok(0);
    };
    let flags = parse(&args[1..])?;
    match sub.as_str() {
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(0)
        }
        "list" => {
            let record = system::library_record();
            if flags.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&record).unwrap_or_default()
                );
            } else {
                print!("{}", list_text(&record));
            }
            Ok(0)
        }
        "show" => {
            let [agent, name] = flags.positional.as_slice() else {
                return Err(format!("prompt show needs AGENT and VARIANT\n{USAGE}"));
            };
            let agent = Agent::parse(agent)?;
            match variant(agent, name, flags.select.as_deref())? {
                Some(variant) => print!("{}", variant.text()),
                None => print!("{}", system::default_text(agent)),
            }
            Ok(0)
        }
        "capture" => capture_command(&flags),
        other => Err(format!("unknown prompt command {other}\n{USAGE}")),
    }
}

fn list_text(record: &Value) -> String {
    let mut out = String::new();
    let rows = |sections: &Value, out: &mut String| {
        for section in sections.as_array().into_iter().flatten() {
            out.push_str(&format!(
                "  {:<28} {:<9} {:>6}  {}\n",
                section["id"].as_str().unwrap_or_default(),
                section["status"].as_str().unwrap_or_default(),
                section["chars"].to_string(),
                section["note"].as_str().unwrap_or_default()
            ));
        }
    };
    for default in record["defaults"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{} default: {} characters{}\n",
            default["default"].as_str().unwrap_or_default(),
            default["chars"],
            if default["protected"] == json!(true) {
                ""
            } else {
                ", no security policy"
            }
        ));
        rows(&default["sections"], &mut out);
        out.push('\n');
    }
    out.push_str("headless library\n");
    rows(&record["library"], &mut out);
    out.push('\n');
    for preset in record["presets"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "preset {}: {} characters for claude-code, {} for codex\n",
            preset["name"].as_str().unwrap_or_default(),
            preset["chars"]["claude-code"],
            preset["chars"]["codex"]
        ));
    }
    out
}

/// The reference manifest each agent's captures use.
fn reference(agent: Agent) -> Result<Manifest, String> {
    let name = match agent {
        Agent::ClaudeCode => "jevprobe2-opus-lean-low-5m.json",
        Agent::Codex | Agent::Microluna => "jevprobe3-luna.json",
    };
    let text = REFERENCE
        .iter()
        .find(|(file, _)| *file == name)
        .map(|(_, text)| *text)
        .ok_or_else(|| format!("no reference manifest {name}"))?;
    Manifest::parse(text)
}

/// The synthetic briefing every capture sends, so request sizes compare.
pub const BRIEFING: &str =
    "<the Coder One briefing, written to briefing.md and piped to standard input>\n";

fn capture_command(flags: &Flags) -> Result<i32, String> {
    let agents = if flags.agents.is_empty() {
        vec![Agent::ClaudeCode, Agent::Codex]
    } else {
        flags.agents.clone()
    };
    let variants: Vec<(String, Option<String>)> = if flags.variants.is_empty() {
        vec![
            ("default".to_string(), None),
            ("core".to_string(), None),
            ("core".to_string(), Some("all".to_string())),
        ]
    } else {
        flags
            .variants
            .iter()
            .map(|name| (name.clone(), flags.select.clone()))
            .collect()
    };
    let out = match &flags.out {
        Some(out) => out.clone(),
        None => crate::credentials::openagents_dir()
            .ok_or("HOME is not set; name --out")?
            .join("coder-one/prompts")
            .join(format!("capture-{}", atif::now_ms())),
    };
    std::fs::create_dir_all(&out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;
    let catalog = out.join("codex-catalog.json");
    let codex_catalog = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".codex/models_cache.json"))
        .filter(|cache| cache.is_file())
        .and_then(|cache| capture::codex_catalog(&cache, &catalog).ok())
        .map(|()| catalog.clone());
    let env = |name: &str| std::env::var(name).ok();
    let mut measurements = Vec::new();
    let mut failed = 0;
    for agent in agents {
        let manifest = reference(agent)?;
        let executor = &manifest.policy.executor;
        let binary = crate::delegate::binary(agent, env)
            .ok_or_else(|| format!("no {} binary on PATH", agent.word()))?;
        for (name, select) in &variants {
            let variant = variant(agent, name, select.as_deref())?;
            let label = match select.as_deref() {
                Some("all") => format!("{name}+optional"),
                Some(ids) => format!("{name}+{ids}"),
                None => name.clone(),
            };
            let plan = Plan {
                agent,
                model: executor.model.clone(),
                binary: binary.clone(),
                effort: executor.effort.clone(),
                tools: executor.tools.clone(),
                prompt_cache_ttl: executor.prompt_cache_ttl.clone(),
                variant,
                label: label.clone(),
                briefing: BRIEFING.to_string(),
                codex_catalog: codex_catalog.clone(),
                timeout: Duration::from_secs(90),
            };
            eprintln!("capture ▸ {} {label}", agent.word());
            match capture::capture(&plan) {
                Ok(captured) => {
                    let file = out.join(format!("{}-{label}.request.json", agent.word()));
                    std::fs::write(
                        &file,
                        format!(
                            "{}\n",
                            serde_json::to_string_pretty(&captured.request).unwrap_or_default()
                        ),
                    )
                    .map_err(|error| error.to_string())?;
                    measurements.push(captured.measurement);
                }
                Err(error) => {
                    failed += 1;
                    eprintln!("capture ▸ {} {label} failed: {error}", agent.word());
                    measurements.push(json!({
                        "schema": capture::SCHEMA,
                        "agent": agent.word(),
                        "label": label,
                        "error": error,
                    }));
                }
            }
        }
    }
    let _ = std::fs::remove_file(&catalog);
    let record = json!({
        "schema": "openagents.coder-one.prompt-captures.v1",
        "cli_versions": { "claude-code": system::CLAUDE_CODE_VERSION, "codex": system::CODEX_VERSION },
        "briefing": BRIEFING,
        "note": "First-request sizes and cache markers from a local capture server; no inference ran. estimated_tokens is characters / 4.",
        "captures": measurements,
    });
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&record).unwrap_or_default()
    );
    std::fs::write(out.join("measurements.json"), &text).map_err(|error| error.to_string())?;
    if flags.json {
        print!("{text}");
    } else {
        print!("{}", captures_text(&record));
        println!("wrote {}", out.display());
    }
    Ok(i32::from(failed > 0))
}

/// The captures as a table: one row per agent and variant.
#[must_use]
pub fn captures_text(record: &Value) -> String {
    let mut out = format!(
        "{:<12} {:<16} {:>9} {:>9} {:>7} {:>9} {:>9}  markers\n",
        "agent", "variant", "system", "request", "tools", "tool chr", "~tokens"
    );
    for capture in record["captures"].as_array().into_iter().flatten() {
        if let Some(error) = capture.get("error") {
            out.push_str(&format!(
                "{:<12} {:<16} failed: {}\n",
                capture["agent"].as_str().unwrap_or_default(),
                capture["label"].as_str().unwrap_or_default(),
                error.as_str().unwrap_or_default()
            ));
            continue;
        }
        let markers: Vec<String> = capture["markers"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|marker| {
                format!(
                    "{}{}",
                    marker["at"].as_str().unwrap_or_default(),
                    match (marker["type"].as_str(), marker["ttl"].as_str()) {
                        (_, Some(ttl)) => format!(" ({ttl})"),
                        (Some("ephemeral"), None) => " (5m)".to_string(),
                        _ => String::new(),
                    }
                )
            })
            .collect();
        let n = |value: &Value| value.as_u64().map_or("—".to_string(), |n| n.to_string());
        out.push_str(&format!(
            "{:<12} {:<16} {:>9} {:>9} {:>7} {:>9} {:>9}  {}{}\n",
            capture["agent"].as_str().unwrap_or_default(),
            capture["label"].as_str().unwrap_or_default(),
            n(&capture["variant"]["chars"]),
            n(&capture["totals"]["text_chars"]),
            n(&capture["totals"]["tools"]),
            n(&capture["totals"]["tools_chars"]),
            n(&capture["totals"]["estimated_tokens"]),
            markers.join(", "),
            if capture["checks"]["protected_present"] == json!(true) {
                ""
            } else {
                "  [no security policy]"
            }
        ));
    }
    out
}
