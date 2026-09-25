use super::*;
use crate::checks::contract::host::{Memory, Ran};
use std::time::Duration;

#[test]
fn recognizes_literal_quote_styles_and_bare_outputs() {
    let p = plan(
        "fixture",
        "Create a CSV file called '/app/result.csv'.\nWrite \"/app/other.json\".\nSave `/app/third.bin`.\nWrite me data.comp that's compressed.",
        "/app",
    );
    assert_eq!(p.obligations.len(), 4);
    assert!(p.obligations.iter().any(|x| x.path == "/app/result.csv"));
    assert!(p.obligations.iter().any(|x| x.path == "/app/data.comp"));
    p.verify().unwrap();
    let roundtrip: Plan = serde_json::from_value(json!(&p)).unwrap();
    assert_eq!(roundtrip, p);
    roundtrip.verify().unwrap();
}

#[test]
fn refuses_optional_conditional_reference_and_unsafe_contexts() {
    for text in [
        "Do not create '/app/no.csv'.",
        "You may write '/app/no.csv'.",
        "If needed, create `/app/no.csv`.",
        "Read '/app/input.csv'.",
        "Write a summary mentioning '/app/input.csv'.",
        "Write either '/app/first.csv' or '/app/second.csv'.",
        "Generate at least one sample file named \"/app/normal.txt\" or \"/app/exponential.txt\" containing your samples.",
        "Write a function that creates '/app/later.csv'.",
        "Write a function to generate '/app/later.csv'.",
        "Write a script for saving '/app/later.csv'.",
        "Write code to output '/app/later.csv'.",
        "Write a command `echo data > /app/later.csv`.",
        "Write the contents of `/app/input.txt` to `/app/output.txt`.",
        "Run `python -c 'write /app/later.csv'`.",
        "For example:\nWrite '/app/no.csv'.",
        "# Examples\n\nWrite '/app/no.csv'.",
        "If a report is wanted:\n\n- Write '/app/no.csv'.",
        "```sh\nwrite /app/no.csv\n```",
        "Write '/app/../outside.csv'.",
        "Write '/other/out.csv'.",
        "Write '<output.csv>'.",
        "Write https://example.com/out.csv.",
    ] {
        assert!(
            plan("fixture", text, "/app").obligations.is_empty(),
            "{text}"
        );
    }
}

#[test]
fn a_new_heading_ends_an_example_section() {
    let p = plan(
        "fixture",
        "# Examples\n\nWrite '/app/no.csv'.\n\n# Required outputs\n\nCreate '/app/yes.csv'.",
        "/app",
    );
    assert_eq!(p.obligations.len(), 1);
    assert_eq!(p.obligations[0].path, "/app/yes.csv");
}

#[test]
fn final_obligations_exclude_temporary_removed_and_relocated_artifacts() {
    for text in [
        "Write scratch.txt as an intermediate file, then delete it before finishing.",
        "Create a temporary file scratch.txt.",
        "Write scratch.txt. Delete scratch.txt when finished.",
        "Write scratch.txt. Remove it before finishing.",
        "Write /app/work/result.txt. Remove /app/work when finished.",
        "Write scratch.txt. Rename scratch.txt to result.txt.",
        "Write scratch.txt. Move it to result.txt.",
        "Write scratch.txt. Clean up with:\n```sh\nrm scratch.txt\n```",
        "Write scratch.txt. scratch.txt must be at most 4 bytes. Delete scratch.txt.",
    ] {
        assert!(
            plan("fixture", text, "/app").obligations.is_empty(),
            "{text}"
        );
    }
    for text in [
        "Write scratch.txt. Write result.txt. Delete scratch.txt.",
        "Write result.txt. Delete /app/old.txt.",
        "Write result.txt. Delete /tmp/old.txt.",
        "Delete /app/old.txt. Write result.txt.",
        "Move all inputs to /app/inputs/. Write result.txt.",
        "Write scratch.txt. Move it to /app/inputs/. Write result.txt.",
    ] {
        let p = plan("fixture", text, "/app");
        assert_eq!(p.obligations.len(), 1, "{text}");
        assert_eq!(p.obligations[0].path, "/app/result.txt", "{text}");
    }
}

