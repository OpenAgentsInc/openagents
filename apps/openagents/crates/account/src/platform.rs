//! The app's own OS credential entry. No CLI or upstream session files are read.
use openagents::{CredentialRecord, CredentialStore, Deployment, StorageError};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub trait Passwords: Send + Sync {
    fn get(&self, service: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    fn set(&self, service: &str, key: &str, bytes: &[u8]) -> Result<(), StorageError>;
    fn delete(&self, service: &str, key: &str) -> Result<(), StorageError>;
    fn list(&self, service: &str) -> Result<Vec<Vec<u8>>, StorageError>;
}

struct SystemPasswords;

pub struct PlatformStore {
    service: String,
    profile: String,
    /// Earlier (service, profile) pairs read once when this entry is empty:
    /// the identity before a rename, or a data directory that moved.
    legacy: Vec<(String, String)>,
    mobile: bool,
    migrate_mobile: bool,
    passwords: Arc<dyn Passwords>,
}
impl PlatformStore {
    pub fn new(service: String, profile: String) -> Self {
        Self {
            service,
            profile,
            legacy: Vec::new(),
            mobile: false,
            migrate_mobile: false,
            passwords: Arc::new(SystemPasswords),
        }
    }
    /// Use a caller-provided platform credential store while keeping profile
    /// and deployment key selection in Rust.
    pub fn with_passwords(mut self, passwords: Arc<dyn Passwords>) -> Self {
        self.passwords = passwords;
        self
    }
    /// iOS can replace the container UUID on update. Only the directory
    /// inside Application Support identifies this app's local profile.
    pub fn mobile(service: String, data_dir: String) -> Self {
        let relative = data_dir
            .split_once("/Library/Application Support/")
            .map(|(_, p)| p);
        let profile = relative.map_or_else(|| data_dir.clone(), |p| format!("ios/{p}"));
        let mut store = Self::new(service.clone(), profile);
        store.mobile = true;
        // Only the everyday app may search its own earlier entries. Fixture
        // profiles keep their separate names and never adopt a real login.
        store.migrate_mobile = relative == Some("core");
        store.legacy.push((service, data_dir));
        store
    }
    /// Carry a sign-in over from earlier entries. The first one found is
    /// saved under this entry; the earlier entry is left in place.
    pub fn with_legacy(mut self, legacy: Vec<(String, String)>) -> Self {
        self.legacy = legacy;
        self
    }
    fn key(&self, deployment: &Deployment) -> String {
        key_for(&self.profile, deployment)
    }

    fn previous_app_login(
        &self,
        deployment: &Deployment,
    ) -> Result<Option<CredentialRecord>, StorageError> {
        if self.service != "com.openagents.app.account" || deployment != &Deployment::production() {
            return Ok(None);
        }
        let Some(mut bytes) = self
            .passwords
            .get("com.openagents.app.account-session", "openagents.com")?
        else {
            return Ok(None);
        };
        // Builds through 62 used this app-owned Keychain entry and shape.
        #[derive(serde::Deserialize)]
        struct Previous {
            origin: String,
            account: String,
            label: String,
            expires_at: u64,
            token: String,
        }
        let previous = if bytes.len() <= 32 * 1024 {
            serde_json::from_slice::<Previous>(&bytes).ok()
        } else {
            None
        };
        bytes.fill(0);
        let Some(previous) = previous else {
            return Ok(None);
        };
        if previous.origin.trim_end_matches('/') != "https://openagents.com"
            || !previous.token.starts_with("sess_")
        {
            return Ok(None);
        }
        serde_json::from_value(serde_json::json!({
            "deployment": deployment,
            "account": {"id": previous.account, "label": previous.label},
            "expires_at": previous.expires_at, "credential": previous.token,
        }))
        .map(Some)
        .map_err(|_| StorageError::Unavailable)
    }
}
fn key_for(profile: &str, deployment: &Deployment) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}\n{}\n{}",
                profile,
                deployment.api_base(),
                deployment.website()
            )
            .as_bytes()
        )
    )
}
fn decode(mut bytes: Vec<u8>) -> Result<Option<CredentialRecord>, StorageError> {
    if bytes.len() > 32 * 1024 {
        bytes.fill(0);
        return Err(StorageError::Unavailable);
    }
    let record = serde_json::from_slice(&bytes).map_err(|_| StorageError::Unavailable);
    bytes.fill(0);
    record
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn mapped(e: security_framework::base::Error) -> StorageError {
    match e.code() {
        -25293 | -25308 | -128 => StorageError::Denied,
        _ => StorageError::Unavailable,
    }
}
impl CredentialStore for PlatformStore {
    fn load(&self, deployment: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        if let Some(bytes) = self.passwords.get(&self.service, &self.key(deployment))? {
            // JSON null records an explicit sign-out. Do not search older
            // entries after the person has removed their saved login.
            return decode(bytes);
        }
        for (service, profile) in &self.legacy {
            // An unreadable earlier entry never blocks a fresh sign-in.
            if let Ok(Some(bytes)) = self.passwords.get(service, &key_for(profile, deployment)) {
                if let Ok(Some(record)) = decode(bytes) {
                    if record.deployment() == deployment {
                        if self.mobile {
                            self.save(&record)?;
                        } else {
                            let _ = self.save(&record);
                        }
                        return Ok(Some(record));
                    }
                }
            }
        }
        if self.migrate_mobile {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut found: Option<CredentialRecord> = None;
            for bytes in self.passwords.list(&self.service)? {
                let Ok(Some(record)) = decode(bytes) else {
                    continue;
                };
                if record.deployment() != deployment || record.expired(now) {
                    continue;
                }
                if let Some(previous) = &found {
                    // Never guess between different accounts. The person can
                    // sign in to choose one; future updates keep that choice.
                    if previous.account().id != record.account().id {
                        return Ok(None);
                    }
                    if previous.expires_at().unwrap_or(u64::MAX)
                        >= record.expires_at().unwrap_or(u64::MAX)
                    {
                        continue;
                    }
                }
                found = Some(record);
            }
            if found.is_none() {
                found = self
                    .previous_app_login(deployment)?
                    .filter(|record| !record.expired(now));
            }
            if let Some(record) = found {
                self.save(&record)?;
                return Ok(Some(record));
            }
        }
        Ok(None)
    }
    fn save(&self, record: &CredentialRecord) -> Result<(), StorageError> {
        let mut bytes = serde_json::to_vec(record).map_err(|_| StorageError::Unavailable)?;
        let result = self
            .passwords
            .set(&self.service, &self.key(record.deployment()), &bytes);
        bytes.fill(0);
        result
    }
    fn clear(&self, deployment: &Deployment) -> Result<(), StorageError> {
        if self.mobile {
            self.passwords
                .set(&self.service, &self.key(deployment), b"null")
        } else {
            self.passwords.delete(&self.service, &self.key(deployment))
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
impl Passwords for SystemPasswords {
    fn get(&self, service: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match security_framework::passwords::get_generic_password(service, key) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) => Err(mapped(e)),
        }
    }
    fn set(&self, service: &str, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        security_framework::passwords::set_generic_password(service, key, bytes).map_err(mapped)
    }
    fn delete(&self, service: &str, key: &str) -> Result<(), StorageError> {
        match security_framework::passwords::delete_generic_password(service, key) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(mapped(e)),
        }
    }
    fn list(&self, service: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        use security_framework::item::{ItemClass, ItemSearchOptions, SearchResult};
        let items = ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(service)
            .load_data(true)
            .limit(256)
            .search();
        match items {
            Ok(items) => Ok(items
                .into_iter()
                .filter_map(|item| match item {
                    SearchResult::Data(bytes) => Some(bytes),
                    _ => None,
                })
                .collect()),
            Err(e) if e.code() == -25300 => Ok(Vec::new()),
            Err(e) => Err(mapped(e)),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
impl Passwords for SystemPasswords {
    fn get(&self, _: &str, _: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Err(StorageError::Unavailable)
    }
    fn set(&self, _: &str, _: &str, _: &[u8]) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
    fn list(&self, _: &str) -> Result<Vec<Vec<u8>>, StorageError> {
        Err(StorageError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, sync::Mutex};

    #[derive(Default)]
    struct Memory(Mutex<BTreeMap<(String, String), Vec<u8>>>);
    impl Passwords for Memory {
        fn get(&self, service: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(&(service.into(), key.into()))
                .cloned())
        }
        fn set(&self, service: &str, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
            self.0
                .lock()
                .unwrap()
                .insert((service.into(), key.into()), bytes.into());
            Ok(())
        }
        fn delete(&self, service: &str, key: &str) -> Result<(), StorageError> {
            self.0.lock().unwrap().remove(&(service.into(), key.into()));
            Ok(())
        }
        fn list(&self, service: &str) -> Result<Vec<Vec<u8>>, StorageError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|((s, _), _)| s == service)
                .map(|(_, b)| b.clone())
                .collect())
        }
    }
    fn path(container: &str, profile: &str) -> String {
        format!(
            "/var/mobile/Containers/Data/Application/{container}/Library/Application Support/{profile}"
        )
    }
    fn mobile(memory: &Arc<Memory>, container: &str, profile: &str) -> PlatformStore {
        let mut store = PlatformStore::mobile("fixture.account".into(), path(container, profile));
        store.passwords = memory.clone();
        store
    }
    fn record(account: &str, expiry: u64, deployment: &Deployment) -> CredentialRecord {
        serde_json::from_value(serde_json::json!({
            "deployment": deployment, "account": {"id": account, "label": "Fixture"},
            "expires_at": expiry, "credential": "sess_fixture",
        }))
        .unwrap()
    }
    fn old(memory: &Arc<Memory>, container: &str, record: &CredentialRecord) {
        let key = key_for(&path(container, "core"), record.deployment());
        memory
            .set(
                "fixture.account",
                &key,
                &serde_json::to_vec(record).unwrap(),
            )
            .unwrap();
    }

    #[test]
    fn mobile_login_survives_container_changes_and_relaunches() {
        let memory = Arc::new(Memory::default());
        let deployment = Deployment::production();
        let saved = record("acct_fixture", u64::MAX, &deployment);
        old(&memory, "before-update", &saved);
        let update = mobile(&memory, "after-update", "core");
        assert_eq!(
            update.load(&deployment).unwrap().unwrap().account().id,
            "acct_fixture"
        );
        // The migrated login is now under a stable entry. No older entry
        // needs to be found on the next install or launch.
        let stable = update.key(&deployment);
        memory
            .0
            .lock()
            .unwrap()
            .retain(|(_, key), _| key == &stable);
        let next = mobile(&memory, "another-update", "core");
        assert_eq!(stable, next.key(&deployment));
        assert_eq!(
            next.load(&deployment).unwrap().unwrap().account().id,
            "acct_fixture"
        );
        assert_ne!(
            update.key(&deployment),
            mobile(&memory, "after-update", "account-fixture-core").key(&deployment)
        );
    }

    #[test]
    fn signing_out_stays_signed_out_after_an_update() {
        let memory = Arc::new(Memory::default());
        let deployment = Deployment::production();
        let saved = record("acct_fixture", u64::MAX, &deployment);
        old(&memory, "old", &saved);
        let app = mobile(&memory, "current", "core");
        assert!(app.load(&deployment).unwrap().is_some());
        app.clear(&deployment).unwrap();
        let next = mobile(&memory, "next", "core");
        assert!(next.load(&deployment).unwrap().is_none());
        assert_eq!(
            memory
                .get("fixture.account", &next.key(&deployment))
                .unwrap()
                .unwrap(),
            b"null"
        );
        next.save(&saved).unwrap();
        assert!(
            mobile(&memory, "last", "core")
                .load(&deployment)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn migration_does_not_choose_between_accounts() {
        let memory = Arc::new(Memory::default());
        let deployment = Deployment::production();
        old(&memory, "a", &record("acct_a", u64::MAX, &deployment));
        old(&memory, "b", &record("acct_b", u64::MAX, &deployment));
        assert!(
            mobile(&memory, "c", "core")
                .load(&deployment)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn migration_ignores_expired_logins_other_sites_and_fixture_profiles() {
        let memory = Arc::new(Memory::default());
        let deployment = Deployment::production();
        old(&memory, "expired", &record("acct_expired", 1, &deployment));
        let other = Deployment::new("https://other.example/v1", "https://other.example").unwrap();
        old(&memory, "other", &record("acct_other", u64::MAX, &other));
        assert!(
            mobile(&memory, "new", "core")
                .load(&deployment)
                .unwrap()
                .is_none()
        );
        old(
            &memory,
            "valid",
            &record("acct_valid", u64::MAX, &deployment),
        );
        assert!(
            mobile(&memory, "new", "account-fixture-core")
                .load(&deployment)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            mobile(&memory, "new", "core")
                .load(&deployment)
                .unwrap()
                .unwrap()
                .account()
                .id,
            "acct_valid"
        );
    }

    #[test]
    fn migration_keeps_the_longest_lived_login_for_the_same_account() {
        let memory = Arc::new(Memory::default());
        let deployment = Deployment::production();
        old(
            &memory,
            "a",
            &record("acct_fixture", u64::MAX - 1, &deployment),
        );
        old(&memory, "b", &record("acct_fixture", u64::MAX, &deployment));
        assert_eq!(
            mobile(&memory, "c", "core")
                .load(&deployment)
                .unwrap()
                .unwrap()
                .expires_at(),
            Some(u64::MAX)
        );
    }

    #[test]
    fn the_previous_mobile_app_login_carries_over_without_reviving_sign_out() {
        let memory = Arc::new(Memory::default());
        let previous = serde_json::to_vec(&serde_json::json!({
            "origin": "https://openagents.com", "account": "acct_previous",
            "label": "Fixture", "expires_at": u64::MAX, "token": "sess_fixture",
        }))
        .unwrap();
        memory
            .set(
                "com.openagents.app.account-session",
                "openagents.com",
                &previous,
            )
            .unwrap();
        let mut app =
            PlatformStore::mobile("com.openagents.app.account".into(), path("new", "core"));
        app.passwords = memory.clone();
        let deployment = Deployment::production();
        assert_eq!(
            app.load(&deployment).unwrap().unwrap().account().id,
            "acct_previous"
        );
        app.clear(&deployment).unwrap();
        assert!(app.load(&deployment).unwrap().is_none());
        let other = Deployment::new("https://other.example/v1", "https://openagents.com").unwrap();
        assert!(app.load(&other).unwrap().is_none());
    }

    #[test]
    fn credential_entries_are_bound_to_profile_api_and_website() {
        let a = PlatformStore::new("fixture.service".into(), "/profile/a".into());
        let b = PlatformStore::new("fixture.service".into(), "/profile/b".into());
        let deployment = Deployment::production();
        let other_api =
            Deployment::new("https://other.example/v1", "https://openagents.com").unwrap();
        let other_site =
            Deployment::new("https://api.openagents.com/v1", "https://other.example").unwrap();
        assert_ne!(a.key(&deployment), b.key(&deployment));
        assert_ne!(a.key(&deployment), a.key(&other_api));
        assert_ne!(a.key(&deployment), a.key(&other_site));
    }
}
