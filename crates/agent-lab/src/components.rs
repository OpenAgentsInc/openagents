//! Operator-installed read-only Lab references. Documents contain only a name and pin.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Metadata read by a trusted source adapter for the exact inputs it consumes.
/// This value does not authenticate a source. Do not construct it from arbitrary
/// legacy tool responses or an experiment's permission flags.
pub struct ModelContextSource<'a> {
    pub version: &'a lab_kit::Version,
    pub artifacts: &'a [lab_kit::Artifact],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolBinding {
    pub id: String,
    pub name: String,
    pub account: String,
    pub project: String,
    pub origin: String,
    pub tool: ReadTool,
    pub arguments: Value,
    pub training_allowed: bool,
    pub private_local: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ReadTool {
    #[serde(rename = "lab_open")]
    Video,
    #[serde(rename = "cad_check")]
    Cad,
    #[serde(rename = "robot_inspect")]
    Robot,
}
impl ReadTool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Video => "lab_open",
            Self::Cad => "cad_check",
            Self::Robot => "robot_inspect",
        }
    }
}
impl ToolBinding {
    /// Check source policy before formatting any content for an evaluation.
    /// A source reader must establish access and supply all consumed artifacts.
    /// Legacy read tools have no such contract and must pass `None`.
    pub fn admit_model_context<'a>(
        &self,
        account: &str,
        project: &str,
        source: Option<ModelContextSource<'a>>,
    ) -> Result<ModelContextSource<'a>, lab_kit::Refusal> {
        let unavailable = || {
            lab_kit::Refusal::new(
                "lab_reference_unavailable",
                "Remove the Lab reference before running this experiment.",
            )
        };
        if !self.validate(account, project) {
            return Err(unavailable());
        }
        let source = source.ok_or_else(unavailable)?;
        let version = source.version;
        let document_key = if matches!(self.tool, ReadTool::Cad) {
            "doc"
        } else {
            "document"
        };
        let same_version = match self.tool {
            ReadTool::Video | ReadTool::Cad => {
                self.arguments["version"].as_u64() == Some(version.number)
            }
            // The legacy Robot identifier has no established mapping to a Lab Version.
            ReadTool::Robot => false,
        };
        if self.arguments[document_key].as_str() != Some(version.document.as_str())
            || !same_version
            || lab_kit::digest(&version.content) != version.content_sha256
        {
            return Err(unavailable());
        }
        lab_kit::check_model_use(version, source.artifacts, lab_kit::ModelUse::Evaluation)?;
        Ok(source)
    }

    pub fn digest(&self) -> String {
        lab_kit::digest(&serde_json::to_value(self).unwrap_or_default())
    }
    pub fn validate(&self, account: &str, project: &str) -> bool {
        let args = self.arguments.as_object();
        !self.private_local
            && self.training_allowed
            && self.account == account
            && self.project == project
            && super::short(&self.id, 80)
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            && super::short(&self.name, 80)
            && args.is_some_and(|a| {
                let doc = if matches!(self.tool, ReadTool::Cad) {
                    "doc"
                } else {
                    "document"
                };
                let allowed = match self.tool {
                    ReadTool::Video => vec![doc, "version", "render"],
                    ReadTool::Cad => vec![doc, "version"],
                    ReadTool::Robot => vec![doc, "version", "frame"],
                };
                a.keys().all(|k| allowed.contains(&k.as_str()))
                    && a.get(doc)
                        .and_then(Value::as_str)
                        .is_some_and(|s| super::short(s, 128))
                    && match self.tool {
                        ReadTool::Video => {
                            a.get("version")
                                .and_then(Value::as_u64)
                                .is_some_and(|n| n > 0)
                                && a.get("render") == Some(&Value::Bool(false))
                        }
                        ReadTool::Cad => a
                            .get("version")
                            .and_then(Value::as_u64)
                            .is_some_and(|n| n > 0),
                        ReadTool::Robot => {
                            a.get("version")
                                .and_then(Value::as_str)
                                .is_some_and(agent_contract::valid_id)
                                && a.get("frame")
                                    .and_then(Value::as_u64)
                                    .is_some_and(|n| n <= u32::MAX as u64)
                        }
                    }
            })
    }
}

/// Deterministic checks for the structured evidence an agent returns from another Lab.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "lab", rename_all = "snake_case", deny_unknown_fields)]
pub enum LabTaskCheck {
    Video { expected_version: u64 },
    Cad,
    Robot,
}
impl LabTaskCheck {
    pub fn check(&self) -> super::Check {
        match self {
            Self::Video { expected_version } => super::Check::Json {
                pointer: "/card/version".into(),
                expected: serde_json::json!(expected_version),
            },
            Self::Cad => super::Check::Json {
                pointer: "/passed".into(),
                expected: serde_json::json!(true),
            },
            Self::Robot => super::Check::Json {
                pointer: "/step/limit_violations".into(),
                expected: serde_json::json!([]),
            },
        }
    }
}
