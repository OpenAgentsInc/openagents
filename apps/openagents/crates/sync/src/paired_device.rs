//! Pairing with a computer by its code, and the pairings this phone keeps.
//!
//! [`pair_scanned`] takes the text of a scanned or pasted code (NIP-HOST
//! connect code, connect link, or `coder-host:` invitation), redeems it,
//! and connects. It needs no prior registry entry: the computer is known
//! from the code and from its own answer. The result is a
//! [`PairedComputer`], which holds the host-signed grant and this phone's
//! key for that computer; a [`PairingStore`] keeps it (the Keychain on
//! Apple platforms, [`KeychainPairings`]) so the pairing outlives the app,
//! and [`open`] connects with it again. The same scan then links the
//! computer to the phone's account (`crate::account_link`).
use crate::device_channel::{Answer, DeviceChannel, HostError, HostLink, Request};
pub use coder_access::RelayPolicy;
use coder_access::{Code, app_rpc::Method, protocol::Access};
use coder_host::client::iroh::{self, EnrollError, IrohRoute};
use coder_host::client::{Connector, Device, Link};
use coder_link::{AttemptId, ConnectionId, Connector as _, HostKey, Report, Stage};
use coder_reach::hints::Locality;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct Paired {
    link: Arc<Link>,
    secret: SecretKey,
    _owner: Owner,
}
struct Owner {
    connector: Mutex<Connector>,
    host: HostKey,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let connector = self
            .connector
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        connector.cancel(&self.host, AttemptId(1));
        connector.close(&self.host, ConnectionId(1));
    }
}
#[async_trait::async_trait]
impl HostLink for Paired {
    async fn revoke(&self) -> Result<(), HostError> {
        match self
            .link
            .call(coder_access::protocol::Operation::Revoke {
                device: self.link.device().key(),
            })
            .await?
        {
            coder_access::protocol::Outcome::Revoked { .. } => Ok(()),
            _ => Err(refused("Couldn't remove this pairing.")),
        }
    }

    async fn proof_headers(
        &self,
        intent: coder_access::http_proof::Intent,
        account: &str,
    ) -> Result<Vec<(String, String)>, HostError> {
        use coder_access::http_proof::{self, DEVICE_HEADER, HOST_HEADER};
        let answer = self
            .app_call(Request {
                method: Method::AuthorizeSync,
                params: serde_json::to_value(&intent)
                    .map_err(|_| refused("Invalid device request."))?,
            })
            .await?;
        if answer.method != Method::AuthorizeSync {
            return Err(refused("Invalid host proof."));
        }
        let host: nostr::domain::Event =
            serde_json::from_value(answer.value).map_err(|_| refused("Invalid host proof."))?;
        let host_header = http_proof::header(&host)?;
        let now = coder_access::unix_time()?;
        http_proof::verify(&host_header, &intent, now)?;
        if host.pubkey != self.link.device().host()
            || http_proof::tag(&host, "p")? != self.link.device().key()
            || http_proof::tag(&host, "account")? != account
        {
            return Err(refused("This host proof belongs to another device."));
        }
        let signed = http_proof::sign(
            &self.secret,
            &intent,
            now,
            vec![("account", account.into()), ("host", host.id.clone())],
        )?;
        Ok(vec![
            (HOST_HEADER.into(), host_header),
            (DEVICE_HEADER.into(), http_proof::header(&signed)?),
        ])
    }
    async fn app_call(&self, request: Request) -> Result<Answer, HostError> {
        self.link.app_call(request).await
    }
}

fn refused(message: &str) -> HostError {
    HostError::Access(coder_access::Error::new(Code::Forbidden, message))
}

/// The record version [`PairedComputer`] is saved with.
const RECORD_VERSION: u32 = 1;

/// One computer this phone paired with: everything needed to connect to it
/// again. It holds this phone's private key for that computer: keep it in
/// protected storage only, never in a log.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedComputer {
    pub v: u32,
    /// The account this pairing was made under; another account on the
    /// same phone doesn't see it.
    pub account: String,
    /// The computer's app device ID (its `EngineInfo` `deviceId`).
    pub device_id: String,
    /// The computer's name, as the phone shows it.
    pub label: String,
    /// The host-signed grant for this phone.
    pub access: Access,
    /// This phone's key for the computer, 64 hex characters.
    secret: String,
    /// The computer's iroh endpoint, from a connect code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iroh: Option<IrohRoute>,
}

