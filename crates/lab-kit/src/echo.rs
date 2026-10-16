//! `echo`: the smallest real kind, for tests of the store, the API, the SDK
//! and every surface's fixture renderer. Its documents are `echo.v1`
//! objects with a `text`; an empty text is refused; its only job, `shout`,
//! makes an upper-case text artifact; its card is a report.

use serde_json::{Value, json};

use crate::change::{Plain, apply_patch};
use crate::{
    Artifact, ArtifactRole, Card, Change, Draft, JobSpec, JobState, JobStatus, KindManifest,
    LabKind, Limits, Measurements, Refusal, Report, Requirements, ToolDecl, Version, View, Want,
};

pub struct Echo;

impl LabKind for Echo {
    const KIND: &'static str = "echo";

    fn manifest() -> KindManifest {
        KindManifest {
            kind: Self::KIND.into(),
            name: "Echo Lab".into(),
            schemas: vec!["echo.v1".into()],
            tool_prefix: "echo_".into(),
            doc_arg: "document".into(),
            tools: vec![ToolDecl {
                name: "echo_set".into(),
                description: "Set an echo document's text.".into(),
                parameters: json!({"type": "object", "properties": {
                    "document": {"type": "string"}, "text": {"type": "string"}},
                    "required": ["document", "text"], "additionalProperties": false}),
            }],
            job_types: vec!["shout".into()],
            views: vec![View::Report],
            limits: Limits::default(),
        }
    }

    fn check(content: &Value, _: &Measurements) -> Result<Report, Refusal> {
        let text = content
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err(Refusal::new("empty", "An echo needs some text."));
        }
        if let Some(extra) = content
            .as_object()
            .and_then(|o| o.keys().find(|k| *k != "text"))
        {
            return Err(Refusal::new(
                "unknown_field",
                format!("An echo doesn't take {extra}."),
            ));
        }
        Ok(Report {
            warnings: (text.len() > 280)
                .then(|| "That's a long echo.".to_owned())
                .into_iter()
                .collect(),
            facts: json!({"chars": text.chars().count()}),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        let content = match change {
            Change::Patch { ops } => apply_patch(&base.content, ops, &Plain)?,
            Change::Intent { intent } => match intent.get("intent").and_then(Value::as_str) {
                Some("set") => {
                    json!({"text": intent.get("text").and_then(Value::as_str).unwrap_or_default()})
                }
                _ => return Err(Refusal::new("unknown_intent", "An echo only takes `set`.")),
            },
        };
        let report = Self::check(&content, m)?;
        Ok(Draft {
            content,
            summary: summary.to_owned(),
            report,
            assets: Vec::new(),
        })
    }

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        let shout = |params: Value| JobSpec {
            kind: Self::KIND.into(),
            job_type: "shout".into(),
            version: version.reference(),
            params,
            requirements: Requirements::default(),
        };
        match want {
            Want::Card => vec![shout(Value::Null)],
            Want::Job { job_type, params } if job_type == "shout" => vec![shout(params.clone())],
            Want::Job { .. } => Vec::new(),
        }
    }

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        let made = artifacts
            .iter()
            .find(|a| a.role == ArtifactRole::Export && a.version == version.reference());
        let job = jobs.iter().find(|j| j.spec.version == version.reference());
        Card {
            schema: crate::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version.content["text"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(40)
                .collect(),
            summary: version.summary.clone(),
            state: if made.is_some() {
                Some(JobStatus::Done)
            } else {
                job.map(|j| j.status)
            },
            state_text: None,
            refusal: None,
            view: View::Report,
            primary: made.map(|a| a.id.clone()),
            poster: None,
            strip: Vec::new(),
            compare: None,
            notes: Vec::new(),
            page: None,
            private_local: version.has_private_local(),
            detail: version.report.facts.clone(),
        }
    }
}
