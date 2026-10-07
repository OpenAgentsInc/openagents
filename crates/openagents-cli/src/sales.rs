//! Private sales records. This command never contacts a prospect or starts an agent.
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::sales::{Role, Store};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
pub const USAGE: &str = "usage: openagents sales COMMAND --root DIR [--credential FILE] [--json]
  init --owner HUMAN --credential FILE
        Initialize the private pipeline and write its owner's credential.
  issue --human HUMAN --role writer|reader --new-credential FILE
        Grant a named human private record access; prints no credential.
  revoke --human HUMAN
        Revoke that human's credential and cancel proposed handoffs to them.
  apply --input FILE
        Apply a bounded versioned JSON command; FILE=- reads stdin.
  list [--after LEAD] [--limit N]
        List only records this credential can read, at most 100.
  show --lead LEAD
        Read one authorized private lead/account record.
  export --lead LEAD --output FILE
        Create a private exclusive JSON export; no shared/public export.
  audit [--after N] [--limit N]
        Read the owner's bounded digest-only audit references.
  suppressed --contact CHANNEL:ADDRESS
        Inspect minimum suppression before an authorized future contact.

All commands require an explicit private host root. Except init, read the
current human's credential from FILE; do not put its secret on the command
line. Propose a handoff through apply with that lead's current revision;
only the named target's credential can accept it. This pipeline grants no
outbound, provider, execution, or customer-data disclosure authority.";
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("init", Effect::Grants),
    Declared::computer("issue", Effect::Grants),
    Declared::computer("revoke", Effect::Grants),
    Declared::computer("apply", Effect::Grants),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("export", Effect::LocalWrite),
    Declared::computer("audit", Effect::ReadOnly),
    Declared::computer("suppressed", Effect::ReadOnly),
];
pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "--help" | "help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(args) => args,
        Err(message) => return output.usage("sales", &message, USAGE),
    };
    match execute_args(&args) {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(message) => output.fail("sales", &message),
    }
}
fn required<'a>(args: &'a Args, name: &str) -> Result<&'a str, String> {
    args.option(name)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("--{name} is required"))
}
fn page(args: &Args) -> Result<usize, String> {
    args.option("limit")
        .unwrap_or("50")
        .parse()
        .map_err(|_| "invalid page limit".into())
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    let positional = args.positional();
    if positional.len() != 1 {
        return Err(USAGE.into());
    }
    let command = positional[0].as_str();
    let allowed: &[&str] = match command {
        "init" => &["root", "owner", "credential"],
        "issue" => &["root", "credential", "human", "role", "new-credential"],
        "revoke" => &["root", "credential", "human"],
        "apply" => &["root", "credential", "input"],
        "list" | "audit" => &["root", "credential", "after", "limit"],
        "show" => &["root", "credential", "lead"],
        "export" => &["root", "credential", "lead", "output"],
        "suppressed" => &["root", "credential", "contact"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|name| !allowed.contains(name))
    {
        return Err("unknown sales option".into());
    }
    for name in match command {
        "init" => vec!["root", "credential", "owner"],
        "issue" => vec!["root", "credential", "human", "role", "new-credential"],
        "revoke" => vec!["root", "credential", "human"],
        "apply" => vec!["root", "credential", "input"],
        "show" => vec!["root", "credential", "lead"],
        "export" => vec!["root", "credential", "lead", "output"],
        "suppressed" => vec!["root", "credential", "contact"],
        _ => vec!["root", "credential"],
    } {
        required(&args, name)?;
    }
    if matches!(command, "list" | "audit") && !(1..=100).contains(&page(&args)?) {
        return Err("page limit must be 1 to 100".into());
    }
    if command == "audit" && args.option("after").unwrap_or("0").parse::<u64>().is_err() {
        return Err("invalid audit cursor".into());
    }
    if command == "issue" && !matches!(args.option("role"), Some("writer" | "reader")) {
        return Err("role must be writer or reader".into());
    }
    Ok(args)
}
#[cfg(test)]
fn execute(words: &[String]) -> Result<Value, String> {
    execute_args(&parse(words)?)
}
fn execute_args(args: &Args) -> Result<Value, String> {
    let command = args.positional()[0].as_str();
    let mut store = Store::open(Path::new(required(&args, "root")?))?;
    let credential = Path::new(required(&args, "credential")?);
    if command == "init" {
        store.initialize(required(&args, "owner")?, credential)?;
        return Ok(
            json!({"initialized":true,"owner":required(&args,"owner")?,"credential_file":credential.display().to_string()}),
        );
    }
    let access = store.authenticate(&Store::read_credential(credential)?)?;
    let result = match command {
        "issue" => {
            let role = match required(&args, "role")? {
                "writer" => Role::Writer,
                "reader" => Role::Reader,
                _ => return Err("role must be writer or reader".into()),
            };
            let human = required(&args, "human")?;
            store.issue(
                &access,
                human,
                role,
                Path::new(required(&args, "new-credential")?),
            )?;
            json!({"issued":human,"role":role})
        }
        "revoke" => {
            let human = required(&args, "human")?;
            store.revoke(&access, human)?;
            json!({"revoked":human})
        }
        "apply" => {
            let path = required(&args, "input")?;
            let mut bytes = Vec::new();
            if path == "-" {
                std::io::stdin()
                    .take(32 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
            } else {
                std::fs::File::open(path)
                    .map_err(|e| e.to_string())?
                    .take(32 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
            }
            serde_json::to_value(store.apply(&access, &bytes)?).map_err(|e| e.to_string())?
        }
        "list" => json!({"records":store.list(&access,args.option("after"),page(&args)?)?}),
        "show" => serde_json::to_value(store.show(&access, required(&args, "lead")?)?)
            .map_err(|e| e.to_string())?,
        "export" => {
            json!({"sha256":store.export(&access,required(&args,"lead")?,Path::new(required(&args,"output")?))?})
        }
        "audit" => {
            let after = args
                .option("after")
                .unwrap_or("0")
                .parse()
                .map_err(|_| "invalid audit cursor")?;
            json!({"audit":store.audit(&access,after,page(&args)?)?})
        }
        "suppressed" => {
            json!({"suppressed":store.is_suppressed(&access,required(&args,"contact")?)?})
        }
        _ => unreachable!(),
    };
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn words(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }
    #[test]
    fn explicit_scratch_root_credentials_and_no_secret_output() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("host");
        let credential = d.path().join("owner-token");
        let result = execute(&words(&[
            "init",
            "--root",
            root.to_str().unwrap(),
            "--owner",
            "operator",
            "--credential",
            credential.to_str().unwrap(),
        ]))
        .unwrap();
        let secret = Store::read_credential(&credential).unwrap();
        assert!(!result.to_string().contains(&secret));
        let result = execute(&words(&[
            "list",
            "--root",
            root.to_str().unwrap(),
            "--credential",
            credential.to_str().unwrap(),
        ]))
        .unwrap();
        assert_eq!(result["records"], json!([]));
        assert!(
            execute(&words(&[
                "list",
                "--credential",
                credential.to_str().unwrap()
            ]))
            .is_err()
        );
        assert!(
            execute(&words(&[
                "list",
                "--root",
                root.to_str().unwrap(),
                "--credential",
                credential.to_str().unwrap(),
                "--limit",
                "101"
            ]))
            .is_err()
        );
        assert!(
            execute(&words(&[
                "list",
                "--root",
                root.to_str().unwrap(),
                "--credential",
                credential.to_str().unwrap(),
                "--unexpected",
                "yes"
            ]))
            .is_err()
        );
    }
}
