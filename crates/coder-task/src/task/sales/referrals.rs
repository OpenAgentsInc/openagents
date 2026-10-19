//! Assisted introductions come from the operator-selected canonical account
//! store. Intake strings and contact labels do not establish attribution.
use super::*;
use tenancy::accounts::referrals::Source;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Introduction {
    pub source: Source,
    pub accounts_revision: String,
    pub recorded_by: String,
    pub recorded_at: u64,
}
impl Introduction {
    pub(super) fn validate(&self, account: &str) -> Result<()> {
        let source = &self.source;
        if source.schema != tenancy::accounts::referrals::SCHEMA
            || source.account != account
            || (source.outcome == tenancy::accounts::referrals::Outcome::Captured)
                != source.referrer.is_some()
            || source
                .referrer
                .as_ref()
                .is_some_and(|identity| identity.version == 0)
            || !self
                .accounts_revision
                .strip_prefix("sha256:")
                .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("invalid assisted acquisition source".into());
        }
        text(&source.request, 128)?;
        id(&self.recorded_by)?;
        Ok(())
    }
}
impl Store {
    pub(super) fn admit_acquisition(
        &self,
        access: &Access,
        lead: &Lead,
        directory: &str,
        now: u64,
    ) -> Result<Introduction> {
        if !Path::new(directory).is_absolute()
            || lead.acquisition.is_some()
            || lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
        {
            return Err(
                "acquisition binding needs an original account source and current permission"
                    .into(),
            );
        }
        // Opening canonical accounts checks private files, the revision digest,
        // and all source identities. The operator chooses its deployment root.
        let accounts = tenancy::Accounts::open(Path::new(directory))
            .map_err(|_| "canonical acquisition accounts are unavailable")?;
        let source = accounts
            .acquisition(&lead.details.account)
            .map_err(|_| "canonical acquisition source is unavailable")?
            .ok_or("canonical acquisition source is missing")?;
        let revision = accounts
            .store()
            .map_err(|_| "canonical acquisition revision is unavailable")?
            .digest;
        let introduction = Introduction {
            source,
            accounts_revision: revision,
            recorded_by: access.principal().into(),
            recorded_at: now,
        };
        introduction.validate(&lead.details.account)?;
        Ok(introduction)
    }
}
