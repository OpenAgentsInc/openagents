//! Dispatching admitted capability and model routes (#10670): the small
//! set of plugin and program releases a surface may run from a route, and
//! the fallback rule for model routes.
//!
//! [`dispatch`] runs a `plugin` route's pinned release at most once, and
//! only when every term matches what was admitted:
//!
//! - The pin (ID, version, and release digest) is in the [`Catalog`], and
//!   not excluded. An excluded or unknown pin refuses as
//!   `route_not_allowed`, and nothing runs.
//! - The snapshot names the release's recipient as a disclosure recipient
//!   and its author fee, with the payer the catalog declares for plugin
//!   fees. Anything else is a new offer: the release's terms changed after
//!   admission, so the person sees them again.
//! - The arguments fit the release's typed argument schema; otherwise the
//!   route refuses as `route_not_allowed` and nothing runs.
//!
//! The run goes through a [`Runner`], which the capability's owner supplies
//! (the plugin host, or the program runtime). Its output is retained by
//! digest, and the record keeps the artifact and the check on it in the
//! route journal before the run starts and when it ends. A record that
//! already ran is followed, never run again. A missing capability returns
//! the existing build or install offer and installs nothing.
//!
//! [`fallback`] decides whether a rate-limited model route may move to
//! another model under the same admission: only when the candidate widens
//! nothing ([`route_contract::AdmissionSnapshot::widens`]: no new
//! recipient, payer, fee, computer, or effect), costs no more than the
//! admitted quote, and is a model the catalog lists as adequate for the
//! task class. Otherwise it is a new offer, never a hidden fallback.

use route_contract::lifecycle::{CheckLabel, Lifecycle};
use route_contract::offer::Action;
use route_contract::record::CapabilityRun;
use route_contract::route::{PluginRoute, RefusalReason, Remedy};
use route_contract::snapshot::{
    AdmissionSnapshot, CapabilityPin, ModelPin, Payer, Recipient, Resource, Widening,
};
use route_contract::{Digest, RouteRecord, RouteResult};
use serde_json::Value;

use crate::route::Journal;

/// A JSON argument's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

impl ArgKind {
    fn fits(self, value: &Value) -> bool {
        match self {
            ArgKind::String => value.is_string(),
            ArgKind::Number => value.is_number(),
            ArgKind::Boolean => value.is_boolean(),
            ArgKind::Object => value.is_object(),
            ArgKind::Array => value.is_array(),
        }
    }
}

/// A release's typed arguments: the fields it requires and the ones it
/// may take, each with its type. No other field is admitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    pub required: Vec<(String, ArgKind)>,
    pub optional: Vec<(String, ArgKind)>,
}

impl Arguments {
    /// Whether `value` is an object with every required field, no unknown
    /// field, and each field of its declared type.
    #[must_use]
    pub fn admits(&self, value: &Value) -> bool {
        let Some(object) = value.as_object() else {
            return false;
        };
        let declared = |name: &str| {
            self.required
                .iter()
                .chain(&self.optional)
                .find(|(field, _)| field == name)
                .map(|(_, kind)| *kind)
        };
        self.required
            .iter()
            .all(|(field, _)| object.contains_key(field))
            && object
                .iter()
                .all(|(field, value)| declared(field).is_some_and(|kind| kind.fits(value)))
    }
}

/// One admitted release: its exact pin, its typed arguments, and the terms
/// a run of it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub pin: CapabilityPin,
    pub arguments: Arguments,
    /// Who receives the request: the plugin or tool provider.
    pub recipient: Recipient,
    /// The author's fee in sats, 0 for none, and who pays it.
    pub fee_sats: u64,
    pub fee_payer: Payer,
    /// This release does no coding: it neither reads nor writes a
    /// repository. Only such releases are in the first eligible set.
    pub noncoding: bool,
}

