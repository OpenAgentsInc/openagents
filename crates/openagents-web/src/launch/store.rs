//! The launch's records, kept in the site's own store
//! ([`crate::chat_store::Store`], as `/contact` keeps its requests):
//!
//! - `launch/waitlist/<email key>.json`: one [`WaitlistEntry`] per email.
//!   The write only succeeds when nothing is there, so a second sign-up
//!   with the same address keeps the first.
//! - `launch/profiles/<a- or e-><key>.json`: one [`Profile`] per account
//!   (signed in) or per email (a guest). A later answer replaces it.
//! - `launch/surveys/<session>.json`: one [`Survey`] per playtest session,
//!   kept once.
//!
//! An email key is the first 32 hex digits of the SHA-256 of the trimmed,
//! lowercased address; an account key is the same over the account id.

use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::chat_store::{Error, Store};

pub(crate) const WAITLIST: &str = "launch/waitlist";
pub(crate) const PROFILES: &str = "launch/profiles";
pub(crate) const SURVEYS: &str = "launch/surveys";

/// The most records of one kind an export reads.
const MAX_EXPORT: usize = 50_000;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct WaitlistEntry {
    pub(crate) email: String,
    /// A [`super::forms::ROLES`] value, or empty.
    #[serde(default)]
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) source: String,
    pub(crate) created_unix: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Profile {
    /// The signed-in account, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) account_id: Option<String>,
    #[serde(default)]
    pub(crate) email: String,
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) engines: Vec<String>,
    #[serde(default)]
    pub(crate) experience: String,
    #[serde(default)]
    pub(crate) make: String,
    #[serde(default)]
    pub(crate) heard: String,
    #[serde(default)]
    pub(crate) contact_ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) session: Option<String>,
    pub(crate) created_unix: u64,
    pub(crate) updated_unix: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Survey {
    pub(crate) session: String,
    /// How easy it was, 1 (hard) to 5 (easy).
    pub(crate) ease: u8,
    /// `yes`, `partial`, or `no`.
    pub(crate) finished: String,
    #[serde(default)]
    pub(crate) confused: String,
    #[serde(default)]
    pub(crate) make: String,
    /// How likely they are to use it again, 0 to 10.
    pub(crate) again: u8,
    pub(crate) device: String,
    pub(crate) browser: String,
    #[serde(default)]
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) contact_ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) account_id: Option<String>,
    #[serde(default)]
    pub(crate) source: String,
    /// Whether the visitor agreed to have their steps recorded.
    #[serde(default)]
    pub(crate) recorded: bool,
    pub(crate) created_unix: u64,
}

/// How a create-once write went.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kept {
    New,
    AlreadyThere,
}

fn short_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn email_key(email: &str) -> String {
    short_hash(&email.trim().to_ascii_lowercase())
}

fn waitlist_key(email: &str) -> String {
    format!("{WAITLIST}/{}.json", email_key(email))
}

fn profile_key(profile: &Profile) -> String {
    match &profile.account_id {
        Some(account) => format!("{PROFILES}/a-{}.json", short_hash(account)),
        None => format!("{PROFILES}/e-{}.json", email_key(&profile.email)),
    }
}

fn survey_key(session: &str) -> String {
    format!("{SURVEYS}/{session}.json")
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|_| Error::Invalid("The record could not be saved."))
}

async fn create<T: Serialize>(store: &Store, key: &str, value: &T) -> Result<Kept, Error> {
    match store.write_key(key, encode(value)?, None).await {
        Ok(_) => Ok(Kept::New),
        Err(Error::Conflict) => Ok(Kept::AlreadyThere),
        Err(error) => Err(error),
    }
}

pub(crate) async fn join_waitlist(store: &Store, entry: &WaitlistEntry) -> Result<Kept, Error> {
    create(store, &waitlist_key(&entry.email), entry).await
}

pub(crate) async fn keep_survey(store: &Store, survey: &Survey) -> Result<Kept, Error> {
    create(store, &survey_key(&survey.session), survey).await
}

pub(crate) async fn survey_done(store: &Store, session: &str) -> Result<bool, Error> {
    Ok(store.read_key(&survey_key(session)).await?.is_some())
}

