//! Where `chat work` runs its issue flows: the placement every backend
//! shares, so the router sees each remote backend as one computer the
//! operator granted and never substitutes one for another.
//!
//! - `here`: this computer (no `--on`).
//! - `boat`: a Boat sandbox per issue (`chat_boat.rs`, #10220). Typing
//!   `--on boat` is the grant.
//! - `gce`: the GCE spot pool `openagents cloud up` granted (`cloud.rs`,
//!   `chat_gce.rs`, #10225). The grant is the pool record `cloud up` wrote;
//!   `cloud down` revokes it, and `--on gce` then refuses.
//!
//! Every remote run ends with a route record whose placement names the
//! computer and the grant ([`granted`]) and whose outcome carries the run's
//! wall time and cost ([`outcome`]).

use route_contract::lifecycle::{CheckLabel, Lifecycle, Projection};
use route_contract::record::RunOutcome;
use route_contract::snapshot::{GrantRef, GrantSource, Placement, WorkspaceBinding};

/// The computer `--on` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Here,
    Boat,
    Gce,
}

impl Target {
    /// `--on WORD`; absent is `here`.
    pub(super) fn parse(word: Option<&str>) -> Result<Self, String> {
        match word {
            None | Some("here") => Ok(Self::Here),
            Some("boat") => Ok(Self::Boat),
            Some("gce" | "cloud") => Ok(Self::Gce),
            Some(other) => Err(format!("--on is `here`, `boat`, or `gce`, not `{other}`")),
        }
    }

    /// The placement name the route record carries.
    pub(super) fn computer(self) -> &'static str {
        match self {
            Self::Here => "here",
            Self::Boat => "boat",
            Self::Gce => "gce",
        }
    }
}

/// The placement of a run on `computer` under the operator's grant `grant`
/// (revocation epoch `epoch`) for `repository`.
pub(super) fn granted(computer: &str, grant: String, epoch: u64, repository: &str) -> Placement {
    Placement {
        computer: Some(computer.to_owned()),
        workspace: Some(WorkspaceBinding {
            project: repository.to_owned(),
            path: None,
        }),
        grant: Some(GrantRef {
            id: grant,
            epoch,
            source: GrantSource::Operator,
        }),
    }
}

/// A finished remote run's projected outcome: completed and verified when
/// the issue flow landed (or opened its pull request), else failed.
pub(super) fn outcome(
    task: &str,
    outcome: &str,
    cost_microusd: Option<u64>,
    wall_ms: Option<u64>,
) -> RunOutcome {
    let landed = matches!(outcome, "landed" | "pull_request" | "queued");
    RunOutcome {
        task: task.to_owned(),
        engine: None,
        revision: None,
        projection: Projection {
            state: if landed {
                Lifecycle::Completed
            } else {
                Lifecycle::Failed
            },
            check: if landed {
                CheckLabel::Verified
            } else {
                CheckLabel::CheckFailed
            },
            cancel_requested: false,
        },
        cost_microusd,
        wall_ms,
        artifacts: Vec::new(),
        payer: None,
        payer_keys: Vec::new(),
    }
}

/// Dollars as micro-dollars, never negative.
pub(super) fn microusd(dollars: f64) -> u64 {
    (dollars * 1_000_000.0).round().max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_names_one_computer_and_never_another() {
        assert_eq!(Target::parse(None), Ok(Target::Here));
        assert_eq!(Target::parse(Some("here")), Ok(Target::Here));
        assert_eq!(Target::parse(Some("boat")), Ok(Target::Boat));
        assert_eq!(Target::parse(Some("gce")), Ok(Target::Gce));
        assert_eq!(Target::parse(Some("cloud")), Ok(Target::Gce));
        assert!(Target::parse(Some("anywhere")).is_err());
        assert_eq!(Target::Gce.computer(), "gce");
    }

    #[test]
    fn a_placement_carries_the_operator_grant() {
        let placement = granted("gce", "gce:p1".into(), 3, "o/r");
        let grant = placement.grant.unwrap();
        assert_eq!(placement.computer.as_deref(), Some("gce"));
        assert_eq!((grant.id.as_str(), grant.epoch), ("gce:p1", 3));
        assert_eq!(grant.source, GrantSource::Operator);
        let run = outcome("k1", "landed", Some(5), Some(9));
        assert_eq!(run.projection.state, Lifecycle::Completed);
        assert_eq!(
            outcome("k1", "failed", None, None).projection.check,
            CheckLabel::CheckFailed
        );
        assert_eq!(microusd(0.0152), 15_200);
    }
}