/// The capability set a surface may dispatch, and the models adequate for
/// a model route's task class.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub releases: Vec<Release>,
    /// Capability IDs that never run from a route, whatever their release.
    pub excluded: Vec<String>,
    pub adequate_models: Vec<ModelPin>,
}

impl Catalog {
    /// The admitted release with exactly this pin.
    #[must_use]
    pub fn release(&self, pin: &CapabilityPin) -> Option<&Release> {
        if self.excluded.contains(&pin.id) {
            return None;
        }
        self.releases
            .iter()
            .find(|release| release.pin == *pin && release.noncoding)
    }
}

/// What a capability's owner runs: one admitted release with its checked
/// arguments.
pub trait Runner {
    /// Runs `release` once and answers its output, or why it failed.
    ///
    /// # Errors
    ///
    /// The run failed.
    fn run(&mut self, release: &Release, arguments: &Value) -> Result<Output, String>;
}

/// A run's output and the check on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// The artifact's bytes, retained by digest.
    pub artifact: Vec<u8>,
    pub check: CheckLabel,
    pub cost_microusd: Option<u64>,
}

/// What [`dispatch`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dispatched {
    /// The release ran; its artifact digest and check are in the record.
    Ran { artifact: Digest, check: CheckLabel },
    /// The release ran and failed.
    Failed { reason: String },
    /// It did not run and will not: the record refused with this reason.
    Refused(RefusalReason),
    /// The terms differ from the admission: the person sees a new offer.
    NewOffer { action: Action, reason: String },
    /// The record already ran or ended; nothing ran again.
    Followed,
}

/// Runs an admitted `plugin` route once, or answers the offer a missing
/// capability makes. `artifacts` receives each output by digest. The
/// record is journaled before the run and when it ends.
///
/// # Errors
///
/// The journal could not be written; nothing ran after a failed write.
pub fn dispatch(
    record: &mut RouteRecord,
    catalog: &Catalog,
    runner: &mut dyn Runner,
    journal: &Journal,
    artifacts: &mut dyn FnMut(&Digest, &[u8]),
    now_ms: u64,
) -> std::io::Result<Dispatched> {
    let pin_and_arguments = match &record.result {
        RouteResult::Plugin {
            plugin:
                PluginRoute::Run {
                    capability,
                    arguments,
                },
        } => (capability.clone(), arguments.clone()),
        RouteResult::MissingCapability { remedy, need } => {
            // The existing build or install flow, as an offer: nothing is
            // installed or built from here.
            let action = match remedy {
                Remedy::Install { .. } => Action::PluginInstall,
                Remedy::Build | Remedy::None => Action::PluginCreate,
            };
            return Ok(Dispatched::NewOffer {
                action,
                reason: format!("no admitted capability does this: {need}"),
            });
        }
        _ => return Ok(Dispatched::Followed),
    };
    let (pin, arguments) = pin_and_arguments;
    if !record.runs.is_empty() || record.settled() || record.state != Lifecycle::Admitted {
        return Ok(Dispatched::Followed);
    }
    let refuse = |record: &mut RouteRecord, reason: RefusalReason, cause: &str| {
        let _ = record.refuse(Some(reason), cause, now_ms);
        journal.write(record).map(|()| Dispatched::Refused(reason))
    };
    let Some(release) = catalog.release(&pin) else {
        return refuse(
            record,
            RefusalReason::RouteNotAllowed,
            "capability_not_admitted",
        );
    };
    if record.snapshot.route.capability.as_ref() != Some(&pin) {
        return refuse(
            record,
            RefusalReason::RouteNotAllowed,
            "capability_not_admitted",
        );
    }
    if let Some(reason) = changed_terms(&record.snapshot, release) {
        return Ok(Dispatched::NewOffer {
            action: Action::PluginRun,
            reason,
        });
    }
    if !release.arguments.admits(&arguments) {
        return refuse(
            record,
            RefusalReason::RouteNotAllowed,
            "arguments_not_admitted",
        );
    }
    // Journal the dispatch before anything runs.
    let _ = record.step(Lifecycle::DispatchPending, "capability_dispatch", now_ms);
    journal.write(record)?;
    let started = std::time::Instant::now();
    let outcome = runner.run(release, &arguments);
    let wall_ms = Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    let (run, dispatched) = match outcome {
        Ok(output) => {
            let digest = Digest::of_bytes(&output.artifact);
            artifacts(&digest, &output.artifact);
            (
                CapabilityRun {
                    ok: true,
                    artifacts: vec![digest.clone()],
                    check: output.check,
                    wall_ms,
                    cost_microusd: output.cost_microusd,
                },
                Dispatched::Ran {
                    artifact: digest,
                    check: output.check,
                },
            )
        }
        Err(reason) => (
            CapabilityRun {
                ok: false,
                artifacts: Vec::new(),
                check: CheckLabel::Pending,
                wall_ms,
                cost_microusd: None,
            },
            Dispatched::Failed { reason },
        ),
    };
    let _ = record.capability_ran(run, now_ms);
    journal.write(record)?;
    Ok(dispatched)
}

