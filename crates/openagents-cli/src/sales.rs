//! Private sales records and separately approved native outbound dispatch.
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::sales::{Role, Store};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
#[path = "sales_agents.rs"]
pub(crate) mod agents;
#[path = "sales_claims.rs"]
mod claims;
#[path = "sales_email.rs"]
mod email;
#[path = "sales_floor.rs"]
mod floor;
#[path = "sales_meetings.rs"]
mod meetings;
#[path = "sales_models.rs"]
mod models;
#[path = "sales_outbox.rs"]
mod outbox;
#[path = "sales_paul.rs"]
pub(crate) mod paul;
#[path = "sales_privacy.rs"]
mod privacy;
#[path = "sales_qualification.rs"]
mod qualification;
#[path = "sales_replies.rs"]
mod replies;
#[path = "sales_town.rs"]
mod town;
#[path = "sales_training.rs"]
mod training;
pub const USAGE: &str = "usage: openagents sales COMMAND --root DIR [--credential FILE] [--json]
  init --owner HUMAN --credential FILE
        Initialize the private pipeline and write its owner's credential.
  issue --human HUMAN --role writer|reader --new-credential FILE
        Grant a named human private record access; prints no credential.
  revoke --human HUMAN
        Revoke that human's credential and cancel proposed handoffs to them.
  apply --input FILE [--evidence-root DIR]
        Apply a bounded versioned JSON command; FILE=- reads stdin.
  list [--after LEAD] [--limit N]
        List only records this credential can read, at most 100.
  show --lead LEAD [--sale SALE | --assignment ID | --journey JOURNEY | --proposal FILE]
        Read one authorized private lead/account record.
        --proposal computes the owner's exact proposal digest before approval.
  export --lead LEAD --output FILE [--sale SALE | --assignment ID | --journey JOURNEY]
        Create a private exclusive JSON export; no shared/public export.
  audit [--after N] [--limit N]
        Read the owner's bounded digest-only audit references.
  suppressed --contact CHANNEL:ADDRESS
        Inspect minimum suppression before an authorized future contact.
  privacy view
        Read owner-only policy, opaque suppression, and native copy cleanup.
  privacy apply --input FILE
        Record owner business admission or immediate human opt-out.
  privacy check --lead LEAD --channel email
        Check current contact admission; grants no outbound authority.
  privacy prune
        Apply due native retention and registered copy cleanup.
  email view
        Read owner-declared mailbox configuration and evidence.
  email apply --input FILE
        Configure or revoke a restricted host mailbox handle.
  email check --input FILE --mailbox-key FILE
        Prepare a private message; grants no dispatch authority.
  email evidence --input FILE --message-sha256 SHA
        Map provider acceptance, delivery, bounce, failure, or uncertainty.
  outbox view
        Read exact owner approval subjects and consumed attempt states.
  outbox propose --input FILE --mailbox-key FILE
        Reserve counts and freeze exact private recipient, content, and authority.
  outbox apply --input FILE [--mailbox-key FILE]
        Approve an exact subject, reject, pause, or review restart.
  outbox fixture --proposal ID --subject-sha256 SHA --input FILE --mailbox-key FILE
        Consume a fixture approval with isolated synthetic provider evidence.
  outbox dispatch --proposal ID --subject-sha256 SHA --mailbox-key FILE
        Consume one live approval through the qualified native SMTP adapter.
  replies view
        Read private reply provenance, safety findings, and owner classifications.
  replies ingest --input FILE
        Import quoted inbox data with immediate native contact safety.
  replies review --input FILE
        Record one exact owner classification; grants no outbound authority.
  replies qualify --expires-at UNIX
        Retain measured native scratch-injection safety results.
  replies revoke --qualification SHA
        Revoke current handler qualification and pause outbound work.
  replies booking --input FILE
        Prepare a human meeting from exact owner-reviewed interested reply.
  replies follow-up --lead LEAD --mode fixture|live
        Retain a no-response plan with real-week spacing and two-attempt bound.
  agents anchor --agent NAME
        Read the owner's exact native agent key and charter pins.
  agents owner
        Read owner-recorded policies and manual certification references.
  agents owner-apply --input FILE [--new-credential FILE]
        Record owner policy, assigned access, draft review, or certification.
  agents policy-check --input FILE
        Check and digest an explicit owner policy.
  agents read
        Read only the assigned lead's granted fields under an agent credential.
  agents apply --input FILE
        Apply a field-scoped change or propose a private draft.
  agents memory
        Read current opaque references and fixed nonidentifying summary fields.
  floor report
  floor escalations
  floor weekly-draft
  town table
  town bodies
  meetings slot --input FILE --expected-version N
        Publish explicit finite owner availability.
  meetings slots
        Read current published slots under assigned native agent access.
  meetings queue
        Read only current opaque assigned-agent proposal references.
  meetings list
        Read only the owner's or named human's bounded private briefs.
  meetings propose --input FILE
        Prepare the owner's bounded private meeting brief.
  meetings recommend --meeting ID --revision N --slot ID --slot-version N
        Recommend a published slot; requires a new owner confirmation.
  meetings confirm --meeting ID --revision N --approve SHA256 --input FILE
        Confirm the exact proposal as owner; input is a private JSON reference.
  meetings accept --meeting ID --revision N --approve SHA256 --input FILE
        Accept the bounded assignment only as the named human.
  meetings decline --meeting ID --revision N --approve SHA256 --input FILE
        Decline the bounded assignment only as the named human.
  meetings show --meeting ID
        Read only the owner's or named human's private brief.
  models policy-check --input FILE
        Check finite source prices and floor limits under owner access.
  models policy --input FILE --approve SHA256
        Record the exact owner-approved expense policy digest.
  models settle --reservation ID --input FILE
        Reconcile original estimates and provider bills; unknown holds remain.
  models show --reservation ID
        Read an original expense receipt, including after agent retirement.
  models history [--after ID] [--limit N]
        Read bounded owner-only expense attribution without customer text.
  claims source --input FILE
        Review an immutable source over its explicit current source root.
  claims review --input FILE
        Review one versioned claim; retain unavailable or rejected decisions.
  claims read --input FILE --release COMMIT
        Validate 1 to 8 claim pins against current evidence, prices, and release.
  claims draft --input FILE --draft ID --release COMMIT
        Compose only reviewed clauses, limits, and structured price/evidence.
  claims validate --draft ID --release COMMIT
        Revalidate an immutable draft; changed or withdrawn sources refuse.
  claims withdraw --input FILE
        Withdraw a source or claim revision with a retained reason reference.
  claims history [--after N] [--limit N]
        Read the owner's bounded claim decision history.
  claims helper-source --input FILE
        Read a reviewed local helper source for the shared model policy.
  claims helper --input FILE --request ID --agent-credential FILE
        Retain bounded cited claims, prices, and recommendation with admitted cost.
  claims helper-show --reference ID
        Read the original private helper result and expense reference.
  qualification questions --dimension claims|compliance|tone
        Read pinned advisory questions without invented decision cuts.
  qualification publish --input FILE
        Retain original owner-labeled suites and a priced source.
  qualification show --package ID
        Read original measurements and frozen package evidence.
  qualification freeze --package ID
        Freeze successful original calibration and development rows.
  qualification accept --reference ID --grade ID --input FILE
        Record an owner review of an original passing sample.
  qualification certify --input FILE
        Require original passing practices and owner-reviewed samples.
  qualification complaint --reference ID --expense ID --input FILE
        Attribute an original real-draft complaint and suspend its actor.
  paul owner-view
  paul propose-draft --requester DEVICE --request ID --input FILE
  paul source
        Read the bounded native verification source; no model availability is implied.
  paul binding-check --input FILE
        Read the digest of an explicit private controller binding.
  paul configure --input FILE --approve SHA256
        Approve the next controller, requester, and native assignment binding.
  paul pipeline --requester DEVICE --request ID
        Read the current admitted queue with its original expense receipt.
  paul research --requester DEVICE --request ID --input FILE
        Retain exact reviewed research and its original expense reference.
  paul practice --requester DEVICE
        Read original opaque synthetic run and expense references.
  training personas
        Read labeled fictional buyer situations.
  training script-source
        Read the exact zero-cost fixture model source.
  training schedule --input FILE
        Retain a bounded practice with exact native identity and reviewed evidence.
  training run-scripted --run ID
        Run an original synthetic fixture once; completion grants no certification.
  training show --run ID
        Read partial practice evidence and original expense references.
  training list --agent NAME [--after ID] [--limit N]
        Read a bounded synthetic schedule through current Paul authority.
  weekly --input MANIFEST --evidence-root DIR --output FILE
        Recheck consented journeys and economics into a private weekly report.
  review --input MANIFEST --evidence-root DIR --report FILE --review FILE --output FILE
        Write owner-reviewed delayed counts; this publishes nothing.

