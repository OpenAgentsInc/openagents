//! The operator's diagnostics: the same public contracts a caller
//! reads, rendered for the person running the door.
//!
//! A doctor answers the questions an operator asks before trusting a
//! resolved profile: which profile resolved, whether the door's public
//! reads answer, and where the caller's workspace stands. Every answer
//! comes from the contracts callers use — `GET /healthz` for liveness,
//! `GET /v1/models` for the doors a credential may name, `GET
//! /v1/balance` for the workspace position. Nothing a report shows may
//! come from a private channel: a doctor reads what any caller could
//! read, or it says `unknown`.
//!
//! Three rules shape the report, the same ones the decision evidence
//! view keeps:
//!
//! - **Three states, never smudged.** A route is reachable, refused
//!   with the door's own typed code, or unreachable — each a distinct
//!   [`DoorStatus`], never one prose string a reader has to parse.
//! - **Unknown is a word, not a zero.** A workspace position the door
//!   does not meter reports `unknown`; an unmeasured field stays
//!   unmeasured rather than wearing a fabricated zero.
//! - **A simulated report can never pass for metered work.** Every
//!   piece carries an [`Origin`], and a report holding even one
//!   invented piece reads `simulated` whole.
//!
//! The module is pure assembly: the caller injects the door probe, so
//! the same report serves a test's fixed answers and a live wiring's
//! real reads. No socket opens here, and no credential enters a line —
//! a resolved destination is already free of credential material, and a
//! doctor never re-prints a credential.

use crate::profiles::{Profile, Source};

pub use coder_terminal::decision::Origin;

/// The public routes a report probes, in the order it probes them:
/// liveness first, then the caller's doors, then the position.
pub const ROUTES: &[&str] = &["/healthz", "/v1/models", "/v1/balance"];

/// The route that carries a workspace position when it answers.
const BALANCE: &str = "/v1/balance";

/// The mark a cut line ends with, so a narrowed report shows it
/// narrowed.
const ELLIPSIS: &str = "...";

/// What one probe of a public route brought back.
///
/// The three states are distinct variants, so a refusal's code and a
/// silence never blur into one "did not answer": `Reachable` means the
/// route answered, `Refused` keeps the door's own typed code, and
/// `Unreachable` means no answer arrived at all. Every variant carries
/// the reading's [`Origin`] — an invented demonstration and a measured
/// probe are different objects, not different renderings.
#[derive(Clone, Debug, PartialEq)]
pub enum DoorStatus {
    /// The route answered. `position` is the workspace reading a
    /// `/v1/balance` answer carried; any other route leaves it `None`.
    Reachable {
        /// The workspace position, when the answer held one.
        position: Option<Position>,
        /// Whether the reading was measured or invented for an example.
        origin: Origin,
    },
    /// The door answered with a typed refusal, kept by its code.
    Refused {
        /// The refusal code, as the door spelled it.
        code: String,
        /// Whether the reading was measured or invented for an example.
        origin: Origin,
    },
    /// The route could not be reached: no answer at all.
    Unreachable {
        /// Whether the reading was measured or invented for an example.
        origin: Origin,
    },
}

impl DoorStatus {
    /// A route that answered, carrying no position.
    #[must_use]
    pub fn reachable(origin: Origin) -> Self {
        Self::Reachable {
            position: None,
            origin,
        }
    }

    /// A `/v1/balance` route that answered with a workspace position.
    #[must_use]
    pub fn answered(position: Position, origin: Origin) -> Self {
        Self::Reachable {
            position: Some(position),
            origin,
        }
    }

    /// A door that refused with the typed `code` it returned.
    #[must_use]
    pub fn refused(code: impl Into<String>, origin: Origin) -> Self {
        Self::Refused {
            code: code.into(),
            origin,
        }
    }

    /// A route that could not be reached at all.
    #[must_use]
    pub fn unreachable(origin: Origin) -> Self {
        Self::Unreachable { origin }
    }

    /// Where the reading came from.
    #[must_use]
    pub fn origin(&self) -> Origin {
        match self {
            Self::Reachable { origin, .. }
            | Self::Refused { origin, .. }
            | Self::Unreachable { origin } => *origin,
        }
    }
}