impl std::fmt::Debug for PairedComputer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret key stays out of debug output.
        f.debug_struct("PairedComputer")
            .field("account", &self.account)
            .field("device_id", &self.device_id)
            .field("label", &self.label)
            .field("host", &self.host())
            .finish_non_exhaustive()
    }
}

impl PairedComputer {
    /// The computer's host key.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.access.grant.host
    }

    fn key(&self) -> Option<SecretKey> {
        self.secret.parse().ok()
    }

    /// Read a saved record; `None` when it is unreadable or another version.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 32 * 1024 {
            return None;
        }
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .filter(|record| record.v == RECORD_VERSION && record.key().is_some())
    }

    /// The bytes to save.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// Where this phone keeps its pairings, by account.
pub trait PairingStore: Send + Sync {
    /// Every pairing saved under `account`.
    fn load(&self, account: &str) -> Result<Vec<PairedComputer>, StoreError>;
    /// Save one pairing, replacing any for the same account and computer.
    fn save(&self, computer: &PairedComputer) -> Result<(), StoreError>;
    /// Remove the pairing with `device_id` under `account`, if any.
    fn delete(&self, account: &str, device_id: &str) -> Result<(), StoreError>;
}

/// The store couldn't be read or written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("This phone couldn't save the computer's pairing.")]
pub struct StoreError;

/// The most pairings one account keeps on a phone.
pub const MAX_PAIRINGS: usize = 32;

/// Pairings in memory only, for tests and platforms without a store.
#[derive(Default)]
pub struct MemoryPairings(Mutex<Vec<PairedComputer>>);

