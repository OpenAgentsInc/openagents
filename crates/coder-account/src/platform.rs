//! The app's own OS credential entry. No CLI or upstream session files are read.
use openagents::{CredentialRecord, CredentialStore, Deployment, StorageError};
use sha2::{Digest, Sha256};
pub struct PlatformStore {
    service: String,
    profile: String,
}
impl PlatformStore {
    pub fn new(service: String, profile: String) -> Self {
        Self { service, profile }
    }
    fn key(&self, deployment: &Deployment) -> String {
        format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}\n{}\n{}",
                    self.profile,
                    deployment.api_base(),
                    deployment.website()
                )
                .as_bytes()
            )
        )
    }
}
#[cfg(target_os = "macos")]
fn mapped(e: security_framework::base::Error) -> StorageError {
    match e.code() {
        -25293 | -25308 | -128 => StorageError::Denied,
        _ => StorageError::Unavailable,
    }
}
impl CredentialStore for PlatformStore {
    fn load(&self, deployment: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        #[cfg(target_os = "macos")]
        {
            let mut bytes = match security_framework::passwords::get_generic_password(
                &self.service,
                &self.key(deployment),
            ) {
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = deployment;
            Err(StorageError::Unavailable)
        }
    }
    fn save(&self, record: &CredentialRecord) -> Result<(), StorageError> {
        #[cfg(target_os = "macos")]
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = record;
            Err(StorageError::Unavailable)
        }
    }
    fn clear(&self, deployment: &Deployment) -> Result<(), StorageError> {
        #[cfg(target_os = "macos")]
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
        #[cfg(not(target_os = "macos"))]
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