/// The workspace position a `/v1/balance` answer carried.
///
/// Every field is known or unknown — an unmetered door has no position
/// to report, and `unknown` is the honest rendering of that, never a
/// zero dressed up as a measurement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Position {
    /// Where the reading came from.
    pub origin: Origin,
    /// The currency the amounts are counted in.
    pub currency: Option<String>,
    /// Credited to the workspace.
    pub credited: Option<u64>,
    /// Held by unsettled reservations.
    pub reserved: Option<u64>,
    /// Settled spend.
    pub settled: Option<u64>,
    /// Returned to the workspace.
    pub refunded: Option<u64>,
    /// Spendable now.
    pub available: Option<u64>,
    /// Left under the workspace's spend authorization.
    pub spend_remaining: Option<u64>,
}

impl Position {
    /// Whether every field is unknown — the unmetered door's position.
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        self.currency.is_none()
            && self.credited.is_none()
            && self.reserved.is_none()
            && self.settled.is_none()
            && self.refunded.is_none()
            && self.available.is_none()
            && self.spend_remaining.is_none()
    }

    /// The position as one operator line: `unknown` whole when nothing
    /// is known, each field named with its amount or `unknown`
    /// otherwise.
    fn line(&self) -> String {
        if self.is_unknown() {
            return format!("position: unknown ({})", self.origin.word());
        }
        let amount = |value: Option<u64>| match value {
            Some(value) => value.to_string(),
            None => "unknown".to_string(),
        };
        let fields = [
            format!("credited {}", amount(self.credited)),
            format!("reserved {}", amount(self.reserved)),
            format!("settled {}", amount(self.settled)),
            format!("refunded {}", amount(self.refunded)),
            format!("available {}", amount(self.available)),
            format!("spend remaining {}", amount(self.spend_remaining)),
            format!("currency {}", self.currency.as_deref().unwrap_or("unknown")),
        ]
        .join(", ");
        format!("position: {fields} ({})", self.origin.word())
    }
}

/// One route's probe, kept with the route's name.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The public route probed.
    pub route: &'static str,
    /// What the probe brought back.
    pub status: DoorStatus,
}

impl Reading {
    /// The reading as one operator line: the route, then its own word —
    /// `reachable`, `refused` with the door's code, or `unreachable` —
    /// and the reading's origin.
    fn line(&self) -> String {
        let state = match &self.status {
            DoorStatus::Reachable { .. } => "reachable".to_string(),
            DoorStatus::Refused { code, .. } => format!("refused {code}"),
            DoorStatus::Unreachable { .. } => "unreachable".to_string(),
        };
        let origin = self.status.origin().word();
        format!("route {}: {state} ({origin})", self.route)
    }
}

/// What a doctor run assembled: the resolved profile, one reading per
/// public route, and the workspace position when `/v1/balance`
/// answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The resolved profile's name.
    pub profile: String,
    /// Where the profile choice itself came from.
    pub picked: Source,
    /// The transport decisions travel over.
    pub transport: String,
    /// The destination the profile resolved — a door URL or relay
    /// address, already free of credential material.
    pub destination: String,
    /// Whether the door keeps questions on this machine or network.
    pub local: bool,
    /// One reading per public route, in probe order.
    pub routes: Vec<Reading>,
    /// The workspace the position was read under, when one was named.
    pub workspace: Option<String>,
    /// The workspace position `/v1/balance` answered — every field
    /// unknown when it did not.
    pub position: Position,
}

impl Report {
    /// The report's origin: `Simulated` when any piece was invented for
    /// an example — a report that is partly a demonstration must not
    /// read as a measurement — `Metered` only when every piece was
    /// measured.
    #[must_use]
    pub fn origin(&self) -> Origin {
        let measured = self
            .routes
            .iter()
            .all(|reading| reading.status.origin() == Origin::Metered)
            && self.position.origin == Origin::Metered;
        if measured {
            Origin::Metered
        } else {
            Origin::Simulated
        }
    }

    /// The operator's lines: which profile, which destination, what
    /// each public route answered, and the workspace position — clipped
    /// to `width`, each cut marked with `...` so a narrowed report
    /// shows it narrowed.
    #[must_use]
    pub fn lines(&self, width: usize) -> Vec<String> {
        let mut lines = vec![
            clip(format!("doctor: {}", self.origin().word()), width),
            clip(
                format!("profile: {} (from {})", self.profile, self.picked),
                width,
            ),
            clip(format!("transport: {}", self.transport), width),
            clip(format!("destination: {}", self.destination), width),
            clip(format!("local: {}", self.local), width),
        ];
        for reading in &self.routes {
            lines.push(clip(reading.line(), width));
        }
        match &self.workspace {
            Some(workspace) => lines.push(clip(format!("workspace: {workspace}"), width)),
            None => lines.push(clip("workspace: none".to_string(), width)),
        }
        lines.push(clip(self.position.line(), width));
        lines
    }
}

