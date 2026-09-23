//! `verify.repair` as a component: one preserved mini-task candidate and
//! one repair arm on an isolated copy of it, with the scripted executor.

use std::time::Duration;

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{Component, Fixture, Ran, input};
use crate::component::jev::JevMode;
use crate::record::{Implementation, Recorder};
use crate::repair::study::{Arm, cell, preserve_one};
use crate::repair::{BriefKind, Profile, Trigger, implementation};

#[derive(Deserialize)]
struct RepairArm {
    brief: BriefKind,
    profile: String,
}

#[derive(Deserialize)]
struct Expect {
    after: String,
    #[serde(default)]
    triggered: Option<bool>,
}

#[derive(Deserialize)]
struct RepairInput {
    task: String,
    variant: String,
    /// The arm; `null` is no repair.
    arm: Option<RepairArm>,
    trigger: Trigger,
    expect: Expect,
}

/// `verify.repair`: a repair arm on a preserved candidate.
pub struct Repair;

impl Component for Repair {
    fn id(&self) -> &'static str {
        "verify.repair"
    }
    fn implementation(&self) -> Implementation {
        implementation(BriefKind::Packet, Trigger::Detected)
    }
    fn about(&self) -> &'static str {
        "One fresh session repairs the candidate from the diagnostic packet, then the checks rerun."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let given: RepairInput = input(fixture)?;
            let task = crate::minitask::find(&given.task)?;
            let scratch = std::env::temp_dir().join(format!(
                "coder-one-component-repair-{}-{}",
                std::process::id(),
                atif::now_ms()
            ));
            let preserved = preserve_one(&task, &given.variant, &scratch.join("preserved")).await?;
            let arm = Arm {
                name: if given.arm.is_some() {
                    "repair"
                } else {
                    "none"
                },
                repair: match &given.arm {
                    Some(arm) => Some((arm.brief, Profile::parse(&arm.profile)?)),
                    None => None,
                },
            };
            let row = cell(
                &preserved,
                &arm,
                given.trigger,
                Duration::from_secs(60),
                &scratch.join("arm"),
            )
            .await?;
            let _ = std::fs::remove_dir_all(&scratch);
            let after = match row["after"].as_bool() {
                Some(true) => "pass",
                Some(false) => "fail",
                None => "unavailable",
            };
            let mut metrics = Map::new();
            for key in ["recovered", "damaged", "triggered", "changed"] {
                metrics.insert(key.to_string(), row[key].clone());
            }
            metrics.insert("cost_usd".to_string(), row["cost_usd"].clone());
            let python = crate::minitask::process::python().is_some();
            metrics.insert(
                "matches_expected".to_string(),
                if python {
                    json!(
                        after == given.expect.after
                            && given
                                .expect
                                .triggered
                                .is_none_or(|t| row["triggered"] == json!(t))
                    )
                } else {
                    Value::Null
                },
            );
            let repair = &row["repair"];
            Ok(Ran {
                output: json!({
                    "candidate": row["candidate"],
                    "before": row["before"],
                    "after": row["after"],
                    "triggered": row["triggered"],
                    "changed": row["changed"],
                    "brief": repair["brief"].get("kind").cloned().unwrap_or(Value::Null),
                    "brief_requirements": repair["brief"].get("requirements").cloned().unwrap_or(Value::Null),
                    "brief_scenarios": repair["brief"].get("scenarios").cloned().unwrap_or(Value::Null),
                    "session_fresh": repair["session"].get("fresh").cloned().unwrap_or(Value::Null),
                    "skipped": repair.get("skipped").cloned().unwrap_or(Value::Null),
                    "recheck_packets": repair.pointer("/recheck/summary/packets").cloned().unwrap_or(Value::Null),
                }),
                metrics,
            })
        })
    }
}
