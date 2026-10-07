//! Consented journeys reuse private pipeline custody and current human rights.

use super::service::Reader;
use super::{Access, Lead, PermissionState, Result, Store};
use receipts::sales_funnel::{
    self as contract, Admission, Event, EventInput, Export, Failure, FailureInput,
    FinancialIdentity, Journey, Kind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
pub const MAX_JOURNEYS: usize = 16;
const MAX_OUTPUT: usize = 8 * 1024 * 1024;

impl Store {
    pub(super) fn admit_funnel(
        &self,
        access: &Access,
        lead: &Lead,
        admission: &Admission,
        command: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Journey> {
        admission.validate(now)?;
        if lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || lead.funnel_journeys.len() >= MAX_JOURNEYS
            || self
                .state
                .leads
                .values()
                .any(|lead| lead.funnel_journeys.contains_key(&admission.id))
        {
            return Err(
                "funnel enrollment needs current permission and a unique bounded journey".into(),
            );
        }
        Reader::new(root)?.read(&admission.consent.evidence)?;
        let journey = Journey {
            schema: contract::SCHEMA.into(),
            admission: admission.clone(),
            pipeline_lead: lead.id.clone(),
            pipeline_revision_at_admission: lead.revision,
            account: lead.details.account.clone(),
            admitted_by: access.principal().into(),
            admitted_at: now,
            admission_command_digest: command.into(),
            admitted_recipients: lead.details.data.recipients.clone(),
            retain_until: lead.details.data.retain_until,
            events: vec![],
            failures: vec![],
        };
        journey.validate()?;
        Ok(journey)
    }
    fn retained_funnel<'a>(
        &self,
        access: &Access,
        lead: &'a Lead,
        journey: &str,
        now: u64,
    ) -> Result<&'a Journey> {
        let journey = lead
            .funnel_journeys
            .get(journey)
            .ok_or("funnel journey is unavailable")?;
        if now >= journey.retain_until || !journey.recipient(access.principal()) {
            return Err("funnel access exceeds its original retention or recipients".into());
        }
        Ok(journey)
    }
    pub(super) fn record_funnel_event(
        &self,
        access: &Access,
        lead: &Lead,
        journey: &str,
        input: &EventInput,
        command: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Event> {
        let journey = self.retained_funnel(access, lead, journey, now)?;
        if journey.admission.consent.expires_at <= now
            || lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || journey.events.len() >= contract::MAX_EVENTS
        {
            return Err("new funnel observations need current consent and bounded history".into());
        }
        let mut reader = Reader::new(root)?;
        input.kind.validate(journey.admission.lane)?;
        for reference in input.kind.references() {
            reader.read(reference)?;
        }
        if let Kind::Purchase {
            source: FinancialIdentity::ServiceSale { sale },
            ..
        } = &input.kind
        {
            let sale = lead
                .service_sales
                .get(sale)
                .ok_or("service observation needs its canonical sale")?;
            if sale.account != journey.account
                || sale.admission.offer_version != journey.admission.offer_version
                || sale.retain_until <= now
                || !sale
                    .admitted_recipients
                    .contains(&format!("human:{}", access.principal()))
            {
                return Err("service observation exceeds its pinned customer/offer scope".into());
            }
        }
        let event = Event {
            input: input.clone(),
            recorded_by: access.principal().into(),
            recorded_at: now,
            command_digest: command.into(),
        };
        let mut updated = journey.clone();
        updated.events.push(event.clone());
        updated.validate()?;
        Ok(event)
    }
    pub(super) fn record_conversion_failure(
        &self,
        access: &Access,
        lead: &Lead,
        journey: &str,
        input: &FailureInput,
        command: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Failure> {
        let journey = self.retained_funnel(access, lead, journey, now)?;
        if input.responsible_human != lead.responsible_human
            || !self
                .state
                .principals
                .get(&input.responsible_human)
                .is_some_and(|human| human.active && human.role != super::Role::Reader)
        {
            return Err(
                "failed conversion responsibility needs the current accepted human owner".into(),
            );
        }
        Reader::new(root)?.read(&input.evidence)?;
        let failure = Failure {
            input: input.clone(),
            recorded_by: access.principal().into(),
            recorded_at: now,
            command_digest: command.into(),
        };
        let mut updated = journey.clone();
        updated.failures.push(failure.clone());
        updated.validate()?;
        Ok(failure)
    }
    pub fn funnel_show(&mut self, access: &Access, lead: &str, journey: &str) -> Result<Journey> {
        let lead = self.show(access, lead)?;
        Ok(self
            .retained_funnel(access, &lead, journey, (self.clock)())?
            .clone())
    }
    pub fn funnel_snapshot(
        &mut self,
        access: &Access,
        lead: &str,
        journey: &str,
    ) -> Result<Export> {
        let record = self.show(access, lead)?;
        let journey = self
            .retained_funnel(access, &record, journey, (self.clock)())?
            .clone();
        let requested = journey
            .events
            .iter()
            .filter_map(|event| {
                if let Kind::Purchase {
                    source: FinancialIdentity::ServiceSale { sale },
                    ..
                } = &event.input.kind
                {
                    Some(sale)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>();
        let services = record
            .service_sales
            .into_iter()
            .filter(|(id, sale)| {
                requested.contains(id)
                    && sale.account == journey.account
                    && sale.admission.offer_version == journey.admission.offer_version
            })
            .collect::<BTreeMap<_, _>>();
        let export = Export {
            schema: contract::EXPORT_SCHEMA.into(),
            journey,
            services,
            exported_by: access.principal().into(),
            exported_at: (self.clock)(),
        };
        export.validate((self.clock)())?;
        Ok(export)
    }
    /// Recheck the exact snapshot against current custody while the store lock
    /// is held. Old paid evidence cannot hide a recorded refund or deletion.
    pub fn authorize_funnel_snapshots(
        &mut self,
        access: &Access,
        snapshots: &[Export],
    ) -> Result<()> {
        self.refresh()?;
        self.admin(access)?;
        for snapshot in snapshots {
            snapshot.validate((self.clock)())?;
            let current = self.funnel_snapshot(
                access,
                &snapshot.journey.pipeline_lead,
                &snapshot.journey.admission.id,
            )?;
            let mut prefix = current.journey.clone();
            prefix.events.truncate(snapshot.journey.events.len());
            prefix.failures.truncate(snapshot.journey.failures.len());
            if prefix != snapshot.journey || current.services != snapshot.services {
                return Err("funnel source snapshot is stale or exceeds current custody".into());
            }
            let lead = self
                .state
                .leads
                .get(&snapshot.journey.pipeline_lead)
                .ok_or("funnel lead is unavailable")?;
            let mut latest = BTreeMap::new();
            for failure in &current.journey.failures {
                latest.insert(&failure.input.event, failure);
            }
            let mut original = BTreeMap::new();
            for failure in &snapshot.journey.failures {
                original.insert(&failure.input.event, failure);
            }
            if snapshot
                .journey
                .events
                .iter()
                .any(|event| latest.get(&event.input.id) != original.get(&event.input.id))
            {
                return Err("failed conversion changed after the operating snapshot".into());
            }
            if latest.values().any(|failure| {
                !failure.input.resolved
                    && (failure.input.responsible_human != lead.responsible_human
                        || !self
                            .state
                            .principals
                            .get(&failure.input.responsible_human)
                            .is_some_and(|p| p.active && p.role != super::Role::Reader))
            }) {
                return Err(
                    "failed conversion ownership needs reconciliation with the current human"
                        .into(),
                );
            }
        }
        Ok(())
    }
    pub fn funnel_write(
        &mut self,
        access: &Access,
        snapshots: &[Export],
        path: &Path,
        bytes: &[u8],
    ) -> Result<String> {
        self.authorize_funnel_snapshots(access, snapshots)?;
        if bytes.len() > MAX_OUTPUT {
            return Err("private funnel output exceeds bound".into());
        }
        self.external_file(path)?;
        use std::io::Write;
        let mut file = super::super::private_open(path, true, true)
            .map_err(|_| "private funnel output refused")?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "private funnel output write failed")?;
        super::super::sync_directory(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|_| "private funnel output directory sync failed")?;
        Ok(super::digest(bytes))
    }
    pub fn funnel_export(
        &mut self,
        access: &Access,
        lead: &str,
        journey: &str,
        path: &Path,
    ) -> Result<String> {
        let snapshot = self.funnel_snapshot(access, lead, journey)?;
        let bytes = serde_json::to_vec_pretty(&snapshot)
            .map_err(|_| "funnel export serialization failed")?;
        // A scoped reader can export its own snapshot; operating reviews still
        // require the owner through funnel_write.
        self.external_file(path)?;
        use std::io::Write;
        let mut file =
            super::super::private_open(path, true, true).map_err(|_| "funnel export refused")?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "funnel export write failed")?;
        super::super::sync_directory(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|_| "funnel export directory sync failed")?;
        Ok(super::digest(&bytes))
    }
}
