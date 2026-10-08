//! Owner control of original measurements and certification evidence.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::{agents::Artifact, qualification as q};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::path::Path;
const USAGE: &str =
    "usage: openagents sales qualification COMMAND --root DIR --credential FILE [--json]
  questions --dimension claims|compliance|tone
        Read pinned question wording; owner data must establish decision cuts.
  publish --input FILE
        Retain original labeled suites and their explicit priced source.
  show --package ID
        Read original package phases, frozen versions, and measurement receipts.
  freeze --package ID
        Freeze successful original calibration and development evidence.
  accept --reference ID --grade ID --input FILE
        Record an exact owner review artifact for a passing original sample.
  certify --input FILE
        Require ten passing practices and twenty distinct owner-reviewed samples.
  complaint --reference ID --expense ID --input FILE
        Attribute an original real-draft complaint and suspend its native actor.
All commands require current private owner access. Files are bounded and private.
Measurements and grading run through an explicitly admitted bounded host adapter.
No command grants sending authority or treats fixture success as provider quality.";
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(a) => a,
        Err(e) => return output.usage("sales qualification", &e, USAGE),
    };
    match execute(&args) {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(e) => output.fail("sales qualification", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "questions" => &["dimension"],
        "publish" | "certify" => &["input"],
        "show" | "freeze" => &["package"],
        "accept" => &["reference", "grade", "input"],
        "complaint" => &["reference", "expense", "input"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential") && !extra.contains(n))
    {
        return Err("unknown sales qualification option".into());
    }
    for name in ["root", "credential"].iter().chain(extra.iter()) {
        required(&args, name)?;
    }
    Ok(args)
}
fn input<T: DeserializeOwned>(args: &Args) -> Result<T, String> {
    serde_json::from_slice(&super::agents::input_bound(args, 2 * 1024 * 1024)?)
        .map_err(|_| "invalid sales qualification input".into())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Certify {
    id: String,
    package: String,
    marks: Vec<String>,
    owner_mark: Artifact,
}
fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let owner = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    store.sales_agent_owner_view(&owner)?;
    match args.positional()[0].as_str() {
        "questions" => {
            let dimension = match required(args, "dimension")? {
                "claims" => q::Dimension::Claims,
                "compliance" => q::Dimension::Compliance,
                "tone" => q::Dimension::Tone,
                _ => return Err("unknown sales qualification dimension".into()),
            };
            let questions = q::question_set(dimension)?;
            Ok(
                json!({"questions":questions,"sha256":questions.digest(),"authority":"advisory_only","decision_cuts":"owner_data_required"}),
            )
        }
        "publish" => serde_json::to_value(
            store.publish_sales_qualification(&owner, &input::<q::Candidate>(args)?)?,
        )
        .map_err(|e| e.to_string()),
        "show" => {
            serde_json::to_value(store.sales_qualification(&owner, required(args, "package")?)?)
                .map_err(|e| e.to_string())
        }
        "freeze" => serde_json::to_value(
            store.freeze_sales_qualification(&owner, required(args, "package")?)?,
        )
        .map_err(|e| e.to_string()),
        "accept" => serde_json::to_value(store.accept_sales_grade(
            &owner,
            required(args, "reference")?,
            required(args, "grade")?,
            &input::<Artifact>(args)?,
        )?)
        .map_err(|e| e.to_string()),
        "certify" => {
            let c: Certify = input(args)?;
            serde_json::to_value(store.certify_sales_agent(
                &owner,
                &c.id,
                &c.package,
                &c.marks,
                &c.owner_mark,
            )?)
            .map_err(|e| e.to_string())
        }
        _ => serde_json::to_value(store.record_sales_complaint(
            &owner,
            required(args, "reference")?,
            required(args, "expense")?,
            &input::<Artifact>(args)?,
        )?)
        .map_err(|e| e.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualification_operators_require_owner_forms_and_cannot_import_verdicts_or_send() {
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert!(
            parse(&words(
                "certify --root private --credential owner --input marks"
            ))
            .is_ok()
        );
        assert!(
            parse(&words(
                "freeze --root private --credential owner --package original"
            ))
            .is_ok()
        );
        assert!(
            parse(&words(
                "publish --root private --credential owner --input suites --probability 1"
            ))
            .is_err()
        );
        assert!(
            parse(&words(
                "certify --root private --credential owner --input marks --send"
            ))
            .is_err()
        );
        assert!(parse(&words("accept --grade original --input review")).is_err());
    }
}