#[test]
fn limits_need_an_independent_output_obligation_and_exact_units() {
    for (text, maximum) in [
        (
            "Write data.comp. data.comp must be at most 2,500 bytes.",
            Some(2500),
        ),
        (
            "Write data.comp. data.comp must be less than 2500 bytes.",
            Some(2499),
        ),
        (
            "Write data.comp. You can generate data.comp any way you want, but data.comp must be at most 2500 bytes.",
            Some(2500),
        ),
        (
            "Write data.comp. data.comp must be at most 0 bytes.",
            Some(0),
        ),
        (
            "Write data.comp. data.comp must be less than 0 bytes.",
            None,
        ),
        (
            "Write data.comp. data.comp must be at most 25,00 bytes.",
            None,
        ),
        ("Write data.comp. data.comp must be at most 2 KiB.", None),
        (
            "Write data.comp. If needed, data.comp must be at most 25 bytes.",
            None,
        ),
        (
            "Write data.comp. If compressed, you can use a tool, but data.comp must be at most 25 bytes.",
            None,
        ),
    ] {
        let p = plan("fixture", text, "/app");
        let bounds: Vec<_> = p
            .obligations
            .iter()
            .filter_map(|o| match o.requirement {
                Requirement::MaxBytes { maximum } => Some(maximum),
                Requirement::Exists => None,
            })
            .collect();
        assert_eq!(bounds, maximum.into_iter().collect::<Vec<_>>(), "{text}");
    }
    assert!(
        plan(
            "fixture",
            "The input data.comp must be at most 2500 bytes.",
            "/app"
        )
        .obligations
        .is_empty()
    );
}

#[tokio::test]
async fn missing_and_oversized_outputs_fail_but_boundary_and_empty_plans_abstain() {
    let p = plan(
        "fixture",
        "Write out.bin. out.bin must be at most 4 bytes.",
        "/app",
    );
    for (contents, call) in [
        (None, Some("fail")),
        (Some(vec![1; 5]), Some("fail")),
        (Some(vec![1; 4]), None),
        (Some(vec![]), None),
    ] {
        let mut host = Memory::default();
        if let Some(bytes) = contents {
            host.files.insert("/app/out.bin".into(), bytes);
        }
        let r = run(&p, "candidate", &host).await.unwrap();
        assert_eq!(r["call"].as_str(), call);
    }
    let empty = plan("fixture", "Read the supplied source.", "/app");
    assert!(run(&empty, "candidate", &Memory::default()).await.unwrap()["call"].is_null());
}

struct StatOnly(Result<Stat, String>);
impl Host for StatOnly {
    fn describe(&self) -> Value {
        json!({"host":"stat-only fixture"})
    }
    async fn stat(&self, _: &str) -> Result<Stat, String> {
        self.0.clone()
    }
    async fn read(&self, _: &str, _: usize) -> Result<Option<Vec<u8>>, String> {
        panic!("literal checks must not read file contents")
    }
    async fn run(&self, _: &str, _: Duration) -> Ran {
        panic!("literal checks must not execute commands")
    }
}

#[tokio::test]
async fn metadata_is_sufficient_and_unreadable_evidence_abstains() {
    let p = plan(
        "fixture",
        "Write out.bin. out.bin must be at most 4 bytes.",
        "/app",
    );
    let large = run(&p, "candidate", &StatOnly(Ok(Stat::File(u64::MAX))))
        .await
        .unwrap();
    assert_eq!(large["call"], "fail");
    let unreadable = run(&p, "candidate", &StatOnly(Err("permission denied".into())))
        .await
        .unwrap();
    assert!(unreadable["call"].is_null());
    assert!(
        unreadable["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["outcome"]["outcome"] == "could_not_run")
    );
}

#[tokio::test]
async fn changed_plan_is_refused_before_any_host_access() {
    let mut p = plan("fixture", "Write out.bin.", "/app");
    p.obligations[0].path = "/other/out.bin".into();
    assert!(
        run(&p, "candidate", &StatOnly(Ok(Stat::File(0))))
            .await
            .is_err()
    );
    p.digest = p.computed_digest();
    assert!(p.verify().is_err());
}

#[test]
fn old_plan_readers_cannot_mistake_literal_matches_for_completion() {
    let p = plan("fixture", "Write out.bin.", "/app");
    assert!(serde_json::from_value::<crate::checks::contract::Plan>(json!(p)).is_err());
}

#[tokio::test]
async fn literal_cli_refuses_inference_and_unused_options_before_loading_files() {
    for args in [
        vec!["literal-plan", "--instruction", "/missing", "--jev", "live"],
        vec![
            "literal-plan",
            "--instruction",
            "/missing",
            "--plan",
            "/ignored",
        ],
        vec!["literal-run", "--plan", "/missing", "--task", "ignored"],
    ] {
        let args = args.into_iter().map(str::to_string).collect::<Vec<_>>();
        let error = crate::checks::contract::cli::command(&args)
            .await
            .unwrap_err();
        assert!(error.contains("documented file options"), "{error}");
    }
}
