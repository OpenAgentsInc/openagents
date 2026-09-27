//! Placement: a pure rule that picks a host for new work.
//!
//! It skips stale, incompatible, overloaded, and zero-weight hosts, and
//! scores the rest by weight × CPU count × idle fraction × available memory
//! fraction. It never moves existing work.

use crate::presence::{ClientProfile, Freshness, Received};

/// Thresholds that mark a host overloaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Skip hosts at or above this CPU utilization percentage.
    pub max_cpu_utilization_pct: u8,
    /// Skip hosts with less available memory than this percentage.
    pub min_memory_available_pct: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_cpu_utilization_pct: 90,
            min_memory_available_pct: 10,
        }
    }
}

/// One host a client could place work on.
#[derive(Clone, Debug)]
pub struct Candidate<'a> {
    pub host: &'a str,
    /// The owner's weight from the directory, or a client's local override.
    pub weight: u32,
    /// The newest accepted presence, if any.
    pub presence: Option<&'a Received>,
    /// Whether the client may start work on this host. A placement choice
    /// never substitutes for the host's own admission.
    pub admitted: bool,
}

/// Why a candidate was skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    NotAdmitted,
    ZeroWeight,
    NoPresence,
    Stale,
    Incompatible,
    NoTelemetry,
    Overloaded,
}

/// A candidate's placement outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assessment<'a> {
    Eligible { host: &'a str, score: u128 },
    Skipped { host: &'a str, reason: Skip },
}

/// Assess every candidate. Scores are exact integers:
/// `weight × cpu_count × (100 − utilization) × available_memory`.
#[must_use]
pub fn assess<'a>(
    candidates: &[Candidate<'a>],
    client: &ClientProfile,
    now: u64,
    freshness: Freshness,
    limits: Limits,
) -> Vec<Assessment<'a>> {
    candidates
        .iter()
        .map(|c| match score(c, client, now, freshness, limits) {
            Ok(score) => Assessment::Eligible {
                host: c.host,
                score,
            },
            Err(reason) => Assessment::Skipped {
                host: c.host,
                reason,
            },
        })
        .collect()
}

/// Pick the highest-scoring eligible host. Ties go to the lexicographically
/// smallest host key, so every client with the same inputs agrees.
#[must_use]
pub fn place<'a>(
    candidates: &[Candidate<'a>],
    client: &ClientProfile,
    now: u64,
    freshness: Freshness,
    limits: Limits,
) -> Option<&'a str> {
    assess(candidates, client, now, freshness, limits)
        .into_iter()
        .filter_map(|a| match a {
            Assessment::Eligible { host, score } => Some((host, score)),
            Assessment::Skipped { .. } => None,
        })
        .max_by(|(ha, sa), (hb, sb)| sa.cmp(sb).then_with(|| hb.cmp(ha)))
        .map(|(host, _)| host)
}