All commands require an explicit private host root. Except init, read the
current human's credential from FILE; do not put its secret on the command
line. Propose a handoff through apply with that lead's current revision;
only the named target's credential can accept it. Generic pipeline operations grant no
outbound, provider, execution, or customer-data disclosure authority. The separate
outbox requires exact owner approval and current native prerequisites for SMTP.
Only the owner can record_service_sale, reconcile_service_payment, or
reconcile_service_fulfillment through apply with --evidence-root DIR.
Use --sale with show/export for the original authorized service scope.
Use --journey for separately consented original funnel scope. Owner-only
weekly/review operations recheck current custody and source evidence.
These records send no invoice or payment and create no product credit.
The owner can record_acquisition with accounts_directory naming the canonical
account service's private directory. It verifies the lead's exact account source
and preserves consent refusal or missing attribution; intake text stays unverified.
Only the owner can propose_partner with an exact owner approval and
--evidence-root. advance_partner uses the named recipient's credential for
acceptance/refusal and the named handoff target's credential for its decision.
Pending invitations contain no private brief or lead-read grant. Changed terms
need a new proposal and approval; commission references establish no earnings.";
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
    Declared::computer("privacy view", Effect::ReadOnly),
    Declared::computer("privacy apply", Effect::Grants),
    Declared::computer("privacy check", Effect::ReadOnly),
    Declared::computer("privacy prune", Effect::LocalWrite),
    Declared::computer("email view", Effect::ReadOnly),
    Declared::computer("email apply", Effect::Grants),
    Declared::computer("email check", Effect::LocalWrite),
    Declared::computer("email evidence", Effect::ReadOnly),
    Declared::computer("outbox view", Effect::ReadOnly),
    Declared::computer("outbox propose", Effect::LocalWrite),
    Declared::computer("outbox apply", Effect::Grants),
    Declared::computer("outbox fixture", Effect::LocalWrite),
    Declared::computer("outbox dispatch", Effect::Publishes),
    Declared::computer("replies view", Effect::ReadOnly),
    Declared::computer("replies ingest", Effect::Grants),
    Declared::computer("replies review", Effect::Grants),
    Declared::computer("replies qualify", Effect::LocalWrite),
    Declared::computer("replies revoke", Effect::Grants),
    Declared::computer("replies booking", Effect::LocalWrite),
    Declared::computer("replies follow-up", Effect::LocalWrite),
    Declared::computer("agents anchor", Effect::ReadOnly),
    Declared::computer("agents owner", Effect::ReadOnly),
    Declared::computer("agents owner-apply", Effect::Grants),
    Declared::computer("agents policy-check", Effect::ReadOnly),
    Declared::computer("agents read", Effect::ReadOnly),
    Declared::computer("agents apply", Effect::LocalWrite),
    Declared::computer("agents memory", Effect::ReadOnly),
    Declared::computer("paul source", Effect::ReadOnly),
    Declared::computer("paul owner-view", Effect::ReadOnly),
    Declared::computer("paul propose-draft", Effect::LocalWrite),
    Declared::computer("paul binding-check", Effect::ReadOnly),
    Declared::computer("paul configure", Effect::Grants),
    Declared::computer("paul pipeline", Effect::LocalWrite),
    Declared::computer("paul research", Effect::LocalWrite),
    Declared::computer("paul practice", Effect::ReadOnly),
    Declared::computer("floor report", Effect::ReadOnly),
    Declared::computer("floor escalations", Effect::ReadOnly),
    Declared::computer("floor weekly-draft", Effect::ReadOnly),
    Declared::computer("town table", Effect::ReadOnly),
    Declared::computer("town bodies", Effect::ReadOnly),
    Declared::computer("meetings slot", Effect::Grants),
    Declared::computer("meetings slots", Effect::ReadOnly),
    Declared::computer("meetings queue", Effect::ReadOnly),
    Declared::computer("meetings list", Effect::ReadOnly),
    Declared::computer("meetings propose", Effect::LocalWrite),
    Declared::computer("meetings recommend", Effect::LocalWrite),
    Declared::computer("meetings confirm", Effect::Grants),
    Declared::computer("meetings accept", Effect::Grants),
    Declared::computer("meetings decline", Effect::Grants),
    Declared::computer("meetings show", Effect::ReadOnly),
    Declared::computer("models policy-check", Effect::ReadOnly),
    Declared::computer("models policy", Effect::Grants),
    Declared::computer("models settle", Effect::LocalWrite),
    Declared::computer("models show", Effect::ReadOnly),
    Declared::computer("models history", Effect::ReadOnly),
    Declared::computer("claims source", Effect::Grants),
    Declared::computer("claims review", Effect::Grants),
    Declared::computer("claims read", Effect::LocalWrite),
    Declared::computer("claims draft", Effect::LocalWrite),
    Declared::computer("claims validate", Effect::LocalWrite),
    Declared::computer("claims withdraw", Effect::Grants),
    Declared::computer("claims history", Effect::ReadOnly),
    Declared::computer("claims helper-source", Effect::ReadOnly),
    Declared::computer("claims helper", Effect::LocalWrite),
    Declared::computer("claims helper-show", Effect::ReadOnly),
    Declared::computer("qualification questions", Effect::ReadOnly),
    Declared::computer("qualification publish", Effect::LocalWrite),
    Declared::computer("qualification show", Effect::ReadOnly),
    Declared::computer("qualification freeze", Effect::LocalWrite),
    Declared::computer("qualification accept", Effect::LocalWrite),
    Declared::computer("qualification certify", Effect::Grants),
    Declared::computer("qualification complaint", Effect::Grants),
    Declared::computer("training personas", Effect::ReadOnly),
    Declared::computer("training script-source", Effect::ReadOnly),
    Declared::computer("training schedule", Effect::LocalWrite),
    Declared::computer("training run-scripted", Effect::LocalWrite),
    Declared::computer("training show", Effect::ReadOnly),
    Declared::computer("training list", Effect::ReadOnly),
    Declared::computer("weekly", Effect::LocalWrite),
    Declared::computer("review", Effect::LocalWrite),
];
pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.first().is_some_and(|w| w == "qualification") {
        return qualification::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "paul") {
        return paul::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "replies") {
        return replies::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "outbox") {
        return outbox::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "training") {
        return training::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "floor") {
        return floor::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "town") {
        return town::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "meetings") {
        return meetings::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "email") {
        return email::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "models") {
        return models::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "privacy") {
        return privacy::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "agents") {
        return agents::run(output, &words[1..]);
    }
    if words.first().is_some_and(|w| w == "claims") {
        return claims::run(output, &words[1..]);
    }
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "weekly" | "review"))
    {
        return crate::sales_weekly::run(output, words);
    }
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
        "apply" => &["root", "credential", "input", "evidence-root"],
        "list" | "audit" => &["root", "credential", "after", "limit"],
        "show" => &[
            "root",
            "credential",
            "lead",
            "sale",
            "assignment",
            "proposal",
            "journey",
        ],
        "export" => &[
            "root",
            "credential",
            "lead",
            "output",
            "sale",
            "assignment",
            "journey",
        ],
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
    if ["sale", "assignment", "proposal", "journey"]
        .iter()
        .filter(|name| args.option(name).is_some())
        .count()
        > 1
    {
        return Err("Select one sales record scope.".into());
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
    if args.option("sale").is_some() && args.option("journey").is_some() {
        return Err("choose one private sale or journey scope".into());
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
            serde_json::to_value(store.apply_with_evidence_root(
                &access,
                &bytes,
                args.option("evidence-root").map(Path::new),
            )?)
            .map_err(|e| e.to_string())?
        }
        "list" => json!({"records":store.list(&access,args.option("after"),page(&args)?)?}),
        "show" => if let Some(assignment) = args.option("assignment") {
            Ok(store.partner_show(&access, required(&args, "lead")?, assignment)?)
        } else if let Some(path) = args.option("proposal") {
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .map_err(|_| "Partner proposal is unavailable.")?
                .take(32 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Partner proposal read failed.")?;
            if bytes.len() > 32 * 1024 {
                return Err("Partner proposal exceeds its input bound.".into());
            }
            let proposal =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid partner proposal JSON.")?;
            Ok(store.partner_digest(&access, required(&args, "lead")?, &proposal)?)
        } else if let Some(sale) = args.option("sale") {
            serde_json::to_value(store.service_show(&access, required(&args, "lead")?, sale)?)
        } else if let Some(journey) = args.option("journey") {
            serde_json::to_value(store.funnel_show(&access, required(&args, "lead")?, journey)?)
        } else {
            serde_json::to_value(store.show(&access, required(&args, "lead")?)?)
        }
        .map_err(|e| e.to_string())?,
        "export" => {
            let path = Path::new(required(&args, "output")?);
            let sha = if let Some(assignment) = args.option("assignment") {
                store.partner_export(&access, required(&args, "lead")?, assignment, path)?
            } else if let Some(sale) = args.option("sale") {
                store.service_export(&access, required(&args, "lead")?, sale, path)?
            } else if let Some(journey) = args.option("journey") {
                store.funnel_export(&access, required(&args, "lead")?, journey, path)?
            } else {
                store.export(&access, required(&args, "lead")?, path)?
            };
            json!({"sha256":sha})
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
