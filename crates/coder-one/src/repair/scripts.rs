//! Repair scripts for the scripted executor: stand-ins for repair
//! profiles, so the repair's control flow and the recovery study run
//! without a model.
//!
//! | Profile | What the session does |
//! | --- | --- |
//! | `fix` | Writes the task's known-good solution, whatever the brief says. |
//! | `fix-if-packet` | Writes it only when the brief carries a diagnostic packet; otherwise claims the work is fine and changes nothing. |
//! | `claim-only` | Claims the work is fine and changes nothing. |
//! | `break` | Writes the task's known-bad solution. |

use crate::minitask::MiniTask;
use crate::scripted::{Act, Briefed, Script, Timed};

/// The repair profiles a scripted repair can play.
pub const PROFILES: &[&str] = &["fix", "fix-if-packet", "claim-only", "break"];

fn claim_only() -> Vec<Timed> {
    vec![
        Timed {
            at_ms: 0,
            act: Act::Claim {
                text: "Checked the work against the task; it already meets it.".to_string(),
            },
        },
        Timed {
            at_ms: 10,
            act: Act::End { error: false },
        },
    ]
}

/// The repair script `profile` for `task`, or `None` for an unknown one.
#[must_use]
pub fn script(task: &MiniTask, profile: &str) -> Option<Script> {
    let known = crate::minitask::scripts(task);
    let variant = |name: &str| {
        known
            .iter()
            .find(|(v, _)| *v == name)
            .map(|(_, s)| s.clone())
    };
    let mut script = match profile {
        "fix" | "fix-if-packet" => variant("good")?,
        "break" => variant("bad")?,
        "claim-only" => {
            let mut base = variant("good")?;
            base.events = claim_only();
            base
        }
        _ => return None,
    };
    if profile == "fix-if-packet" {
        script.briefed = Some(Box::new(Briefed {
            contains: super::PACKET_MARK.to_string(),
            otherwise: claim_only(),
        }));
    }
    script.name = format!("{}-repair-{profile}", task.id);
    Some(script)
}
