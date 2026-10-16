//! The app's own OS credential entry. No CLI or upstream session files are read.
use openagents::{CredentialRecord, CredentialStore, Deployment, StorageError};
use sha2::{Digest, Sha256};
pub struct PlatformStore {
    service: String,
    profile: String,
    /// Earlier (service, profile) pairs read once when this entry is empty:
    /// the identity before a rename, or a data directory that moved.
    legacy: Vec<(String, String)>,
}
impl PlatformStore {
    pub fn new(service: String, profile: String) -> Self {
        Self {
            service,
            profile,
            legacy: Vec::new(),
        }
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
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn read(service: &str, key: &str) -> Result<Option<CredentialRecord>, StorageError> {
    let mut bytes = match security_framework::passwords::get_generic_password(service, key) {
        Ok(b) => b,
        Err(e) if e.code() == -25300 => return Ok(None),
        Err(e) => return Err(mapped(e)),
    };
    if bytes.len() > 32 * 1024 {
        bytes.fill(0);
        return Err(StorageError::Unavailable);
    }
    let record = serde_json::from_slice(&bytes).map_err(|_| StorageError::Unavailable);
    bytes.fill(0);
    record.map(Some)
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
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            if let Some(record) = read(&self.service, &self.key(deployment))? {
                return Ok(Some(record));
            }
            for (service, profile) in &self.legacy {
                // An unreadable earlier entry never blocks a fresh sign-in.
                if let Ok(Some(record)) = read(service, &key_for(profile, deployment)) {
                    if record.deployment() == deployment {
                        let _ = self.save(&record);
                        return Ok(Some(record));
                    }
                }
            }
            Ok(None)
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = deployment;
            Err(StorageError::Unavailable)
        }
    }
    fn save(&self, record: &CredentialRecord) -> Result<(), StorageError> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let mut bytes = serde_json::to_vec(record).map_err(|_| StorageError::Unavailable)?;
            let result = security_framework::passwords::set_generic_password(
                &self.service,
                &self.key(record.deployment()),
                &bytes,
            )
            .map_err(mapped);
            bytes.fill(0);
            result
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = record;
            Err(StorageError::Unavailable)
        }
    }
    fn clear(&self, deployment: &Deployment) -> Result<(), StorageError> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            match security_framework::passwords::delete_generic_password(
                &self.service,
                &self.key(deployment),
            ) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == -25300 => Ok(()),
                Err(e) => Err(mapped(e)),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = deployment;
            Err(StorageError::Unavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
