//! Service records share the private pipeline's lock, authorization, revision,
//! atomic storage, audit, suppression, and retention. This sends no invoice.

use super::{Access, Lead, PermissionState, Result, Store};
use receipts::service_sale::{
    self as contract, Admission, Export, PaymentInput, Reference, Sale, Verification,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path};

pub const MAX_SALES: usize = 16;
const MAX_FILE: usize = 8 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;

pub(super) struct Reader<'a> {
    root: &'a Path,
    sources: BTreeMap<String, (String, Vec<u8>)>,
    total: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn new(root: Option<&'a Path>) -> Result<Self> {
        let root = root.ok_or("service recording needs an explicit private evidence root")?;
        let meta =
            fs::symlink_metadata(root).map_err(|_| "service evidence root is unavailable")?;
        if !meta.is_dir() || meta.file_type().is_symlink() || meta.permissions().mode() & 0o077 != 0
        {
            return Err("service evidence root must be a private directory".into());
        }
        Ok(Self {
            root,
            sources: BTreeMap::new(),
            total: 0,
        })
    }
    pub(super) fn read(&mut self, r: &Reference) -> Result<Vec<u8>> {
        r.validate()?;
        if let Some((hash, bytes)) = self.sources.get(&r.path) {
            if hash != &r.sha256 {
                return Err("conflicting service source digest".into());
            }
            return Ok(bytes.clone());
        }
        let mut path = self.root.to_path_buf();
        for component in Path::new(&r.path).components() {
            let Component::Normal(part) = component else {
                return Err("service source leaves its root".into());
            };
            path.push(part);
            if fs::symlink_metadata(&path)
                .map_err(|_| "service evidence is missing")?
                .file_type()
                .is_symlink()
            {
                return Err("symlink service evidence is refused".into());
            }
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| "service evidence read refused")?;
        if !file
            .metadata()
            .map_err(|_| "service evidence metadata")?
            .is_file()
        {
            return Err("service evidence must be a regular file".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "service evidence read failed")?;
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or("service source overflow")?;
        if bytes.is_empty()
            || bytes.len() > MAX_FILE
            || self.total > MAX_TOTAL
            || super::digest(&bytes) != r.sha256
        {
            return Err("service source is missing, oversized, or changed".into());
        }
        self.sources
            .insert(r.path.clone(), (r.sha256.clone(), bytes.clone()));
        Ok(bytes)
    }
}
impl Store {
    pub(super) fn admit_service(
        &self,
        access: &Access,
        lead: &Lead,
        admission: &Admission,
        command_digest: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Sale> {
        if lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || lead.service_sales.len() >= MAX_SALES
        {
            return Err("service admission needs current permission and bounded records".into());
        }
        if self
            .state
            .leads
            .values()
            .flat_map(|l| l.service_sales.values())
            .any(|s| {
                s.admission.id == admission.id
                    || s.admission.invoice.id == admission.invoice.id
                    || s.admission.invoice.evidence.sha256 == admission.invoice.evidence.sha256
                    || (s.admission.invoice.payment_route_reference
                        == admission.invoice.payment_route_reference
                        && s.admission.invoice.external_reference
                            == admission.invoice.external_reference)
            })
        {
            return Err("duplicate service sale or authoritative invoice".into());
        }
        let mut reader = Reader::new(root)?;
        let facts = contract::verify_sources(
            admission,
            &lead.id,
            &lead.details.account,
            lead.revision,
            now,
            |r| reader.read(r),
        )?;
        if !lead
            .details
            .data
            .recipients
            .contains(&format!("human:{}", facts.support_human))
            || admission.fulfillment.as_ref().is_some_and(|f| {
                !lead
                    .details
                    .data
                    .recipients
                    .contains(&format!("human:{}", f.responsible_human))
            })
        {
            return Err(
                "service support or fulfillment owner exceeds the admitted data recipients".into(),
            );
        }
        let sale = Sale {
            schema: contract::SCHEMA.into(),
            admission: admission.clone(),
            pipeline_lead: lead.id.clone(),
            pipeline_revision_at_admission: lead.revision,
            account: lead.details.account.clone(),
            admitted_by: access.principal().into(),
            admitted_at: now,
            admission_command_digest: command_digest.into(),
            admitted_recipients: lead.details.data.recipients.clone(),
            retain_until: lead.details.data.retain_until,
            facts,
            payments: vec![],
            fulfillment_reconciliations: vec![],
        };
        sale.validate()?;
        Ok(sale)
    }
    fn retained_service<'a>(
        &self,
        access: &Access,
        lead: &'a Lead,
        id: &str,
        now: u64,
    ) -> Result<&'a Sale> {
        let sale = lead
            .service_sales
            .get(id)
            .ok_or("service sale is unavailable")?;
        if sale.retain_until <= now
            || !sale
                .admitted_recipients
                .contains(&format!("human:{}", access.principal()))
        {
            return Err("service access exceeds its admitted retention or recipients".into());
        }
        Ok(sale)
    }
    pub(super) fn reconcile_service(
        &self,
        access: &Access,
        lead: &Lead,
        id: &str,
        payment: &PaymentInput,
        command_digest: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Verification> {
        let sale = self.retained_service(access, lead, id, now)?;
        sale.validate_next(payment)?;
        if let Some(reference) = &payment.external_reference {
            if self
                .state
                .leads
                .values()
                .flat_map(|l| l.service_sales.values())
                .any(|other| {
                    other.admission.id != sale.admission.id
                        && other.admission.invoice.payment_route_reference
                            == sale.admission.invoice.payment_route_reference
                        && other
                            .payments
                            .iter()
                            .any(|v| v.input.external_reference.as_ref() == Some(reference))
                })
            {
                return Err("the same external payment cannot fund two service invoices".into());
            }
        }
        let mut reader = Reader::new(root)?;
        reader.read(&payment.evidence)?;
        let facts = contract::verify_sources(
            &sale.admission,
            &sale.pipeline_lead,
            &sale.account,
            sale.pipeline_revision_at_admission,
            sale.admitted_at,
            |r| reader.read(r),
        )?;
        if facts != sale.facts {
            return Err("retained service acceptance changed".into());
        }
        let verification = Verification {
            input: payment.clone(),
            verified_by: access.principal().into(),
            verified_at: now,
            command_digest: command_digest.into(),
        };
        let mut updated = sale.clone();
        updated.payments.push(verification.clone());
        updated.validate()?;
        Ok(verification)
    }
    pub fn service_show(&mut self, access: &Access, lead: &str, sale: &str) -> Result<Sale> {
        let record = self.show(access, lead)?;
        Ok(self
            .retained_service(access, &record, sale, (self.clock)())?
            .clone())
    }
    pub(super) fn reconcile_fulfillment(
        &self,
        access: &Access,
        lead: &Lead,
        id: &str,
        input: &contract::FulfillmentInput,
        command_digest: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<contract::FulfillmentVerification> {
        let sale = self.retained_service(access, lead, id, now)?;
        let verification = contract::FulfillmentVerification {
            input: input.clone(),
            verified_by: access.principal().into(),
            verified_at: now,
            command_digest: command_digest.into(),
        };
        let mut updated = sale.clone();
        updated
            .fulfillment_reconciliations
            .push(verification.clone());
        updated.validate()?;
        let f = updated
            .effective_fulfillment()?
            .ok_or("service has no admitted fulfillment obligation")?;
        let mut reader = Reader::new(root)?;
        contract::verify_fulfillment(
            &f,
            &sale.account,
            &sale.admission.offer_version,
            &sale.admission.invoice.id,
            now,
            |r| reader.read(r),
        )?;
        Ok(verification)
    }
    pub fn service_export(
        &mut self,
        access: &Access,
        lead: &str,
        sale: &str,
        path: &Path,
    ) -> Result<String> {
        let sale = self.service_show(access, lead, sale)?;
        let export = Export {
            schema: contract::EXPORT_SCHEMA.into(),
            sale,
            exported_by: access.principal().into(),
            exported_at: (self.clock)(),
        };
        export.validate()?;
        self.external_file(path)?;
        let bytes = serde_json::to_vec_pretty(&export)
            .map_err(|_| "service export serialization failed")?;
        let mut file =
            super::super::private_open(path, true, true).map_err(|_| "service export refused")?;
        use std::io::Write;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "service export write failed")?;
        super::super::sync_directory(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|_| "service export directory sync failed")?;
        Ok(super::digest(&bytes))
    }
}