fn score(
    c: &Candidate<'_>,
    client: &ClientProfile,
    now: u64,
    freshness: Freshness,
    limits: Limits,
) -> Result<u128, Skip> {
    if !c.admitted {
        return Err(Skip::NotAdmitted);
    }
    if c.weight == 0 {
        return Err(Skip::ZeroWeight);
    }
    let received = c.presence.ok_or(Skip::NoPresence)?;
    if received.presence.host != c.host {
        return Err(Skip::NoPresence);
    }
    received.judge(now, freshness).map_err(|_| Skip::Stale)?;
    received
        .presence
        .compatible(client)
        .map_err(|_| Skip::Incompatible)?;
    let t = received.presence.telemetry.ok_or(Skip::NoTelemetry)?;
    if t.cpu_utilization_pct >= limits.max_cpu_utilization_pct
        || t.memory_available_pct < limits.min_memory_available_pct
        || t.cpu_utilization_pct >= 100
        || t.memory_available_pct == 0
    {
        return Err(Skip::Overloaded);
    }
    Ok(u128::from(c.weight)
        * u128::from(t.cpu_count)
        * u128::from(100 - t.cpu_utilization_pct)
        * u128::from(t.memory_available_pct))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presence::tests::sample;
    use crate::presence::{Telemetry, VersionRange};

    const CLIENT: ClientProfile = ClientProfile {
        protocol: 1,
        accepts: VersionRange { min: 1, max: 1 },
    };

    fn received(host: u8, cpu: u32, util: u8, mem: u8, at: u64) -> Received {
        let mut presence = sample(host, 1, at);
        presence.telemetry = Some(Telemetry {
            cpu_count: cpu,
            cpu_utilization_pct: util,
            memory_available_pct: mem,
        });
        Received {
            presence,
            received_at: at,
        }
    }

    fn candidate<'a>(r: &'a Received, weight: u32) -> Candidate<'a> {
        Candidate {
            host: &r.presence.host,
            weight,
            presence: Some(r),
            admitted: true,
        }
    }

    #[test]
    fn scores_by_weight_cpu_idle_and_memory() {
        let a = received(2, 8, 50, 50, 1000); // 1 × 8 × 50 × 50 = 20,000
        let b = received(3, 4, 0, 60, 1000); //  1 × 4 × 100 × 60 = 24,000
        let c = received(4, 16, 80, 20, 1000); // 1 × 16 × 20 × 20 = 6,400
        let list = [candidate(&a, 1), candidate(&b, 1), candidate(&c, 1)];
        let fresh = Freshness::default();
        assert_eq!(
            place(&list, &CLIENT, 1010, fresh, Limits::default()),
            Some(b.presence.host.as_str())
        );
        // Weight shifts the choice.
        let list = [candidate(&a, 2), candidate(&b, 1), candidate(&c, 1)];
        assert_eq!(
            place(&list, &CLIENT, 1010, fresh, Limits::default()),
            Some(a.presence.host.as_str())
        );
    }

    #[test]
    fn skips_stale_overloaded_zero_weight_and_incompatible() {
        let fresh = Freshness::default();
        let stale = received(2, 64, 0, 100, 100);
        let overloaded_cpu = received(3, 64, 95, 100, 1000);
        let overloaded_mem = received(4, 64, 0, 5, 1000);
        let zero = received(5, 64, 0, 100, 1000);
        let mut incompatible = received(6, 64, 0, 100, 1000);
        incompatible.presence.compatibility = VersionRange { min: 2, max: 2 };
        let mut blind = received(7, 64, 0, 100, 1000);
        blind.presence.telemetry = None;
        let list = [
            candidate(&stale, 1),
            candidate(&overloaded_cpu, 1),
            candidate(&overloaded_mem, 1),
            candidate(&zero, 0),
            candidate(&incompatible, 1),
            candidate(&blind, 1),
        ];
        let reasons: Vec<Skip> = assess(&list, &CLIENT, 1010, fresh, Limits::default())
            .into_iter()
            .map(|a| match a {
                Assessment::Skipped { reason, .. } => reason,
                Assessment::Eligible { .. } => panic!("nothing is eligible"),
            })
            .collect();
        assert_eq!(
            reasons,
            vec![
                Skip::Stale,
                Skip::Overloaded,
                Skip::Overloaded,
                Skip::ZeroWeight,
                Skip::Incompatible,
                Skip::NoTelemetry,
            ]
        );
        assert_eq!(place(&list, &CLIENT, 1010, fresh, Limits::default()), None);
    }

    #[test]
    fn edge_cases() {
        let fresh = Freshness::default();
        assert_eq!(place(&[], &CLIENT, 0, fresh, Limits::default()), None);
        // Ties go to the smaller host key.
        let a = received(2, 8, 10, 50, 1000);
        let b = received(3, 8, 10, 50, 1000);
        let smaller = a.presence.host.clone().min(b.presence.host.clone());
        let list = [candidate(&a, 1), candidate(&b, 1)];
        assert_eq!(
            place(&list, &CLIENT, 1000, fresh, Limits::default()),
            Some(smaller.as_str())
        );
        let list = [candidate(&b, 1), candidate(&a, 1)];
        assert_eq!(
            place(&list, &CLIENT, 1000, fresh, Limits::default()),
            Some(smaller.as_str())
        );
        // Largest inputs do not overflow.
        let big = received(2, 4096, 0, 100, 1000);
        let assessed = assess(
            &[candidate(&big, u32::MAX)],
            &CLIENT,
            1000,
            fresh,
            Limits::default(),
        );
        assert_eq!(
            assessed,
            vec![Assessment::Eligible {
                host: big.presence.host.as_str(),
                score: u128::from(u32::MAX) * 4096 * 100 * 100
            }]
        );
        // Missing presence, unadmitted host, and a presence for another host.
        let mut missing = candidate(&a, 1);
        missing.presence = None;
        let mut unadmitted = candidate(&a, 1);
        unadmitted.admitted = false;
        let mut swapped = candidate(&a, 1);
        swapped.host = &b.presence.host;
        let reasons: Vec<_> = assess(
            &[missing, unadmitted, swapped],
            &CLIENT,
            1000,
            fresh,
            Limits::default(),
        );
        assert!(matches!(
            reasons[0],
            Assessment::Skipped {
                reason: Skip::NoPresence,
                ..
            }
        ));
        assert!(matches!(
            reasons[1],
            Assessment::Skipped {
                reason: Skip::NotAdmitted,
                ..
            }
        ));
        assert!(matches!(
            reasons[2],
            Assessment::Skipped {
                reason: Skip::NoPresence,
                ..
            }
        ));
        // A future sample is not fresh.
        let mut future = received(2, 8, 10, 50, 1000);
        future.presence.observed_at = 2000;
        assert_eq!(
            place(
                &[candidate(&future, 1)],
                &CLIENT,
                1000,
                fresh,
                Limits::default()
            ),
            None
        );
    }
}