/// Keep `profile`, replacing an earlier one for the same account or
/// email but keeping when it was first made.
pub(crate) async fn keep_profile(store: &Store, profile: &Profile) -> Result<(), Error> {
    let key = profile_key(profile);
    for _ in 0..3 {
        let (expected, created) = match store.read_key(&key).await? {
            Some((bytes, generation)) => {
                let created = serde_json::from_slice::<Profile>(&bytes)
                    .map(|p| p.created_unix)
                    .unwrap_or(profile.created_unix);
                (Some(generation), created)
            }
            None => (None, profile.created_unix),
        };
        let mut next = profile.clone();
        next.created_unix = created.min(profile.created_unix);
        match store
            .write_key(&key, encode(&next)?, expected.as_deref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// The profile kept for `account_id`, if any.
pub(crate) async fn account_profile(
    store: &Store,
    account_id: &str,
) -> Result<Option<Profile>, Error> {
    let probe = Profile {
        account_id: Some(account_id.to_owned()),
        ..Profile::default()
    };
    Ok(store
        .read_key(&profile_key(&probe))
        .await?
        .and_then(|(bytes, _)| serde_json::from_slice(&bytes).ok()))
}

/// Remove the profile kept for `account_id`; nothing there is fine.
pub(crate) async fn forget_account(store: &Store, account_id: &str) -> Result<(), Error> {
    let probe = Profile {
        account_id: Some(account_id.to_owned()),
        ..Profile::default()
    };
    let key = profile_key(&probe);
    if let Some((_, generation)) = store.read_key(&key).await? {
        store.delete_key(&key, &generation).await?;
    }
    Ok(())
}

/// Every record in `folder` that decodes as `T`, oldest first by key.
pub(crate) async fn all<T>(store: &Store, folder: &str) -> Result<Vec<T>, Error>
where
    T: for<'de> Deserialize<'de>,
{
    let keys = store.folder_keys_limit(folder, MAX_EXPORT).await?;
    let read: Vec<Result<Option<(Vec<u8>, String)>, Error>> = stream::iter(keys)
        .map(|key| async move { store.read_key(&key).await })
        .buffered(32)
        .collect()
        .await;
    let mut records = Vec::with_capacity(read.len());
    for item in read {
        if let Some((bytes, _)) = item?
            && let Ok(record) = serde_json::from_slice(&bytes)
        {
            records.push(record);
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        (dir, store)
    }

    #[tokio::test]
    async fn the_waitlist_keeps_one_entry_per_email_whatever_its_case() {
        let (_dir, store) = store();
        let first = WaitlistEntry {
            email: "Ada@Example.com".into(),
            role: "student".into(),
            source: "hn".into(),
            created_unix: 1,
        };
        assert_eq!(join_waitlist(&store, &first).await.unwrap(), Kept::New);
        let again = WaitlistEntry {
            email: " ada@example.COM ".into(),
            created_unix: 2,
            ..first.clone()
        };
        assert_eq!(
            join_waitlist(&store, &again).await.unwrap(),
            Kept::AlreadyThere
        );
        let kept: Vec<WaitlistEntry> = all(&store, WAITLIST).await.unwrap();
        assert_eq!(kept, [first]);
    }

    #[tokio::test]
    async fn a_profile_is_replaced_but_keeps_when_it_was_first_made() {
        let (_dir, store) = store();
        let mut profile = Profile {
            account_id: Some("acct_1".into()),
            email: "ada@example.com".into(),
            role: "indie".into(),
            created_unix: 10,
            updated_unix: 10,
            ..Profile::default()
        };
        keep_profile(&store, &profile).await.unwrap();
        profile.role = "student".into();
        profile.created_unix = 20;
        profile.updated_unix = 20;
        keep_profile(&store, &profile).await.unwrap();
        let kept = account_profile(&store, "acct_1").await.unwrap().unwrap();
        assert_eq!(kept.role, "student");
        assert_eq!((kept.created_unix, kept.updated_unix), (10, 20));
        // A guest with the same email is a separate record.
        keep_profile(
            &store,
            &Profile {
                account_id: None,
                ..profile.clone()
            },
        )
        .await
        .unwrap();
        assert_eq!(all::<Profile>(&store, PROFILES).await.unwrap().len(), 2);
        forget_account(&store, "acct_1").await.unwrap();
        assert!(account_profile(&store, "acct_1").await.unwrap().is_none());
        forget_account(&store, "acct_1").await.unwrap();
    }

    #[tokio::test]
    async fn a_session_keeps_one_survey() {
        let (_dir, store) = store();
        let survey = Survey {
            session: "pt_0123456789abcdef01234567".into(),
            ease: 4,
            finished: "yes".into(),
            again: 9,
            device: "mobile".into(),
            browser: "Safari".into(),
            created_unix: 5,
            ..Survey::default()
        };
        assert!(!survey_done(&store, &survey.session).await.unwrap());
        assert_eq!(keep_survey(&store, &survey).await.unwrap(), Kept::New);
        let second = Survey {
            ease: 1,
            ..survey.clone()
        };
        assert_eq!(
            keep_survey(&store, &second).await.unwrap(),
            Kept::AlreadyThere
        );
        assert!(survey_done(&store, &survey.session).await.unwrap());
        assert_eq!(all::<Survey>(&store, SURVEYS).await.unwrap(), [survey]);
    }
}
