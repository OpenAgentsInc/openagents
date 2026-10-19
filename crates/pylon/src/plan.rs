//! `openagents pylon plan`: the ordered steps that get this computer
//! serving, built from what `doctor` read. Each step says who does it (an
//! `agent` may run it unattended; the `owner` must, because it needs their
//! judgement, key, or money destination), its download size, roughly how
//! long it takes, what it costs, and the command.

use serde::Serialize;

use crate::setup::{self, ALLOW_SLOW, CLEF_FLASH_BYTES, Report};

/// Who takes a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    Agent,
    Owner,
}

/// One step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    /// A stable name: `hardware`, `disk`, `model`, `engine`, `selfcheck`,
    /// `link`, `payout`, `online`.
    pub id: &'static str,
    pub who: Who,
    /// What it does, in plain words.
    pub does: String,
    /// What it downloads, when it downloads something.
    pub size: Option<String>,
    /// Roughly how long it takes.
    pub time: Option<String>,
    /// What it costs.
    pub cost: String,
    /// The command, when there is one.
    pub command: Option<String>,
    /// Already done on this computer.
    pub done: bool,
}

/// What `plan` knows beyond `doctor`'s report.
#[derive(Debug, Clone, Copy, Default)]
pub struct Facts {
    /// The pylon carries an owner link.
    pub linked: bool,
    /// A payout destination is set; `None` when this build can't tell.
    pub payout_set: Option<bool>,
}

/// The command that brings the pylon online.
pub const ONLINE: &str = "openagents pylon start --allow-any";