impl PairingStore for MemoryPairings {
    fn load(&self, account: &str) -> Result<Vec<PairedComputer>, StoreError> {
        Ok(lock(&self.0)
            .iter()
            .filter(|c| c.account == account)
            .cloned()
            .collect())
    }
    fn save(&self, computer: &PairedComputer) -> Result<(), StoreError> {
        let mut all = lock(&self.0);
        all.retain(|c| !(c.account == computer.account && c.device_id == computer.device_id));
        if all.iter().filter(|c| c.account == computer.account).count() >= MAX_PAIRINGS {
            return Err(StoreError);
        }
        all.push(computer.clone());
        Ok(())
    }
    fn delete(&self, account: &str, device_id: &str) -> Result<(), StoreError> {
        lock(&self.0).retain(|c| !(c.account == account && c.device_id == device_id));
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Pairings in the Keychain: one generic password per computer under
/// `service`, on this device only (not in iCloud Keychain).
pub struct KeychainPairings {
    service: String,
}

impl KeychainPairings {
    /// `service` is the app's own Keychain service for pairings.
    #[must_use]
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(account: &str, device_id: &str) -> String {
        format!("{account}\n{device_id}")
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
impl PairingStore for KeychainPairings {
    fn load(&self, account: &str) -> Result<Vec<PairedComputer>, StoreError> {
        use security_framework::item::{ItemClass, ItemSearchOptions, SearchResult};
        let items = ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(&self.service)
            .load_data(true)
            .limit(256)
            .search();
        let items = match items {
            Ok(items) => items,
            Err(error) if error.code() == -25300 => return Ok(Vec::new()),
            Err(_) => return Err(StoreError),
        };
        Ok(items
            .into_iter()
            .filter_map(|item| match item {
                SearchResult::Data(mut bytes) => {
                    let record = PairedComputer::decode(&bytes);
                    bytes.fill(0);
                    record
                }
                _ => None,
            })
            .filter(|record| record.account == account)
            .take(MAX_PAIRINGS)
            .collect())
    }
    fn save(&self, computer: &PairedComputer) -> Result<(), StoreError> {
        let mut bytes = computer.encode();
        if bytes.is_empty() {
            return Err(StoreError);
        }
        let result = security_framework::passwords::set_generic_password(
            &self.service,
            &Self::entry(&computer.account, &computer.device_id),
            &bytes,
        )
        .map_err(|_| StoreError);
        bytes.fill(0);
        result
    }
    fn delete(&self, account: &str, device_id: &str) -> Result<(), StoreError> {
        match security_framework::passwords::delete_generic_password(
            &self.service,
            &Self::entry(account, device_id),
        ) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25300 => Ok(()),
            Err(_) => Err(StoreError),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
impl PairingStore for KeychainPairings {
    fn load(&self, _: &str) -> Result<Vec<PairedComputer>, StoreError> {
        Err(StoreError)
    }
    fn save(&self, _: &PairedComputer) -> Result<(), StoreError> {
        Err(StoreError)
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), StoreError> {
        Err(StoreError)
    }
}

/// Why pairing didn't add the computer, in words for the screen.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PairError(pub String);

impl PairError {
    fn new(message: &str) -> Self {
        Self(message.into())
    }
}

const UNREACHABLE: &str = "Couldn't reach your computer. Check that OpenAgents is open on it and that both are online, then scan again.";
const NOT_ACCEPTED: &str = "Your computer didn't accept this code. It may have been used already. Show a new code on your computer and scan again.";
const EXPIRED: &str = "This code has expired. Show a new code on your computer and scan again.";
const MISMATCH: &str = "The answer didn't come from the computer that showed the code. Scan the code on your own computer again.";

fn host_message(error: &HostError) -> PairError {
    PairError::new(match error {
        HostError::Access(error) if error.code == Code::Expired => EXPIRED,
        HostError::Access(error)
            if matches!(error.code, Code::Forbidden | Code::Revoked | Code::Denied) =>
        {
            NOT_ACCEPTED
        }
        _ => UNREACHABLE,
    })
}

/// Whether `text` is a computer's code, before anything is dialed: `None`
/// for a code to pair with, or the sentence to show for anything else.
#[must_use]
pub fn code_problem(text: &str) -> Option<String> {
    coder_computers::connect::classify(text).err()
}

/// A finished scan: the computer to keep and its open connection, over
/// which the phone then links the computer to its account
/// (`crate::account_link`).
pub struct Scan {
    pub computer: PairedComputer,
    pub channel: DeviceChannel,
}

/// Pair with the computer a scanned or pasted code names, under `account`,
/// and connect. A connect code goes over iroh, then over the Nostr relay
/// when iroh can't reach the computer; a host invitation goes over the
/// relay it names. No registry entry is needed.
///
/// # Errors
/// Why the computer wasn't added, in words for the screen.
pub async fn pair_scanned(
    text: &str,
    account: &str,
    policy: RelayPolicy,
) -> Result<Scan, PairError> {
    use coder_computers::connect::{Scanned, classify};
    let scanned = classify(text).map_err(PairError)?;
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let (access, route, label) = match scanned {
        Scanned::HostInvitation(code) => {
            let access = coder_access::client::redeem(&code, &secret, policy)
                .await
                .map_err(|error| host_message(&HostError::Access(error)))?;
            (access, None, String::new())
        }
        Scanned::Connect(code) => {
            let dialer = iroh::Dialer::new(secp256k1::rand::random());
            let enrolled =
                match iroh::enroll(&dialer, &code, iroh::DEFAULT_RELAY, &secret, policy).await {
                    Ok(enrolled) => enrolled,
                    Err(EnrollError::Unreachable) => {
                        iroh::enroll_on_relay(&code, iroh::DEFAULT_RELAY, &secret, policy)
                            .await
                            .map_err(|error| host_message(&error))?
                    }
                    Err(EnrollError::Mismatch) => return Err(PairError::new(MISMATCH)),
                    Err(EnrollError::Refused { error, .. }) => return Err(host_message(&error)),
                };
            (enrolled.access, Some(enrolled.route), enrolled.label)
        }
    };
    let mut computer = PairedComputer {
        v: RECORD_VERSION,
        account: account.into(),
        device_id: String::new(),
        label,
        access,
        secret: secret.display_secret().to_string(),
        iroh: route,
    };
    let (channel, device_id) = open(&computer, policy).await.map_err(|error| {
        tracing::debug!(%error, "paired computer didn't connect");
        PairError::new(UNREACHABLE)
    })?;
    computer.device_id = device_id;
    if computer.label.trim().is_empty() {
        computer.label = "Computer".into();
    }
    Ok(Scan { computer, channel })
}

/// Connect to a paired computer with its saved grant. Answers the open
/// channel and the computer's app device ID, which must match the saved
/// one when the record has one.
///
/// # Errors
/// The connection failed, or another computer answered.
pub async fn open(
    computer: &PairedComputer,
    policy: RelayPolicy,
) -> Result<(DeviceChannel, String), HostError> {
    let secret = computer
        .key()
        .ok_or_else(|| refused("This pairing can't be read."))?;
    let device = Arc::new(Device::new(computer.access.clone(), secret, policy)?);
    let (channel, device_id) = connect(device, secret, computer.iroh.clone()).await?;
    if !computer.device_id.is_empty() && computer.device_id != device_id {
        return Err(refused("This code belongs to another computer."));
    }
    Ok((channel, device_id))
}

async fn connect(
    device: Arc<Device>,
    secret: SecretKey,
    route: Option<IrohRoute>,
) -> Result<(DeviceChannel, String), HostError> {
    // Enrollment commits before the next presence publication for this device.
    let reach = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match coder_host::client::fetch_reach(&device, device.relay()).await {
                Ok(reach) => return Ok(reach),
                Err(HostError::Reach(error)) if error.code == coder_reach::Refusal::Stale => {
                    tokio::time::sleep(Duration::from_millis(250)).await
                }
                Err(error) => return Err(error),
            }
        }
    })
    .await
    .map_err(|_| refused("This computer hasn't published its device connection yet."))??;
    let capabilities = reach.presence.presence.capabilities;
    if !capabilities
        .iter()
        .any(|c| c == coder_access::app_rpc::CAPABILITY)
    {
        return Err(refused(
            "This computer doesn't support app requests over this connection.",
        ));
    }
    let (mut connector, mut reports) =
        Connector::new(tokio::runtime::Handle::current(), Locality::OtherMachine);
    let host = connector.add(device)?;
    if route.is_some() {
        connector.set_iroh(Arc::new(iroh::Dialer::new(secp256k1::rand::random())));
        connector.set_iroh_route(&host, route);
    }
    connector.open(&host, AttemptId(1), Stage::Establishing);
    let owner = Owner {
        connector: Mutex::new(connector),
        host: host.clone(),
    };
    let established = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some((key, report)) = reports.recv().await {
            if key != host {
                continue;
            }
            match report {
                Report::Established(AttemptId(1)) => return true,
                Report::Failed(AttemptId(1), _) => return false,
                _ => {}
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    if !established {
        return Err(refused("Couldn't connect to this computer."));
    }
    let link = lock(&owner.connector)
        .link(&host, ConnectionId(1))
        .ok_or_else(|| refused("The device connection closed."))?;
    let paired = Arc::new(Paired {
        link,
        secret,
        _owner: owner,
    });
    let identity = paired
        .app_call(Request {
            method: Method::EngineInfo,
            params: serde_json::json!({}),
        })
        .await?;
    let device_id = identity.value["deviceId"]
        .as_str()
        .filter(|id| identity.method == Method::EngineInfo && !id.is_empty() && id.len() <= 128)
        .ok_or_else(|| refused("This computer didn't say who it is."))?
        .to_owned();
    Ok((DeviceChannel::new(paired, true, &capabilities), device_id))
}

/// Redeem an explicit connect code and verify the selected app computer.
/// Kept for callers that pair a computer already in the registry; the
/// pairing isn't saved.
pub async fn pair(
    code: &str,
    expected_device: &str,
    expected_host: &str,
) -> Result<DeviceChannel, HostError> {
    pair_with_policy(
        code,
        expected_device,
        expected_host,
        RelayPolicy::Production,
    )
    .await
}

pub async fn pair_with_policy(
    code: &str,
    expected_device: &str,
    expected_host: &str,
    policy: RelayPolicy,
) -> Result<DeviceChannel, HostError> {
    let invitation =
        coder_access::protocol::HostInvitation::parse(code, coder_access::unix_time()?, policy)?;
    if invitation.host() != expected_host {
        return Err(refused("This code belongs to another computer."));
    }
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let access = coder_access::client::redeem(code, &secret, policy).await?;
    let device = Arc::new(Device::new(access, secret, policy)?);
    let (channel, device_id) = connect(device, secret, None).await?;
    if device_id != expected_device {
        return Err(refused("This code belongs to another computer."));
    }
    Ok(channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(account: &str, device: &str) -> PairedComputer {
        let host = SecretKey::new(&mut secp256k1::rand::rng());
        let phone = SecretKey::new(&mut secp256k1::rand::rng());
        let host_key = coder_reach::pubkey(&host);
        let phone_key = coder_reach::pubkey(&phone);
        let access: Access = serde_json::from_value(serde_json::json!({
            "v": coder_access::protocol::ACCESS,
            "requires": [],
            "grant": {
                "v": "fixture", "requires": [], "grant": "grant", "host": host_key,
                "owner": host_key, "device": phone_key,
                "relay": "wss://relay.openagents.com/", "rights": ["observe"], "epoch": 1,
                "origin": {"kind": "invitation", "id": "invite", "issuer": host_key},
                "issued_at": 1, "expires_at": 2,
            },
            "authorization": {
                "id": "0".repeat(64), "pubkey": host_key, "created_at": 1, "kind": 1,
                "tags": [], "content": "", "sig": "0".repeat(128),
            },
        }))
        .unwrap_or_else(|error| panic!("fixture access: {error}"));
        PairedComputer {
            v: RECORD_VERSION,
            account: account.into(),
            device_id: device.into(),
            label: "Studio".into(),
            access,
            secret: phone.display_secret().to_string(),
            iroh: Some(IrohRoute {
                endpoint: "ab".repeat(32),
                relay: None,
                direct: vec![],
            }),
        }
    }

    #[test]
    fn a_saved_pairing_reads_back_and_never_debugs_its_key() {
        let computer = record("acct_a", "desktop-1");
        let bytes = computer.encode();
        let back = PairedComputer::decode(&bytes).unwrap();
        assert_eq!(back.device_id, "desktop-1");
        assert_eq!(back.host(), computer.host());
        assert_eq!(back.iroh, computer.iroh);
        assert_eq!(
            back.key().map(|k| k.secret_bytes()),
            computer.key().map(|k| k.secret_bytes())
        );
        assert!(!format!("{computer:?}").contains(&computer.secret));
        // Another version, a bad key, or junk isn't read.
        let mut other: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        other["v"] = serde_json::json!(2);
        assert!(PairedComputer::decode(&serde_json::to_vec(&other).unwrap()).is_none());
        other["v"] = serde_json::json!(1);
        other["secret"] = serde_json::json!("nope");
        assert!(PairedComputer::decode(&serde_json::to_vec(&other).unwrap()).is_none());
        assert!(PairedComputer::decode(b"{}").is_none());
    }

    #[test]
    fn pairings_are_kept_per_account_and_replaced_per_computer() {
        let store = MemoryPairings::default();
        store.save(&record("acct_a", "desktop-1")).unwrap();
        store.save(&record("acct_a", "desktop-1")).unwrap();
        store.save(&record("acct_a", "desktop-2")).unwrap();
        store.save(&record("acct_b", "desktop-1")).unwrap();
        assert_eq!(store.load("acct_a").unwrap().len(), 2);
        assert_eq!(store.load("acct_b").unwrap().len(), 1);
        store.delete("acct_a", "desktop-1").unwrap();
        let left = store.load("acct_a").unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].device_id, "desktop-2");
        assert_eq!(store.load("acct_b").unwrap().len(), 1);
        for n in 0..MAX_PAIRINGS - 1 {
            store.save(&record("acct_a", &format!("more-{n}"))).unwrap();
        }
        assert_eq!(
            store.save(&record("acct_a", "one-too-many")),
            Err(StoreError)
        );
    }

    #[test]
    fn the_scanner_knows_a_computer_code_before_dialing() {
        assert_eq!(code_problem("openagents-connect:AQID"), None);
        assert_eq!(code_problem("https://openagents.com/connect#AQID"), None);
        assert_eq!(code_problem("coder-host:AAAA"), None);
        assert!(code_problem("https://example.com").is_some());
    }

    #[tokio::test]
    async fn a_scan_that_is_not_a_computer_code_says_so_before_dialing() {
        let error = pair_scanned("coder-pair:AAAA", "acct_a", RelayPolicy::LoopbackTest)
            .await
            .err()
            .unwrap();
        assert!(error.0.contains("Chats"));
        let error = pair_scanned("hello", "acct_a", RelayPolicy::LoopbackTest)
            .await
            .err()
            .unwrap();
        assert!(error.0.contains("isn't a code"));
    }
}
