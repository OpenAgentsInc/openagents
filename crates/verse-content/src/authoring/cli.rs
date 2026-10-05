//! JSON commands for a persistent local editor and standalone content operations.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Read, Write},
    path::Path,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Inspect {},
    Transaction {
        transaction: Transaction,
    },
    Undo {
        expected_revision: u64,
    },
    Redo {
        expected_revision: u64,
    },
    Preview {
        #[serde(default)]
        ticks: u32,
        #[serde(default)]
        navigation: Option<f64>,
    },
    Build {},
}
fn value(value: &impl Serialize) -> Result<serde_json::Value> {
    serde_json::to_value(value).map_err(|e| Diagnostic::at("command", "$", e.to_string()))
}
pub fn command(
    workspace: &mut Workspace,
    active: &mut Option<Preview>,
    command: &Command,
) -> Result<serde_json::Value> {
    match command {
        Command::Inspect {} => workspace.inspect(),
        Command::Transaction { transaction } => {
            Ok(serde_json::json!({"revision":workspace.transact(transaction)?}))
        }
        Command::Undo { expected_revision } => {
            Ok(serde_json::json!({"revision":workspace.undo(*expected_revision)?}))
        }
        Command::Redo { expected_revision } => {
            Ok(serde_json::json!({"revision":workspace.redo(*expected_revision)?}))
        }
        Command::Build {} => value(&workspace.build()?),
        Command::Preview { ticks, navigation } => {
            let mut candidate = workspace.preview()?;
            candidate.step(*ticks)?;
            let report = value(&candidate.report(*navigation)?)?;
            *active = Some(candidate);
            Ok(report)
        }
    }
}
/// Every input line receives either a result or a diagnostic. A failure leaves the editor open.
pub fn session(
    workspace: &mut Workspace,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<()> {
    let mut active = None;
    for _ in 0..4096 {
        let mut line = Vec::new();
        let count = (&mut *input)
            .take(2 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| Diagnostic::at("stdin", "$", e.to_string()))?;
        if count == 0 {
            return Ok(());
        }
        if line.len() > 2 * 1024 * 1024 {
            return Err(Diagnostic::at("stdin", "$", "Editor line exceeds 2 MiB"));
        }
        let result = parse::<Command>("stdin", &line, 2 * 1024 * 1024)
            .and_then(|c| command(workspace, &mut active, &c));
        let response = match result {
            Ok(result) => serde_json::json!({"ok":true,"result":result}),
            Err(error) => serde_json::json!({"ok":false,"diagnostic":error}),
        };
        serde_json::to_writer(&mut *output, &response)
            .map_err(|e| Diagnostic::at("stdout", "$", e.to_string()))?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| Diagnostic::at("stdout", "$", e.to_string()))?;
    }
    Err(Diagnostic::at(
        "stdin",
        "$",
        "Editor command budget is 4096; reopen the workspace",
    ))
}
/// Dispatches the author's subcommands; all filesystem writes stay in the named workspace.
pub fn run(args: &[std::ffi::OsString]) -> Result<()> {
    let usage = "Usage: verse-content author init INPUT WORKSPACE ZONE | inspect WORKSPACE | apply WORKSPACE TRANSACTION | undo|redo WORKSPACE REVISION | preview WORKSPACE TICKS [NAV_HALF] | build WORKSPACE | edit WORKSPACE";
    let fail = || Diagnostic::at("arguments", "$", usage);
    let name = args.first().and_then(|s| s.to_str()).ok_or_else(fail)?;
    if name == "init" {
        if args.len() != 4 {
            return Err(fail());
        }
        let workspace = Workspace::init(
            Path::new(&args[1]),
            Path::new(&args[2]),
            args[3].to_str().ok_or_else(fail)?.into(),
        )?;
        println!(
            "{}",
            serde_json::json!({"revision":workspace.revision(),"zone":workspace.document().zone})
        );
        return Ok(());
    }
    let root = args.get(1).ok_or_else(fail)?;
    let mut workspace = Workspace::open(Path::new(root))?;
    let mut active = None;
    let output = match name {
        "inspect" if args.len() == 2 => command(&mut workspace, &mut active, &Command::Inspect {})?,
        "apply" if args.len() == 3 => {
            let tx = parse::<Transaction>(
                "transaction.json",
                &workspace::read(Path::new(&args[2]), 2 * 1024 * 1024)?,
                2 * 1024 * 1024,
            )?;
            command(
                &mut workspace,
                &mut active,
                &Command::Transaction { transaction: tx },
            )?
        }
        "undo" | "redo" if args.len() == 3 => {
            let expected_revision = args[2]
                .to_str()
                .ok_or_else(fail)?
                .parse()
                .map_err(|_| fail())?;
            let c = if name == "undo" {
                Command::Undo { expected_revision }
            } else {
                Command::Redo { expected_revision }
            };
            command(&mut workspace, &mut active, &c)?
        }
        "preview" if (3..=4).contains(&args.len()) => {
            let ticks = args[2]
                .to_str()
                .ok_or_else(fail)?
                .parse()
                .map_err(|_| fail())?;
            let navigation = args
                .get(3)
                .map(|s| s.to_str().ok_or_else(fail)?.parse().map_err(|_| fail()))
                .transpose()?;
            let report = command(
                &mut workspace,
                &mut active,
                &Command::Preview { ticks, navigation },
            )?;
            let svg = active.as_ref().ok_or_else(fail)?.svg(navigation)?;
            let path = Path::new(root).join("preview.svg");
            // create_new refuses an unexpected link; replacement happens only after a valid preview.
            workspace::write_preview(&path, svg.as_bytes())?;
            report
        }
        "build" if args.len() == 2 => command(&mut workspace, &mut active, &Command::Build {})?,
        "edit" if args.len() == 2 => {
            return session(
                &mut workspace,
                &mut std::io::stdin().lock(),
                &mut std::io::stdout().lock(),
            );
        }
        _ => return Err(fail()),
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|e| Diagnostic::at("stdout", "$", e.to_string()))?
    );
    Ok(())
}
