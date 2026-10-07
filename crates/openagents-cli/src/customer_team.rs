//! Team commands project the native customer custody and existing account APIs.
use crate::{Args, Output};
use coder::customer::{Store, TeamCommand, TeamStatus};
use serde_json::{Value, json};
use std::path::Path;
pub const USAGE: &str = "usage: openagents customer team COMMAND --root DIR [OPTIONS]
  change --input FILE [--invitation FILE]
        Apply one reviewed TeamCommand; invitation tokens stay in private files.
  members --workspace ID
        Read current roles and membership through the selected account.
  switch --workspace ID --door NAME [--alias NAME]
        Select current workspace and payer rights for future purchases.
  inspect --operation ID
        Read the original team intent under current account and workspace rights.

TeamCommand has id, origin, account, credential_alias, and action. Actions are
create, invite, accept, withdraw, role, remove, and transfer. Invite returns a
private invitation_file reference for a separately authorized handoff. Accept
requires that file and the exact reviewed workspace. No command delivers an
invitation. Unknown effects require inspection; changing an ID cannot retry
the same unresolved effect. Switching preserves previous purchase attribution.
Existing customer change and inspect commands retain account recovery custody.";
fn required<'a>(args: &'a Args, name: &str) -> Result<&'a str, String> {
    args.option(name)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("--{name} is required"))
}
pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(a) => a,
        Err(e) => return output.usage("customer team", &e, USAGE),
    };
    let Some(command) = args
        .positional()
        .first()
        .filter(|_| args.positional().len() == 1)
    else {
        return output.usage("customer team", "Select one team command.", USAGE);
    };
    let allowed: &[&str] = match command.as_str() {
        "change" => &["root", "input", "invitation"],
        "members" => &["root", "workspace"],
        "switch" => &["root", "workspace", "door", "alias"],
        "inspect" => &["root", "operation"],
        _ => return output.usage("customer team", "Unknown team command.", USAGE),
    };
    if args.option_names().iter().any(|n| !allowed.contains(n))
        || allowed
            .iter()
            .filter(|n| !matches!(**n, "invitation" | "alias"))
            .any(|n| required(&args, n).is_err())
    {
        return output.usage(
            "customer team",
            "Use declared options and private input files.",
            USAGE,
        );
    }
    match crate::runtime().block_on(execute(&args, command)) {
        Ok((value, applied)) => {
            output.emit(&value, |v| serde_json::to_string(v).unwrap_or_default());
            if applied { 0 } else { 1 }
        }
        Err(error) => output.fail("customer team", &error),
    }
}
async fn execute(args: &Args, command: &str) -> Result<(Value, bool), String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let value = match command {
        "change" => {
            let command: TeamCommand = serde_json::from_slice(&Store::private_input(
                Path::new(required(args, "input")?),
                8192,
            )?)
            .map_err(|_| "Invalid private team intent.")?;
            let invitation = args
                .option("invitation")
                .map(|p| Store::private_input(Path::new(p), 8192))
                .transpose()?;
            let view = store.change_team(command, invitation).await?;
            let applied = view.status == TeamStatus::Applied;
            return Ok((
                serde_json::to_value(view).map_err(|_| "Invalid team view.")?,
                applied,
            ));
        }
        "members" => store.team_members(required(args, "workspace")?).await?,
        "switch" => json!(
            store
                .switch_team(
                    required(args, "workspace")?,
                    required(args, "door")?,
                    args.option("alias")
                )
                .await?
        ),
        "inspect" => json!(store.inspect_team(required(args, "operation")?).await?),
        _ => return Err("Unknown team command.".into()),
    };
    Ok((value, true))
}
