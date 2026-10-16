use super::*;
use lab_kit::{
    Artifact, Card, Change, Draft, JobSpec, JobState, KindManifest, Limits, Measurements, Report,
    Requirements, ToolDecl, Version, View, Want,
};
pub struct AgentLab;
impl LabKind for AgentLab {
    const KIND: &'static str = "agent";
    fn manifest() -> KindManifest {
        KindManifest {
            kind: "agent".into(),
            name: "Agent Lab".into(),
            schemas: vec![
                "agent-draft.v1".into(),
                "agent-suite.v1".into(),
                "agent-experiment.v1".into(),
                "agent-results.v1".into(),
            ],
            tool_prefix: "agent_lab_".into(),
            doc_arg: "document".into(),
            tools: vec![ToolDecl {
                name: "agent_lab_save".into(),
                description: "Save a checked agent experiment version without running it.".into(),
                parameters: json!({"type":"object","properties":{"document":{"type":"string"},"content":{"type":"object"}},"required":["document","content"],"additionalProperties":false}),
            }],
            job_types: vec!["trial".into()],
            views: vec![View::Report, View::Compare],
            limits: Limits {
                max_doc_bytes: 128 * 1024,
                max_docs_per_space: 64,
                max_versions_per_doc: 64,
                max_args_bytes: 128 * 1024,
                runs_per_hour: 32,
            },
        }
    }
    fn check(content: &Value, _: &Measurements) -> Result<Report, Refusal> {
        let c: Content = serde_json::from_value(content.clone())
            .map_err(|_| invalid("Check the Agent Lab document fields."))?;
        c.validate()?;
        Ok(Report {
            warnings: vec![],
            facts: json!({"valid":true,"executed":false}),
        })
    }
    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        m: &Measurements,
    ) -> Result<Draft, Refusal> {
        if !short(summary, 160) {
            return Err(invalid("Add a short change summary."));
        }
        let content = match change {
            Change::Patch { ops } => {
                lab_kit::apply_patch(&base.content, ops, &lab_kit::change::Plain)?
            }
            Change::Intent { intent } => intent
                .get("content")
                .cloned()
                .ok_or_else(|| invalid("Choose the document changes."))?,
        };
        let report = Self::check(&content, m)?;
        Ok(Draft {
            content,
            summary: summary.into(),
            report,
            assets: vec![],
        })
    }
    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec> {
        match want {
            Want::Job { job_type, params }
                if job_type == "trial" && Self::check(&version.content, &Value::Null).is_ok() =>
            {
                vec![JobSpec {
                    kind: "agent".into(),
                    job_type: "trial".into(),
                    version: version.reference(),
                    params: params.clone(),
                    requirements: Requirements {
                        max_seconds: 1800,
                        ..Default::default()
                    },
                }]
            }
            _ => vec![],
        }
    }
    fn card(version: &Version, jobs: &[JobState], _: &[Artifact]) -> Card {
        let title = version
            .content
            .pointer("/experiment/title")
            .and_then(Value::as_str)
            .unwrap_or("Agent Lab");
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: "agent".into(),
            document: version.document.clone(),
            version: version.number,
            title: title.into(),
            summary: version.summary.clone(),
            state: jobs
                .iter()
                .find(|j| j.spec.version == version.reference())
                .map(|j| j.status),
            state_text: None,
            refusal: None,
            view: View::Report,
            primary: None,
            poster: None,
            strip: vec![],
            compare: None,
            notes: vec![],
            page: Some(format!(
                "/labs/agent?document={}&version={}",
                version.document, version.number
            )),
            private_local: version.has_private_local(),
            detail: version.report.facts.clone(),
        }
    }
}