/// The diagnostics surface: assembles a [`Report`] from a resolved
/// profile and a probe over the public routes.
pub struct Doctor;

impl Doctor {
    /// Probe the public routes and assemble the report.
    ///
    /// `door` is the probe: it is called once per route in [`ROUTES`]
    /// with the route's path and answers what that read found. The
    /// closure is the caller's, so a test answers deterministically and
    /// this module never opens a socket. `workspace`, when named, is
    /// the account the `/v1/balance` position is read under.
    #[must_use]
    pub fn report(
        profile: &Profile,
        door: &impl Fn(&str) -> DoorStatus,
        workspace: Option<&str>,
    ) -> Report {
        let routes: Vec<Reading> = ROUTES
            .iter()
            .map(|&route| Reading {
                route,
                status: door(route),
            })
            .collect();
        // The position is the balance route's answer, when it answered
        // with one. Any other outcome leaves every field unknown — a
        // refusal, a silence, and an answer that carried no position
        // all report the same honest `unknown`.
        let balance = routes.iter().find(|reading| reading.route == BALANCE);
        let position = balance
            .and_then(|reading| match &reading.status {
                DoorStatus::Reachable { position, .. } => position.clone(),
                _ => None,
            })
            .unwrap_or_else(|| Position {
                origin: balance.map_or(Origin::Simulated, |reading| reading.status.origin()),
                ..Position::default()
            });
        let destination = match profile {
            Profile::HostedHttp { url, .. }
            | Profile::DirectLocal { url, .. }
            | Profile::OwnProvider { url, .. } => url.value.clone(),
            Profile::Relay { relay, .. } => relay.value.clone(),
        };
        Report {
            profile: profile.name().to_string(),
            picked: profile.picked(),
            transport: profile.transport().to_string(),
            destination,
            local: profile.is_local(),
            routes,
            workspace: workspace.map(str::to_string),
            position,
        }
    }
}