/// The steps, in order: hardware, disk, model, engine, self-check, link,
/// payout, online. Steps already done stay in the list marked `done`.
#[must_use]
pub fn steps(report: &Report, facts: Facts) -> Vec<Step> {
    let free = "free".to_string();
    let block = |fix: &str| report.blocks.iter().find(|b| b.fix == fix);
    let mut steps = Vec::new();
    if !report.fit.useful {
        steps.push(Step {
            id: "hardware",
            who: Who::Owner,
            does: format!(
                "Decide whether to serve on the CPU: {}. Callers in a hurry will skip a slow pylon.",
                report.fit.note
            ),
            size: None,
            time: None,
            cost: free.clone(),
            command: Some(ALLOW_SLOW.into()),
            done: block(ALLOW_SLOW).is_none(),
        });
    }
    let model_gb = CLEF_FLASH_BYTES as f64 / 1e9;
    if let Some(disk) = report.blocks.iter().find(|b| b.what.contains("GB free")) {
        steps.push(Step {
            id: "disk",
            who: Who::Owner,
            does: format!("Make room: {} ({})", disk.what, disk.fix),
            size: None,
            time: None,
            cost: free.clone(),
            command: None,
            done: false,
        });
    }
    steps.push(Step {
        id: "model",
        who: Who::Agent,
        does: "Download the pinned decision model (Clef-Flash Q4_K_M) and check its digest".into(),
        size: Some(format!("{model_gb:.1} GB")),
        time: Some("a few minutes on a fast connection".into()),
        cost: free.clone(),
        command: Some("openagents pylon setup".into()),
        done: report.model.verified,
    });
    steps.push(Step {
        id: "engine",
        who: Who::Agent,
        does: format!("Install the Psionic engine ({})", setup::SERVER),
        size: None,
        time: Some("about 15 minutes to build from source".into()),
        cost: free.clone(),
        command: Some(
            report
                .blocks
                .iter()
                .find(|b| b.what.contains(setup::SERVER))
                .map_or_else(|| setup::BUILD_SERVER.to_string(), |b| b.fix.clone()),
        ),
        done: report.server.is_some(),
    });
    let checked = report
        .selfcheck
        .as_ref()
        .is_some_and(|c| c.pass && c.model_id == setup::CLEF_FLASH_MODEL_ID);
    steps.push(Step {
        id: "selfcheck",
        who: Who::Agent,
        does: "Answer the shipped reference decisions on this computer and compare them with the reference".into(),
        size: None,
        time: Some("about a minute".into()),
        cost: free.clone(),
        command: Some("openagents pylon selfcheck".into()),
        done: checked,
    });
    steps.push(Step {
        id: "link",
        who: Who::Owner,
        does: "Link this pylon to your OpenAgents identity, so its work and earnings count as yours (your key is read once, never stored)".into(),
        size: None,
        time: Some("a minute".into()),
        cost: free.clone(),
        command: Some("openagents pylon link --owner-secret FILE".into()),
        done: facts.linked,
    });
    steps.push(Step {
        id: "payout",
        who: Who::Owner,
        does: "Choose where earnings go (this computer's OpenAgents wallet by default, or a Lightning address)".into(),
        size: None,
        time: Some("a minute".into()),
        cost: free.clone(),
        command: Some("openagents pylon payout set".into()),
        done: facts.payout_set == Some(true),
    });
    steps.push(Step {
        id: "online",
        who: Who::Agent,
        does: "Start the engine, self-check it, and start serving".into(),
        size: None,
        time: Some("under a minute once the model is loaded".into()),
        cost: free,
        command: Some(if report.fit.useful {
            ONLINE.into()
        } else {
            ALLOW_SLOW.into()
        }),
        done: false,
    });
    steps
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::setup::tests::{mac, passing, verified};
    use crate::setup::{Host, ModelState, report};

    fn ids(steps: &[Step]) -> Vec<&'static str> {
        steps.iter().map(|s| s.id).collect()
    }

    #[test]
    fn a_fresh_mac_plans_download_engine_check_link_payout_online() {
        let r = report(
            mac(32.0),
            ModelState {
                path: PathBuf::from("/m/Clef-Flash-Q4_K_M.gguf"),
                present: false,
                verified: false,
                model_id: None,
                sha256: None,
            },
            None,
            None,
            None,
            "wss://relay.openagents.com",
            true,
        );
        let steps = steps(&r, Facts::default());
        assert_eq!(
            ids(&steps),
            ["model", "engine", "selfcheck", "link", "payout", "online"]
        );
        assert!(steps.iter().all(|s| !s.done));
        assert_eq!(steps[0].size.as_deref(), Some("6.5 GB"));
        // Money and identity are the owner's; the rest an agent may run.
        for step in &steps {
            let owner = matches!(step.id, "link" | "payout");
            assert_eq!(step.who == Who::Owner, owner, "{}", step.id);
        }
    }

    #[test]
    fn a_ready_machine_marks_its_done_steps() {
        let r = report(
            mac(32.0),
            verified(),
            Some(PathBuf::from("/b/psionic-openai-server")),
            None,
            Some(passing()),
            "wss://relay.openagents.com",
            true,
        );
        let steps = steps(
            &r,
            Facts {
                linked: true,
                payout_set: Some(true),
            },
        );
        let done: Vec<_> = steps.iter().filter(|s| s.done).map(|s| s.id).collect();
        assert_eq!(done, ["model", "engine", "selfcheck", "link", "payout"]);
        assert_eq!(steps.last().unwrap().command.as_deref(), Some(ONLINE));
    }

    #[test]
    fn a_slow_machine_and_a_full_disk_come_first_for_the_owner() {
        let r = report(
            Host {
                gpu: "none".into(),
                disk_free_gb: Some(1.0),
                ..Host::default()
            },
            ModelState {
                path: PathBuf::from("/m/x.gguf"),
                present: false,
                verified: false,
                model_id: None,
                sha256: None,
            },
            None,
            None,
            None,
            "wss://relay.openagents.com",
            true,
        );
        let steps = steps(&r, Facts::default());
        assert_eq!(
            ids(&steps),
            [
                "hardware",
                "disk",
                "model",
                "engine",
                "selfcheck",
                "link",
                "payout",
                "online"
            ]
        );
        assert_eq!(steps[0].who, Who::Owner);
        assert_eq!(steps[1].who, Who::Owner);
        assert_eq!(steps.last().unwrap().command.as_deref(), Some(ALLOW_SLOW));
    }
}