/// Why a release's terms differ from what the snapshot admitted, if they
/// do: its recipient, its fee, or the payer of its fee.
fn changed_terms(snapshot: &AdmissionSnapshot, release: &Release) -> Option<String> {
    if !snapshot.disclosure.recipients.contains(&release.recipient) {
        return Some(format!(
            "the release sends the request to {}, which was not admitted",
            release.recipient.id
        ));
    }
    let fee = snapshot
        .money
        .fees
        .iter()
        .find(|fee| fee.plugin == release.pin.id)
        .map_or(0, |fee| fee.sats);
    if fee != release.fee_sats {
        return Some(format!(
            "the release's fee is {} sats, not the {fee} admitted",
            release.fee_sats
        ));
    }
    if release.fee_sats > 0 {
        let payer = snapshot
            .money
            .payers
            .iter()
            .find(|entry| entry.resource == Resource::PluginFee)
            .map(|entry| &entry.payer);
        if payer != Some(&release.fee_payer) {
            return Some("another payer covers the release's fee".into());
        }
    }
    None
}

/// Whether a model route may move to `candidate` under its admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// The same admission covers it.
    Allowed,
    /// It widens the admission, costs more, or is not adequate: a new
    /// offer, with why.
    NewOffer {
        widenings: Vec<Widening>,
        reason: String,
    },
}

/// The fallback rule for a model route: `candidate` is the snapshot the
/// fallback would run under, built from the admitted one.
#[must_use]
pub fn fallback(
    admitted: &AdmissionSnapshot,
    candidate: &AdmissionSnapshot,
    catalog: &Catalog,
) -> Fallback {
    let widenings = candidate.widens(admitted);
    if !widenings.is_empty() {
        return Fallback::NewOffer {
            widenings,
            reason: "it widens the admission".into(),
        };
    }
    // Any payer the admission did not name, for any resource: the
    // contract's own check lets a cost move off OpenAgents, which the
    // person never agreed to here.
    if candidate
        .money
        .payers
        .iter()
        .any(|entry| !admitted.money.payers.contains(entry))
    {
        return Fallback::NewOffer {
            widenings: vec![Widening::Payer],
            reason: "another payer would cover it".into(),
        };
    }
    let ceiling = |snapshot: &AdmissionSnapshot| snapshot.money.quote.as_ref().map(|q| q.max_sats);
    if let (Some(admitted), candidate) = (ceiling(admitted), ceiling(candidate))
        && candidate.is_none_or(|candidate| candidate > admitted)
    {
        return Fallback::NewOffer {
            widenings,
            reason: "it costs more than the admitted quote".into(),
        };
    }
    match &candidate.route.model {
        Some(model) if catalog.adequate_models.contains(model) => Fallback::Allowed,
        _ => Fallback::NewOffer {
            widenings,
            reason: "the model is not adequate for this task class".into(),
        },
    }
}

#[cfg(test)]
mod tests;