/// Cut a line at `width` characters. Whatever is dropped ends with
/// `...`, so a cut is always marked, never silent.
fn clip(text: String, width: usize) -> String {
    if text.chars().count() <= width {
        return text;
    }
    if width <= ELLIPSIS.len() {
        return ELLIPSIS[..width].to_string();
    }
    let mut taken: String = text.chars().take(width - ELLIPSIS.len()).collect();
    taken.push_str(ELLIPSIS);
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::Sourced;
    use jev::ApiKey;

    /// A credential-shaped value, so a leak has something to leak.
    const SECRET: &str = "oak_d34db33f.sup3rs3cr3t";

    /// A hosted profile holding a credential — the shape most likely to
    /// leak a secret if a line forgets itself.
    fn hosted() -> Profile {
        Profile::HostedHttp {
            url: Sourced {
                value: "https://door.example.com".to_string(),
                source: Source::Flag,
            },
            model: Sourced {
                value: "shared-kev".to_string(),
                source: Source::Default,
            },
            key: Sourced {
                value: ApiKey::new(SECRET),
                source: Source::Env("CODER_DECISION_KEY"),
            },
            picked: Source::Flag,
        }
    }

    /// A probe that answers every route, carrying a full position on
    /// the balance read — the healthy door.
    fn healthy(route: &str) -> DoorStatus {
        match route {
            BALANCE => DoorStatus::answered(
                Position {
                    origin: Origin::Metered,
                    currency: Some("usd".to_string()),
                    credited: Some(12_500),
                    reserved: Some(300),
                    settled: Some(4_200),
                    refunded: Some(0),
                    available: Some(8_000),
                    spend_remaining: Some(3_200),
                },
                Origin::Metered,
            ),
            _ => DoorStatus::reachable(Origin::Metered),
        }
    }

    #[test]
    fn a_healthy_door_reports_all_three_routes() {
        let report = Doctor::report(&hosted(), &healthy, Some("acme"));
        assert_eq!(report.routes.len(), ROUTES.len());
        for (reading, route) in report.routes.iter().zip(ROUTES) {
            assert_eq!(reading.route, *route);
            assert!(
                matches!(reading.status, DoorStatus::Reachable { .. }),
                "{route}: {:?}",
                reading.status
            );
        }
        let joined = report.lines(120).join("\n");
        assert!(joined.contains("doctor: metered"), "{joined}");
        assert!(joined.contains("profile: hosted_http"), "{joined}");
        assert!(joined.contains("transport: http"), "{joined}");
        assert!(
            joined.contains("destination: https://door.example.com"),
            "{joined}"
        );
        assert!(joined.contains("route /healthz: reachable"), "{joined}");
        assert!(joined.contains("route /v1/models: reachable"), "{joined}");
        assert!(joined.contains("route /v1/balance: reachable"), "{joined}");
        assert!(joined.contains("workspace: acme"), "{joined}");
        assert!(joined.contains("credited 12500"), "{joined}");
        assert!(joined.contains("available 8000"), "{joined}");
    }

    #[test]
    fn a_refusal_reports_its_code_distinctly_from_unreachable() {
        let door = |route: &str| match route {
            "/v1/models" => DoorStatus::refused("quota_exhausted", Origin::Metered),
            BALANCE => DoorStatus::unreachable(Origin::Metered),
            _ => DoorStatus::reachable(Origin::Metered),
        };
        let report = Doctor::report(&hosted(), &door, Some("acme"));
        match &report.routes[1].status {
            DoorStatus::Refused { code, .. } => assert_eq!(code, "quota_exhausted"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(matches!(
            report.routes[2].status,
            DoorStatus::Unreachable { .. }
        ));
        let joined = report.lines(120).join("\n");
        assert!(
            joined.contains("route /v1/models: refused quota_exhausted"),
            "{joined}"
        );
        assert!(
            joined.contains("route /v1/balance: unreachable"),
            "{joined}"
        );
    }

    #[test]
    fn an_unmetered_door_reports_an_unknown_position() {
        let door = |route: &str| match route {
            BALANCE => DoorStatus::refused("unmetered", Origin::Metered),
            _ => DoorStatus::reachable(Origin::Metered),
        };
        let report = Doctor::report(&hosted(), &door, Some("acme"));
        assert!(report.position.is_unknown());
        let joined = report.lines(120).join("\n");
        assert!(joined.contains("route /v1/balance: refused unmetered"), "{joined}");
        assert!(joined.contains("position: unknown"), "{joined}");
        assert!(!joined.contains("position: credited"), "{joined}");

        // A position that answered partly known renders the known
        // fields and keeps the rest unknown — never a zero.
        let partial = |route: &str| match route {
            BALANCE => DoorStatus::answered(
                Position {
                    origin: Origin::Metered,
                    credited: Some(10),
                    ..Position::default()
                },
                Origin::Metered,
            ),
            _ => DoorStatus::reachable(Origin::Metered),
        };
        let report = Doctor::report(&hosted(), &partial, Some("acme"));
        let joined = report.lines(200).join("\n");
        assert!(joined.contains("credited 10"), "{joined}");
        assert!(joined.contains("reserved unknown"), "{joined}");
        assert!(joined.contains("available unknown"), "{joined}");
    }

    #[test]
    fn a_simulated_report_is_labeled() {
        let door = |_: &str| DoorStatus::reachable(Origin::Simulated);
        let report = Doctor::report(&hosted(), &door, None);
        assert_eq!(report.origin(), Origin::Simulated);
        let joined = report.lines(120).join("\n");
        assert!(joined.contains("doctor: simulated"), "{joined}");
        assert!(joined.contains("route /healthz: reachable (simulated)"), "{joined}");
    }

    #[test]
    fn no_line_carries_credential_material() {
        let report = Doctor::report(&hosted(), &healthy, Some("acme"));
        let joined = report.lines(120).join("\n");
        assert!(!joined.contains(SECRET), "{joined}");
        assert!(!joined.contains("sup3rs3cr3t"), "{joined}");
        assert!(!joined.contains("oak_d34db33f"), "{joined}");
        let narrow = report.lines(12).join("\n");
        assert!(!narrow.contains(SECRET), "{narrow}");
    }

    #[test]
    fn narrow_widths_elide_with_a_marker() {
        let report = Doctor::report(&hosted(), &healthy, Some("acme"));
        let lines = report.lines(24);
        assert!(lines.iter().any(|line| line.ends_with(ELLIPSIS)), "{lines:?}");
        for line in &lines {
            assert!(line.chars().count() <= 24, "{line}");
        }
        for line in report.lines(2) {
            assert!(line.chars().count() <= 2, "{line}");
        }
    }
}
