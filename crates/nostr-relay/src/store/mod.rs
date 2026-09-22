//! Postgres-backed event admission and reads.
//!
//! Runtime data statements are prepared once when a [`Store`] connects.
//! Versioned, compile-time migration DDL is the sole `batch_execute` use and
//! is applied under one transaction and advisory lock.

mod error;
mod migration;
mod statements;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    future::poll_fn,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::{Value, json};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_postgres::{AsyncMessage, Client, NoTls, Row, types::ToSql};

use crate::domain::{
    BLOCK_GLOBAL_ONLY_KINDS, DM_VISIBILITY_KIND, DeletionRequest, DeletionTombstone, DomainError,
    Event, EventClass, Filter, GroupAction, GroupMetadata, IDENTITY_ARCHIVE_LIST_KIND,
    IDENTITY_ARCHIVED_KIND, IDENTITY_UNARCHIVED_KIND, IdentityArchiveRequest, RelaySigner,
    ReplacementDecision, Tag, compare_replacement_order, search_terms, validate_block_ingest,
};

pub use error::StoreError;
pub use migration::MigrationReport;
use statements::Statements;

const EVENT_CHANNEL: &str = "nostr_event";
const EPHEMERAL_CHANNEL: &str = "nostr_ephemeral";
const LISTEN_EVENT_SQL: &str = "LISTEN nostr_event";
const LISTEN_EPHEMERAL_SQL: &str = "LISTEN nostr_ephemeral";
const EPHEMERAL_CHUNK_BYTES: usize = 3_500;
const EPHEMERAL_MAX_BYTES: usize = 1_048_576;
const EPHEMERAL_MAX_ASSEMBLIES: usize = 256;
const NOSTR_EFFECT_SOURCE_EXISTS_SQL: &str = "SELECT to_regclass('public.events') IS NOT NULL";
const NOSTR_EFFECT_PENDING_SQL: &str = r#"
SELECT source.id, source.pubkey, source.created_at, source.kind,
       source.tags::text, source.content, source.sig
FROM public.events source
LEFT JOIN nostr_effect_import_ledger imported ON imported.event_id = source.id
WHERE imported.event_id IS NULL
ORDER BY source.created_at ASC,
         CASE WHEN source.kind = 9007 THEN 0 ELSE 1 END ASC,
         source.id DESC
LIMIT 1000
"#;
const NOSTR_EFFECT_REJECTED_SQL: &str = r#"
SELECT source.id, source.pubkey, source.created_at, source.kind,
       source.tags::text, source.content, source.sig
FROM public.events source
JOIN nostr_effect_import_ledger imported ON imported.event_id = source.id
WHERE imported.outcome = 'rejected'
ORDER BY source.created_at ASC,
         CASE WHEN source.kind = 9007 THEN 0 ELSE 1 END ASC,
         source.id DESC
LIMIT 1000
"#;

/// A successfully committed admission or a protocol-level refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Stored { ingest_seq: i64 },
    Duplicate,
    Ephemeral,
    Rejected(AdmissionRejection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejection {
    BlockedPubkey(String),
    BlockedKind(String),
    PubkeyNotAllowed,
    KindNotAllowed,
    NotMember,
    ContentTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    TooManyTags {
        actual: usize,
        max: usize,
    },
    TimestampTooFarInFuture {
        created_at: u64,
        latest_allowed: u64,
    },
    TimestampTooOld {
        created_at: u64,
        earliest_allowed: u64,
    },
    AuthEvent,
    Deleted,
    Superseded,
    GroupNotFound,
    GroupUnauthorized,
    GroupClosed,
    GroupAlreadyMember,
    GroupUnsupportedKind,
    GroupPreviousUnknown,
    GroupSigningUnavailable,
}

impl AdmissionRejection {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BlockedPubkey(_) => "blocked_pubkey",
            Self::BlockedKind(_) => "blocked_kind",
            Self::PubkeyNotAllowed => "pubkey_not_allowed",
            Self::KindNotAllowed => "kind_not_allowed",
            Self::NotMember => "not_member",
            Self::ContentTooLarge { .. } => "content_too_large",
            Self::TooManyTags { .. } => "too_many_tags",
            Self::TimestampTooFarInFuture { .. } => "timestamp_future",
            Self::TimestampTooOld { .. } => "timestamp_old",
            Self::AuthEvent => "auth_event",
            Self::Deleted => "deleted",
            Self::Superseded => "superseded",
            Self::GroupNotFound => "group_not_found",
            Self::GroupUnauthorized => "group_unauthorized",
            Self::GroupClosed => "group_closed",
            Self::GroupAlreadyMember => "group_already_member",
            Self::GroupUnsupportedKind => "group_unsupported_kind",
            Self::GroupPreviousUnknown => "group_previous_unknown",
            Self::GroupSigningUnavailable => "group_signing_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagementRequest {
    BanPubkey {
        pubkey: String,
        reason: String,
    },
    UnbanPubkey {
        pubkey: String,
    },
    ListBannedPubkeys,
    AllowPubkey {
        pubkey: String,
        reason: String,
    },
    UnallowPubkey {
        pubkey: String,
    },
    ListAllowedPubkeys,
    AllowKind {
        kind: u16,
    },
    DisallowKind {
        kind: u16,
    },
    ListAllowedKinds,
    CreateGroup {
        id: String,
        metadata: GroupMetadata,
        admin_pubkey: String,
    },
    DeleteGroup {
        id: String,
    },
    ListGroups,
    PutGroupUser {
        id: String,
        pubkey: String,
        roles: Vec<String>,
    },
    RemoveGroupUser {
        id: String,
        pubkey: String,
    },
}

/// The operator-owned limits that the gateway advertises in NIP-11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayPolicy {
    pub closed_membership: bool,
    pub max_content_bytes: usize,
    pub max_tags: usize,
    pub max_future_seconds: u64,
    pub max_past_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityStatus {
    pub closed_membership: bool,
    pub direct_member: bool,
}

/// A committed durable position or a validated ephemeral event delivered
/// through Postgres `NOTIFY` without ever entering a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreNotification {
    Stored(i64),
    Ephemeral(Event),
}

/// A decoded stored event and its relay-local monotonic position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub event: Event,
    pub ingest_seq: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRecord {
    pub sha256: String,
    pub storage_key: String,
    pub size: u64,
    pub media_type: String,
    pub uploaded_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaUploadOutcome {
    pub record: MediaRecord,
    /// The blob row is new; no upload of these bytes was registered before.
    pub created: bool,
    /// The uploader already owned these bytes before this registration.
    pub owned_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaDeleteOutcome {
    NotOwned,
    OwnerRemoved,
    BlobRemoved(MediaRecord),
}

/// One bounded compatibility-import sweep from nostr-effect's event table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyImportReport {
    pub scanned: usize,
    pub stored: usize,
    pub duplicate: usize,
    pub ephemeral: usize,
    pub expired: usize,
    pub rejected: usize,
    pub rejection_reasons: BTreeMap<String, usize>,
}

impl LegacyImportReport {
    pub fn is_empty(&self) -> bool {
        self.scanned == 0
    }

    pub fn merge(&mut self, other: &Self) {
        self.scanned += other.scanned;
        self.stored += other.stored;
        self.duplicate += other.duplicate;
        self.ephemeral += other.ephemeral;
        self.expired += other.expired;
        self.rejected += other.rejected;
        for (reason, count) in &other.rejection_reasons {
            *self.rejection_reasons.entry(reason.clone()).or_default() += count;
        }
    }
}

/// One database connection and its complete prepared-statement set.
///
/// The M3 gateway can create a small fixed set of these workers; M2 does not
/// add a pool or another service.
pub struct Store {
    client: Client,
    statements: Statements,
    connection_current: Arc<AtomicBool>,
    connection_task: JoinHandle<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdmissionMode {
    Public,
    Legacy,
}

impl Store {
    pub async fn connect(config: &str) -> Result<Self, StoreError> {
        let (store, _) = Self::connect_with_report(config).await?;
        Ok(store)
    }

    pub async fn connect_with_report(config: &str) -> Result<(Self, MigrationReport), StoreError> {
        let (mut client, connection_current, connection_task) = open_client(config).await?;
        let migration_report = migration::apply(&mut client).await?;
        let statements = Statements::prepare(&client).await?;
        Ok((
            Self {
                client,
                statements,
                connection_current,
                connection_task,
            },
            migration_report,
        ))
    }

    /// Connect with a runtime role that may read the migration ledger but has
    /// no schema-creation privileges. This fails closed on missing, changed,
    /// or unknown migration versions.
    pub async fn connect_verified(config: &str) -> Result<Self, StoreError> {
        let (client, connection_current, connection_task) = open_client(config).await?;
        migration::verify(&client).await?;
        let statements = Statements::prepare(&client).await?;
        Ok(Self {
            client,
            statements,
            connection_current,
            connection_task,
        })
    }

    pub fn is_current(&self) -> bool {
        self.connection_current.load(Ordering::Acquire)
    }

    fn ensure_current(&self) -> Result<(), StoreError> {
        if self.is_current() {
            Ok(())
        } else {
            Err(StoreError::ConnectionClosed)
        }
    }

    /// Validate and atomically admit one event. A `Stored` result is returned
    /// only after the transaction, including its `NOTIFY`, has committed.
    pub async fn admit(&mut self, event: &Event, now: u64) -> Result<AdmissionOutcome, StoreError> {
        self.admit_with_signer(event, now, None).await
    }

    pub async fn admit_with_signer(
        &mut self,
        event: &Event,
        now: u64,
        relay_signer: Option<&RelaySigner>,
    ) -> Result<AdmissionOutcome, StoreError> {
        self.admit_with_identity(event, now, relay_signer, None)
            .await
    }

    /// Admit an event for a NIP-AA virtual member authenticated on the owning
    /// connection. The owner mapping is checked in the same policy transaction;
    /// no persistent relay-membership row is created for the agent.
    pub async fn admit_with_identity(
        &mut self,
        event: &Event,
        now: u64,
        relay_signer: Option<&RelaySigner>,
        virtual_owner: Option<&str>,
    ) -> Result<AdmissionOutcome, StoreError> {
        self.admit_inner(
            event,
            now,
            relay_signer,
            virtual_owner,
            AdmissionMode::Public,
        )
        .await
    }

    async fn admit_legacy(
        &mut self,
        event: &Event,
        now: u64,
        relay_signer: Option<&RelaySigner>,
    ) -> Result<AdmissionOutcome, StoreError> {
        event.validate_nip01_structure()?;
        self.admit_inner(event, now, relay_signer, None, AdmissionMode::Legacy)
            .await
    }

    /// Replay one already-signed historical event without retroactively
    /// imposing extension handlers that the source relay did not enforce.
    /// NIP-01 crypto, replacement/deletion, expiration, and current operator
    /// policy remain authoritative.
    pub async fn admit_historical(
        &mut self,
        event: &Event,
        now: u64,
    ) -> Result<AdmissionOutcome, StoreError> {
        self.admit_legacy(event, now, None).await
    }

    async fn admit_inner(
        &mut self,
        event: &Event,
        now: u64,
        relay_signer: Option<&RelaySigner>,
        virtual_owner: Option<&str>,
        mode: AdmissionMode,
    ) -> Result<AdmissionOutcome, StoreError> {
        self.ensure_current()?;
        if mode == AdmissionMode::Public {
            event.validate_structure()?;
            event.validate_crypto()?;
            validate_block_ingest(event, now).map_err(crate::domain::DomainError::InvalidEvent)?;
        } else {
            // Compatibility imports retain the exact signed preimage, but do
            // not retroactively impose server-side extensions adopted after
            // the source relay accepted the event. New network admissions
            // always use the strict public path above.
            event.validate_crypto()?;
        }
        if event.kind == 22_242 {
            return Ok(AdmissionOutcome::Rejected(AdmissionRejection::AuthEvent));
        }
        if let Some(expiration) = event.expiration()
            && expiration <= now
        {
            return Err(crate::domain::DomainError::ExpiredEvent { expiration, now }.into());
        }
        let created_at = pg_i64(event.created_at, "created_at")?;
        let expires_at = event
            .expiration()
            .map(|value| pg_i64(value, "expiration"))
            .transpose()?;
        let kind = i32::from(event.kind);
        let replacement = event.replacement_address();
        let deletion = if event.kind == 5 {
            Some(DeletionRequest::from_event(event)?)
        } else {
            None
        };
        let group_action = if mode == AdmissionMode::Public {
            GroupAction::from_event(event)?
        } else {
            None
        };
        let lock_keys = admission_lock_keys(event, replacement.as_ref(), deletion.as_ref());
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;

        if transaction
            .query_opt(&statements.duplicate, &[&event.id])
            .await?
            .is_some()
        {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Duplicate);
        }

        let policy_row = transaction
            .query_opt(&statements.policy, &[])
            .await?
            .ok_or_else(|| StoreError::InvalidPolicy("singleton row is missing".to_owned()))?;
        let policy = AdmissionPolicy::from_row(&policy_row)?;

        if let Some(row) = transaction
            .query_opt(&statements.blocked_pubkey, &[&event.pubkey])
            .await?
        {
            let reason = row.get::<_, String>(0);
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::BlockedPubkey(reason),
            ));
        }
        if let Some(row) = transaction
            .query_opt(&statements.blocked_kind, &[&kind])
            .await?
        {
            let reason = row.get::<_, String>(0);
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(AdmissionRejection::BlockedKind(
                reason,
            )));
        }
        let pubkey_allowed = transaction
            .query_one(&statements.allowed_pubkey, &[&event.pubkey])
            .await?
            .get::<_, bool>(0);
        if !pubkey_allowed {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::PubkeyNotAllowed,
            ));
        }
        let kind_allowed = transaction
            .query_one(&statements.allowed_kind, &[&kind])
            .await?
            .get::<_, bool>(0);
        if !kind_allowed {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::KindNotAllowed,
            ));
        }
        if policy.closed_membership {
            let direct_member = transaction
                .query_opt(&statements.member, &[&event.pubkey])
                .await?
                .is_some();
            let virtual_member = if direct_member {
                false
            } else if let Some(owner) = virtual_owner {
                transaction
                    .query_opt(&statements.agent_owner, &[&event.pubkey, &owner])
                    .await?
                    .is_some()
            } else {
                false
            };
            if !direct_member && !virtual_member {
                transaction.commit().await?;
                return Ok(AdmissionOutcome::Rejected(AdmissionRejection::NotMember));
            }
        }
        if event.content.len() > policy.max_content_bytes {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::ContentTooLarge {
                    actual_bytes: event.content.len(),
                    max_bytes: policy.max_content_bytes,
                },
            ));
        }
        if event.tags.len() > policy.max_tags {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::TooManyTags {
                    actual: event.tags.len(),
                    max: policy.max_tags,
                },
            ));
        }
        let latest_allowed = now.saturating_add(policy.max_future_seconds);
        if event.created_at > latest_allowed {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::TimestampTooFarInFuture {
                    created_at: event.created_at,
                    latest_allowed,
                },
            ));
        }
        if policy.max_past_seconds > 0 {
            let earliest_allowed = now.saturating_sub(policy.max_past_seconds);
            if event.created_at < earliest_allowed {
                transaction.commit().await?;
                return Ok(AdmissionOutcome::Rejected(
                    AdmissionRejection::TimestampTooOld {
                        created_at: event.created_at,
                        earliest_allowed,
                    },
                ));
            }
        }

        for lock_key in lock_keys {
            transaction
                .query_one(&statements.advisory_lock, &[&lock_key])
                .await?;
        }

        let group_id = (mode == AdmissionMode::Public)
            .then(|| group_scope(event))
            .flatten();
        if mode == AdmissionMode::Public
            && (39_000..=39_005).contains(&event.kind)
            && relay_signer.is_none_or(|signer| signer.pubkey() != event.pubkey)
        {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Rejected(
                AdmissionRejection::GroupUnauthorized,
            ));
        }
        if let Some(group_id) = group_id {
            let group = load_group(&transaction, &statements, group_id).await?;
            if !group_previous_references_are_current(
                &transaction,
                &statements,
                event,
                group_id,
                now,
            )
            .await?
            {
                transaction.commit().await?;
                return Ok(AdmissionOutcome::Rejected(
                    AdmissionRejection::GroupPreviousUnknown,
                ));
            }
            match &group_action {
                Some(GroupAction::CreateGroup) => {
                    if group.is_some()
                        || relay_signer.is_none_or(|signer| signer.pubkey() != event.pubkey)
                    {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(
                            AdmissionRejection::GroupUnauthorized,
                        ));
                    }
                }
                _ if group.is_none() => {
                    transaction.commit().await?;
                    return Ok(AdmissionOutcome::Rejected(
                        AdmissionRejection::GroupNotFound,
                    ));
                }
                Some(GroupAction::Join { code }) => {
                    if group_member(&transaction, &statements, group_id, &event.pubkey)
                        .await?
                        .is_some()
                    {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(
                            AdmissionRejection::GroupAlreadyMember,
                        ));
                    }
                    if group.as_ref().is_some_and(|group| group.closed)
                        && !valid_group_invite(&transaction, &statements, group_id, code.as_deref())
                            .await?
                    {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(AdmissionRejection::GroupClosed));
                    }
                }
                Some(GroupAction::Leave) => {
                    if group_member(&transaction, &statements, group_id, &event.pubkey)
                        .await?
                        .is_none()
                    {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(
                            AdmissionRejection::GroupUnauthorized,
                        ));
                    }
                }
                Some(_) => {
                    let relay_author =
                        relay_signer.is_some_and(|signer| signer.pubkey() == event.pubkey);
                    let admin = group_member(&transaction, &statements, group_id, &event.pubkey)
                        .await?
                        .is_some_and(|roles| !roles.is_empty());
                    if !relay_author && !admin {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(
                            AdmissionRejection::GroupUnauthorized,
                        ));
                    }
                }
                None => {
                    if group_member(&transaction, &statements, group_id, &event.pubkey)
                        .await?
                        .is_none()
                    {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(
                            AdmissionRejection::GroupUnauthorized,
                        ));
                    }
                    if group.as_ref().is_some_and(|group| {
                        group
                            .supported_kinds
                            .as_ref()
                            .is_some_and(|kinds| !kinds.contains(&event.kind))
                    }) {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(
                            AdmissionRejection::GroupUnsupportedKind,
                        ));
                    }
                }
            }
            if group_action.is_some() && relay_signer.is_none() {
                transaction.commit().await?;
                return Ok(AdmissionOutcome::Rejected(
                    AdmissionRejection::GroupSigningUnavailable,
                ));
            }
        }

        // A conflicting process may have committed while this transaction
        // waited for an event lock.
        if transaction
            .query_opt(&statements.duplicate, &[&event.id])
            .await?
            .is_some()
        {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Duplicate);
        }

        if event.kind != 5 {
            let replacement_identifier = replacement
                .as_ref()
                .map(|address| address.identifier.as_str());
            let params: &[&(dyn ToSql + Sync)] = &[
                &event.id,
                &event.pubkey,
                &kind,
                &replacement_identifier,
                &created_at,
            ];
            let deleted = transaction
                .query_one(&statements.tombstone_match, params)
                .await?
                .get::<_, bool>(0);
            if deleted {
                transaction.commit().await?;
                return Ok(AdmissionOutcome::Rejected(AdmissionRejection::Deleted));
            }
        }

        let mut replaced_event_id = None;
        if let Some(address) = &replacement {
            let address_kind = i32::from(address.kind);
            let params: &[&(dyn ToSql + Sync)] =
                &[&address_kind, &address.pubkey, &address.identifier];
            if let Some(row) = transaction.query_opt(&statements.head, params).await? {
                let current_id = row.get::<_, String>(0);
                let current_created_at = row.get::<_, i64>(1);
                let current_created_at = u64::try_from(current_created_at).map_err(|_| {
                    StoreError::CorruptRow("replaceable head has negative timestamp".to_owned())
                })?;
                match compare_replacement_order(
                    current_created_at,
                    &current_id,
                    event.created_at,
                    &event.id,
                ) {
                    ReplacementDecision::KeepCurrent => {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Rejected(AdmissionRejection::Superseded));
                    }
                    ReplacementDecision::Duplicate => {
                        transaction.commit().await?;
                        return Ok(AdmissionOutcome::Duplicate);
                    }
                    ReplacementDecision::ReplaceCurrent => {
                        replaced_event_id = Some(current_id);
                    }
                }
            }
        }

        if event.class() == EventClass::Ephemeral {
            notify_ephemeral(&transaction, &statements, event).await?;
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Ephemeral);
        }

        // Serialize sequence allocation and commit order across processes.
        // This makes an `ingest_seq` high-water mark a safe EOSE boundary.
        transaction.query_one(&statements.ingest_lock, &[]).await?;

        let tags_json = serde_json::to_string(&event.tags)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        let replacement_identifier = replacement
            .as_ref()
            .map(|address| address.identifier.as_str());
        let insert_params: &[&(dyn ToSql + Sync)] = &[
            &event.id,
            &event.pubkey,
            &created_at,
            &kind,
            &tags_json,
            &event.content,
            &event.sig,
            &replacement_identifier,
            &expires_at,
        ];
        let Some(row) = transaction
            .query_opt(&statements.insert_event, insert_params)
            .await?
        else {
            transaction.commit().await?;
            return Ok(AdmissionOutcome::Duplicate);
        };
        let ingest_seq = row.get::<_, i64>(0);

        for (tag_name, tag_value) in event.indexed_tags() {
            let tag_name = tag_name.to_string();
            let params: &[&(dyn ToSql + Sync)] = &[&event.id, &tag_name, &tag_value, &created_at];
            transaction.execute(&statements.insert_tag, params).await?;
        }

        if let Some(address) = &replacement {
            let address_kind = i32::from(address.kind);
            let params: &[&(dyn ToSql + Sync)] = &[
                &address_kind,
                &address.pubkey,
                &address.identifier,
                &event.id,
                &created_at,
            ];
            transaction.execute(&statements.upsert_head, params).await?;
            if let Some(old_id) = replaced_event_id {
                transaction
                    .execute(&statements.delete_event, &[&old_id])
                    .await?;
            }
        }

        if let Some(request) = deletion {
            apply_deletion(&transaction, &statements, &request).await?;
        }

        if let (Some(group_id), Some(action)) = (group_id, group_action.as_ref()) {
            apply_group_action(&transaction, &statements, group_id, &event.pubkey, action).await?;
            if !matches!(action, GroupAction::DeleteGroup) {
                if matches!(action, GroupAction::Join { .. } | GroupAction::Leave) {
                    let signer = relay_signer.expect("group actions require a relay signer");
                    let (kind, content) = if matches!(action, GroupAction::Join { .. }) {
                        (9_000, "accepted group join")
                    } else {
                        (9_001, "accepted group leave")
                    };
                    let relay_action = signer.sign(
                        now,
                        kind,
                        vec![
                            Tag::new(vec!["h".into(), group_id.into()]),
                            Tag::new(vec!["p".into(), event.pubkey.clone()]),
                            Tag::new(vec!["e".into(), event.id.clone()]),
                        ],
                        content.into(),
                    );
                    insert_internal_regular_event(&transaction, &statements, &relay_action).await?;
                }
                generate_group_metadata(
                    &transaction,
                    &statements,
                    relay_signer.expect("group actions require a relay signer"),
                    group_id,
                    now,
                )
                .await?;
            }
        }

        let payload = ingest_seq.to_string();
        transaction
            .query_one(&statements.notify, &[&payload])
            .await?;
        transaction.commit().await?;
        Ok(AdmissionOutcome::Stored { ingest_seq })
    }

    /// Import at most 1,000 unprocessed rows from nostr-effect's legacy
    /// `public.events` table. Every decodable event retains cryptographic,
    /// replacement, deletion, and policy checks. Historical events bypass
    /// only newer extension validation and group-derived writes that the
    /// source relay never enforced. The source table is never modified. A
    /// durable per-ID ledger makes repeated and overlapping sweeps idempotent.
    pub async fn import_nostr_effect_events(
        &mut self,
        now: u64,
        relay_signer: Option<&RelaySigner>,
    ) -> Result<LegacyImportReport, StoreError> {
        self.ensure_current()?;
        let source_exists = self.client.prepare(NOSTR_EFFECT_SOURCE_EXISTS_SQL).await?;
        if !self
            .client
            .query_one(&source_exists, &[])
            .await?
            .get::<_, bool>(0)
        {
            return Err(StoreError::LegacyImport(
                "public.events does not exist".to_owned(),
            ));
        }
        // This statement is deliberately prepared only after the optional
        // source table has been proven to exist. Preparing it at every Store
        // connection would make normal fresh deployments fail closed.
        let pending = self.client.prepare(NOSTR_EFFECT_PENDING_SQL).await?;
        self.import_nostr_effect_rows(&pending, now, relay_signer)
            .await
    }

    /// Retry one bounded batch previously rejected during the first pass.
    /// This is intentionally separate from the drain loop so a permanently
    /// incompatible legacy row cannot prevent the listener from binding.
    pub async fn retry_rejected_nostr_effect_events(
        &mut self,
        now: u64,
        relay_signer: Option<&RelaySigner>,
    ) -> Result<LegacyImportReport, StoreError> {
        self.ensure_current()?;
        let rejected = self.client.prepare(NOSTR_EFFECT_REJECTED_SQL).await?;
        self.import_nostr_effect_rows(&rejected, now, relay_signer)
            .await
    }

    async fn import_nostr_effect_rows(
        &mut self,
        select: &tokio_postgres::Statement,
        now: u64,
        relay_signer: Option<&RelaySigner>,
    ) -> Result<LegacyImportReport, StoreError> {
        let rows = self.client.query(select, &[]).await?;
        let mut report = LegacyImportReport::default();
        for row in rows {
            report.scanned += 1;
            let source_id = row.get::<_, String>(0);
            let ledger_outcome = match decode_nostr_effect_event(&row) {
                Ok(event) if event.is_expired(now) => {
                    report.expired += 1;
                    "expired"
                }
                Ok(event) => match self.admit_legacy(&event, now, relay_signer).await {
                    Ok(AdmissionOutcome::Stored { .. }) => {
                        report.stored += 1;
                        "stored"
                    }
                    Ok(AdmissionOutcome::Duplicate) => {
                        report.duplicate += 1;
                        "duplicate"
                    }
                    Ok(AdmissionOutcome::Ephemeral) => {
                        report.ephemeral += 1;
                        "ephemeral"
                    }
                    Ok(AdmissionOutcome::Rejected(rejection)) => {
                        report.rejected += 1;
                        record_import_rejection(&mut report, admission_rejection_code(&rejection));
                        "rejected"
                    }
                    Err(StoreError::Domain(error)) => {
                        report.rejected += 1;
                        record_import_rejection(&mut report, domain_error_code(&error));
                        "rejected"
                    }
                    Err(StoreError::TimestampOutOfRange { .. }) => {
                        report.rejected += 1;
                        record_import_rejection(&mut report, "timestamp_out_of_range");
                        "rejected"
                    }
                    Err(StoreError::Serialization(_)) => {
                        report.rejected += 1;
                        record_import_rejection(&mut report, "serialization");
                        "rejected"
                    }
                    Err(StoreError::CorruptRow(_)) => {
                        report.rejected += 1;
                        record_import_rejection(&mut report, "corrupt_row");
                        "rejected"
                    }
                    Err(error) => return Err(error),
                },
                Err(_) => {
                    report.rejected += 1;
                    record_import_rejection(&mut report, "decode");
                    "rejected"
                }
            };
            self.client
                .execute(
                    &self.statements.record_nostr_effect_import,
                    &[&source_id, &ledger_outcome],
                )
                .await?;
        }
        Ok(report)
    }

    pub async fn event_by_id(&self, id: &str, now: u64) -> Result<Option<StoredEvent>, StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        self.client
            .query_opt(&self.statements.event_by_id, &[&id, &now])
            .await?
            .map(decode_event_row)
            .transpose()
    }

    pub async fn latest_ingest_seq(&self) -> Result<i64, StoreError> {
        self.ensure_current()?;
        Ok(self
            .client
            .query_one(&self.statements.latest_ingest, &[])
            .await?
            .get(0))
    }

    pub async fn relay_policy(&self) -> Result<RelayPolicy, StoreError> {
        self.ensure_current()?;
        let row = self
            .client
            .query_opt(&self.statements.policy, &[])
            .await?
            .ok_or_else(|| StoreError::InvalidPolicy("singleton row is missing".to_owned()))?;
        Ok(AdmissionPolicy::from_row(&row)?.into())
    }

    pub async fn identity_status(&self, pubkey: &str) -> Result<IdentityStatus, StoreError> {
        self.ensure_current()?;
        let policy_row = self
            .client
            .query_opt(&self.statements.policy, &[])
            .await?
            .ok_or_else(|| StoreError::InvalidPolicy("singleton row is missing".to_owned()))?;
        let closed_membership = AdmissionPolicy::from_row(&policy_row)?.closed_membership;
        let direct_member = self
            .client
            .query_opt(&self.statements.member, &[&pubkey])
            .await?
            .is_some();
        Ok(IdentityStatus {
            closed_membership,
            direct_member,
        })
    }

    /// Persist a verified owner relation with Buzz-compatible first-mint-wins
    /// semantics. Repeating the same relation succeeds; a conflicting owner
    /// cannot replace the main owner.
    pub async fn materialize_agent_owner(
        &mut self,
        agent_pubkey: &str,
        owner_pubkey: &str,
        require_owner_member: bool,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        if require_owner_member
            && transaction
                .query_opt(&statements.member, &[&owner_pubkey])
                .await?
                .is_none()
        {
            transaction.commit().await?;
            return Ok(false);
        }
        transaction
            .execute(
                &statements.insert_agent_owner,
                &[&agent_pubkey, &owner_pubkey],
            )
            .await?;
        let matches = transaction
            .query_opt(&statements.agent_owner, &[&agent_pubkey, &owner_pubkey])
            .await?
            .is_some();
        transaction.commit().await?;
        Ok(matches)
    }

    pub async fn is_agent_owner(
        &self,
        agent_pubkey: &str,
        owner_pubkey: &str,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        Ok(self
            .client
            .query_opt(
                &self.statements.agent_owner,
                &[&agent_pubkey, &owner_pubkey],
            )
            .await?
            .is_some())
    }

    pub async fn workspace_icon(&self) -> Result<Option<String>, StoreError> {
        self.ensure_current()?;
        Ok(self
            .client
            .query_one(&self.statements.workspace_icon, &[])
            .await?
            .get(0))
    }

    /// True when a channel window may be served to `reader`.
    ///
    /// A missing group and an open group are readable. A closed group is
    /// readable only for a recorded member. A hidden channel is
    /// indistinguishable from an empty response with no bounds overlay.
    pub async fn channel_window_served(
        &mut self,
        channel: &str,
        reader: &str,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let transaction = self.client.transaction().await?;
        let served = match load_group(&transaction, &self.statements, channel).await? {
            None => true,
            Some(group) if !group.closed => true,
            Some(_) => group_member(&transaction, &self.statements, channel, reader)
                .await?
                .is_some(),
        };
        Ok(served)
    }

    /// Stored push leases. This read is the executor's, not a client's.
    pub async fn push_leases(&mut self, now: u64, limit: i64) -> Result<Vec<Event>, StoreError> {
        self.ensure_current()?;
        let now = i64::try_from(now).map_err(|_| StoreError::CorruptRow("now".to_owned()))?;
        let rows = self
            .client
            .query(
                "SELECT id, pubkey, created_at, kind, tags::text, content, sig, ingest_seq
                 FROM nostr_event
                 WHERE kind = 30350 AND (expires_at IS NULL OR expires_at > $1)
                 ORDER BY created_at DESC
                 LIMIT $2",
                &[&now, &limit],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_event_row(row).map(|stored| stored.event))
            .collect()
    }

    pub async fn set_workspace_icon(
        &mut self,
        event: &Event,
        icon: &str,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        let kind = i32::from(event.kind);
        if transaction
            .query_opt(
                &statements.accept_block_command,
                &[&event.id, &event.pubkey, &kind],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Ok(false);
        }
        let icon = (!icon.is_empty()).then_some(icon);
        transaction
            .execute(&statements.set_workspace_icon, &[&icon, &event.id])
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn process_identity_archive(
        &mut self,
        event: &Event,
        request: &IdentityArchiveRequest,
        consent: &str,
        now: u64,
        signer: &RelaySigner,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        let kind = i32::from(event.kind);
        if transaction
            .query_opt(
                &statements.accept_block_command,
                &[&event.id, &event.pubkey, &kind],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Ok(false);
        }

        let changed = if request.archive {
            transaction
                .query_opt(
                    &statements.upsert_archived_identity,
                    &[
                        &request.target,
                        &request.reason,
                        &request.replaced_by,
                        &consent,
                        &event.pubkey,
                        &event.id,
                    ],
                )
                .await?
                .is_some()
        } else {
            transaction
                .query_opt(&statements.delete_archived_identity, &[&request.target])
                .await?
                .is_some()
        };
        if changed {
            transaction.query_one(&statements.ingest_lock, &[]).await?;
            let mut delta_tags = vec![
                Tag::new(vec!["-".into()]),
                Tag::new(vec!["p".into(), request.target.clone()]),
                Tag::new(vec!["consent".into(), consent.into(), event.pubkey.clone()]),
                Tag::new(vec!["e".into(), event.id.clone()]),
            ];
            if let Some(reason) = &request.reason {
                delta_tags.push(Tag::new(vec!["reason".into(), reason.clone()]));
            }
            if let Some(replaced_by) = &request.replaced_by {
                delta_tags.push(Tag::new(vec!["replaced-by".into(), replaced_by.clone()]));
            }
            let delta_kind = if request.archive {
                IDENTITY_ARCHIVED_KIND
            } else {
                IDENTITY_UNARCHIVED_KIND
            };
            let delta = signer.sign(now, delta_kind, delta_tags, event.content.clone());
            insert_internal_regular_event(&transaction, &statements, &delta).await?;

            let archived = transaction
                .query(&statements.list_archived_identities, &[])
                .await?
                .into_iter()
                .map(|row| row.get::<_, String>(0))
                .collect::<Vec<_>>();
            let mut snapshot_tags = vec![Tag::new(vec!["-".into()])];
            snapshot_tags.extend(
                archived
                    .into_iter()
                    .map(|pubkey| Tag::new(vec!["p".into(), pubkey])),
            );
            insert_internal_replaceable_event(
                &transaction,
                &statements,
                signer,
                IDENTITY_ARCHIVE_LIST_KIND,
                "",
                now,
                snapshot_tags,
                String::new(),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(changed)
    }

    pub async fn process_dm_visibility(
        &mut self,
        event: &Event,
        channel: &str,
        hidden: bool,
        now: u64,
        signer: &RelaySigner,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        if transaction
            .query_opt(&statements.group_member, &[&channel, &event.pubkey])
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Err(StoreError::Management(
                "DM visibility actor is not a member of the target group".into(),
            ));
        }
        let kind = i32::from(event.kind);
        if transaction
            .query_opt(
                &statements.accept_block_command,
                &[&event.id, &event.pubkey, &kind],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Ok(false);
        }
        let changed = if hidden {
            transaction
                .query_opt(&statements.insert_dm_hidden, &[&event.pubkey, &channel])
                .await?
                .is_some()
        } else {
            transaction
                .query_opt(&statements.delete_dm_hidden, &[&event.pubkey, &channel])
                .await?
                .is_some()
        };
        if changed {
            transaction.query_one(&statements.ingest_lock, &[]).await?;
            let hidden_groups = transaction
                .query(&statements.list_dm_hidden, &[&event.pubkey])
                .await?
                .into_iter()
                .map(|row| row.get::<_, String>(0))
                .collect::<Vec<_>>();
            let mut tags = vec![
                Tag::new(vec!["d".into(), event.pubkey.clone()]),
                Tag::new(vec!["p".into(), event.pubkey.clone()]),
            ];
            tags.extend(
                hidden_groups
                    .into_iter()
                    .map(|group| Tag::new(vec!["h".into(), group])),
            );
            insert_internal_replaceable_event(
                &transaction,
                &statements,
                signer,
                DM_VISIBILITY_KIND,
                &event.pubkey,
                now,
                tags,
                String::new(),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(changed)
    }

    pub async fn event_by_ingest_seq(
        &self,
        ingest_seq: i64,
        now: u64,
    ) -> Result<Option<StoredEvent>, StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        self.client
            .query_opt(&self.statements.event_by_ingest, &[&ingest_seq, &now])
            .await?
            .map(decode_event_row)
            .transpose()
    }

    /// Read a bounded, stable catch-up page through a previously sampled
    /// high-water mark.
    pub async fn events_after(
        &self,
        after: i64,
        through: i64,
        now: u64,
        limit: usize,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        let limit = pg_limit(limit)?;
        let rows = self
            .client
            .query(
                &self.statements.events_after,
                &[&after, &through, &now, &limit],
            )
            .await?;
        rows.into_iter().map(decode_event_row).collect()
    }

    /// Execute one bounded NIP-01 filter with one immutable prepared query.
    pub async fn query_filter(
        &self,
        filter: &Filter,
        now: u64,
        max_results: usize,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.query_filter_through(filter, now, max_results, i64::MAX)
            .await
    }

    /// Execute a historical filter through a stable durable high-water mark.
    pub async fn query_filter_through(
        &self,
        filter: &Filter,
        now: u64,
        max_results: usize,
        through: i64,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.query_filter_inner(filter, now, max_results, through, None, None)
            .await
    }

    /// As [`Store::query_filter_through`], but issue a Postgres cancellation
    /// request when the owning client disconnects or replaces the REQ.
    pub async fn query_filter_cancellable(
        &self,
        filter: &Filter,
        now: u64,
        max_results: usize,
        through: i64,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.query_filter_inner(filter, now, max_results, through, Some(cancel), None)
            .await
    }

    pub async fn query_filter_for(
        &self,
        filter: &Filter,
        now: u64,
        max_results: usize,
        through: i64,
        cancel: watch::Receiver<bool>,
        read_pubkeys: &[String],
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.query_filter_inner(
            filter,
            now,
            max_results,
            through,
            Some(cancel),
            Some(read_pubkeys),
        )
        .await
    }

    async fn query_filter_inner(
        &self,
        filter: &Filter,
        now: u64,
        max_results: usize,
        through: i64,
        mut cancel: Option<watch::Receiver<bool>>,
        read_pubkeys: Option<&[String]>,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        self.ensure_current()?;
        filter.validate()?;
        let now = pg_i64(now, "now")?;
        let requested_limit = filter.limit.unwrap_or(max_results).min(max_results);
        let limit = pg_limit(requested_limit)?;
        let ids = filter.ids.clone();
        let authors = filter.authors.clone();
        let kinds = filter.kinds.as_ref().map(|values| {
            values
                .iter()
                .map(|value| i32::from(*value))
                .collect::<Vec<_>>()
        });
        let since = filter
            .since
            .map(|value| pg_i64(value, "since"))
            .transpose()?;
        let until = filter
            .until
            .map(|value| pg_i64(value, "until"))
            .transpose()?;
        let tag_map = filter
            .tags
            .iter()
            .map(|(key, values)| (key.to_string(), values))
            .collect::<BTreeMap<_, _>>();
        let tags_json = serde_json::to_string(&tag_map)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        let read_pubkeys = read_pubkeys.map(<[String]>::to_vec);
        let search = filter.search.as_deref().map(search_terms);
        let params: &[&(dyn ToSql + Sync)] = &[
            &ids,
            &authors,
            &kinds,
            &since,
            &until,
            &tags_json,
            &now,
            &limit,
            &through,
            &read_pubkeys,
            &search,
        ];
        let rows = if let Some(cancel) = &mut cancel {
            if *cancel.borrow() {
                return Err(StoreError::QueryCancelled);
            }
            let query = self.client.query(&self.statements.query_filter, params);
            tokio::pin!(query);
            tokio::select! {
                result = &mut query => result?,
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow() {
                        self.client.cancel_token().cancel_query(NoTls).await?;
                        return Err(StoreError::QueryCancelled);
                    }
                    query.await?
                }
            }
        } else {
            self.client
                .query(&self.statements.query_filter, params)
                .await?
        };
        rows.into_iter().map(decode_event_row).collect()
    }

    pub async fn count_filters(
        &self,
        filters: &[Filter],
        now: u64,
        max_count: usize,
        read_pubkeys: &[String],
    ) -> Result<Option<usize>, StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        let scan_limit = pg_limit(max_count.saturating_add(1))?;
        let read_pubkeys = (!read_pubkeys.is_empty()).then(|| read_pubkeys.to_vec());
        let mut ids = BTreeSet::new();
        for filter in filters {
            filter.validate()?;
            let filter_ids = filter.ids.clone();
            let authors = filter.authors.clone();
            let kinds = filter.kinds.as_ref().map(|values| {
                values
                    .iter()
                    .map(|value| i32::from(*value))
                    .collect::<Vec<_>>()
            });
            let since = filter
                .since
                .map(|value| pg_i64(value, "since"))
                .transpose()?;
            let until = filter
                .until
                .map(|value| pg_i64(value, "until"))
                .transpose()?;
            let tag_map = filter
                .tags
                .iter()
                .map(|(key, values)| (key.to_string(), values))
                .collect::<BTreeMap<_, _>>();
            let tags_json = serde_json::to_string(&tag_map)
                .map_err(|error| StoreError::Serialization(error.to_string()))?;
            let search = filter.search.as_deref().map(search_terms);
            let params: &[&(dyn ToSql + Sync)] = &[
                &filter_ids,
                &authors,
                &kinds,
                &since,
                &until,
                &tags_json,
                &now,
                &read_pubkeys,
                &search,
                &scan_limit,
            ];
            for row in self
                .client
                .query(&self.statements.query_filter_ids, params)
                .await?
            {
                ids.insert(row.get::<_, String>(0));
                if ids.len() > max_count {
                    return Ok(None);
                }
            }
        }
        Ok(Some(ids.len()))
    }

    pub async fn delete_expired(&self, now: u64) -> Result<u64, StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        Ok(self
            .client
            .execute(&self.statements.delete_expired, &[&now])
            .await?)
    }

    pub async fn media_blob(&self, sha256: &str) -> Result<Option<MediaRecord>, StoreError> {
        self.ensure_current()?;
        self.client
            .query_opt(&self.statements.media_blob, &[&sha256])
            .await?
            .map(decode_media_row)
            .transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_media(
        &mut self,
        authorization_id: &str,
        authorization_pubkey: &str,
        sha256: &str,
        size: u64,
        media_type: &str,
        uploaded_at: u64,
        max_bytes_per_pubkey: u64,
    ) -> Result<MediaUploadOutcome, StoreError> {
        self.ensure_current()?;
        let size = pg_i64(size, "media size")?;
        let uploaded_at = pg_i64(uploaded_at, "media upload")?;
        let max_bytes = pg_i64(max_bytes_per_pubkey, "media quota")?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        transaction
            .query_one(&statements.advisory_lock, &[&format!("media:{sha256}")])
            .await?;
        transaction
            .query_one(
                &statements.advisory_lock,
                &[&format!("media-owner:{authorization_pubkey}")],
            )
            .await?;
        if transaction
            .query_opt(
                &statements.accept_media_auth,
                &[&authorization_id, &authorization_pubkey, &"upload"],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Err(StoreError::Media(
                "authorization event was already used".into(),
            ));
        }
        let already_owned = transaction
            .query_opt(&statements.media_owner, &[&sha256, &authorization_pubkey])
            .await?
            .is_some();
        if !already_owned {
            let owned_bytes = transaction
                .query_one(&statements.media_owner_bytes, &[&authorization_pubkey])
                .await?
                .get::<_, i64>(0);
            if owned_bytes.saturating_add(size) > max_bytes {
                return Err(StoreError::Media("pubkey media quota exceeded".into()));
            }
        }
        let created = transaction
            .query_opt(
                &statements.insert_media_blob,
                &[&sha256, &authorization_id, &size, &media_type, &uploaded_at],
            )
            .await?
            .is_some();
        transaction
            .execute(
                &statements.insert_media_owner,
                &[&sha256, &authorization_pubkey],
            )
            .await?;
        let record = decode_media_row(
            transaction
                .query_one(&statements.media_blob_any, &[&sha256])
                .await?,
        )?;
        transaction.commit().await?;
        Ok(MediaUploadOutcome {
            record,
            created,
            owned_before: already_owned,
        })
    }

    /// Release a registration whose body never arrived whole. The uploader's
    /// new ownership goes; a blob row that is still unpublished and has no
    /// owner left goes with it. Ownership the uploader held before the
    /// registration, and any published blob, stay.
    pub async fn abandon_media(
        &mut self,
        authorization_pubkey: &str,
        sha256: &str,
        owned_before: bool,
    ) -> Result<(), StoreError> {
        self.ensure_current()?;
        if owned_before {
            return Ok(());
        }
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        transaction
            .query_one(&statements.advisory_lock, &[&format!("media:{sha256}")])
            .await?;
        transaction
            .query_one(
                &statements.advisory_lock,
                &[&format!("media-owner:{authorization_pubkey}")],
            )
            .await?;
        transaction
            .query_opt(
                &statements.delete_media_owner,
                &[&sha256, &authorization_pubkey],
            )
            .await?;
        let has_owner = transaction
            .query_one(&statements.media_has_owner, &[&sha256])
            .await?
            .get::<_, bool>(0);
        if !has_owner {
            transaction
                .query_opt(&statements.delete_unpublished_media_blob, &[&sha256])
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Drop every upload registration that never published and was made
    /// at or before `cutoff`, with the ownership it reserved. Returns the
    /// dropped records so the caller can remove any bytes left on disk.
    pub async fn release_stale_media_reservations(
        &self,
        cutoff: u64,
    ) -> Result<Vec<MediaRecord>, StoreError> {
        self.ensure_current()?;
        let cutoff = pg_i64(cutoff, "media reservation cutoff")?;
        self.client
            .query(&self.statements.delete_stale_media_reservations, &[&cutoff])
            .await?
            .into_iter()
            .map(decode_media_row)
            .collect()
    }

    pub async fn finalize_media(&self, sha256: &str) -> Result<(), StoreError> {
        self.ensure_current()?;
        if self
            .client
            .query_opt(&self.statements.finalize_media_blob, &[&sha256])
            .await?
            .is_none()
        {
            return Err(StoreError::Media(
                "media registration disappeared before finalization".into(),
            ));
        }
        Ok(())
    }

    pub async fn delete_media(
        &mut self,
        authorization_id: &str,
        authorization_pubkey: &str,
        sha256: &str,
    ) -> Result<MediaDeleteOutcome, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        transaction
            .query_one(&statements.advisory_lock, &[&format!("media:{sha256}")])
            .await?;
        transaction
            .query_one(
                &statements.advisory_lock,
                &[&format!("media-owner:{authorization_pubkey}")],
            )
            .await?;
        if transaction
            .query_opt(
                &statements.accept_media_auth,
                &[&authorization_id, &authorization_pubkey, &"delete"],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Err(StoreError::Media(
                "authorization event was already used".into(),
            ));
        }
        if transaction
            .query_opt(
                &statements.delete_media_owner,
                &[&sha256, &authorization_pubkey],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Ok(MediaDeleteOutcome::NotOwned);
        }
        let has_owner = transaction
            .query_one(&statements.media_has_owner, &[&sha256])
            .await?
            .get::<_, bool>(0);
        let record = decode_media_row(
            transaction
                .query_one(&statements.media_blob_any, &[&sha256])
                .await?,
        )?;
        let outcome = if has_owner {
            MediaDeleteOutcome::OwnerRemoved
        } else {
            transaction
                .query_opt(&statements.delete_media_blob, &[&sha256])
                .await?;
            MediaDeleteOutcome::BlobRemoved(record)
        };
        transaction.commit().await?;
        Ok(outcome)
    }

    pub async fn manage(
        &mut self,
        authorization_id: &str,
        authorization_pubkey: &str,
        request: ManagementRequest,
        now: u64,
        relay_signer: Option<&RelaySigner>,
    ) -> Result<Value, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let transaction = self.client.transaction().await?;
        if transaction
            .query_opt(
                &statements.accept_management,
                &[&authorization_id, &authorization_pubkey],
            )
            .await?
            .is_none()
        {
            transaction.commit().await?;
            return Err(StoreError::Management(
                "authorization event was already used".into(),
            ));
        }

        let result = match request {
            ManagementRequest::BanPubkey { pubkey, reason } => {
                transaction
                    .execute(&statements.ban_pubkey, &[&pubkey, &reason])
                    .await?;
                json!(true)
            }
            ManagementRequest::UnbanPubkey { pubkey } => {
                transaction
                    .execute(&statements.unban_pubkey, &[&pubkey])
                    .await?;
                json!(true)
            }
            ManagementRequest::ListBannedPubkeys => json!(
                transaction
                    .query(&statements.list_banned_pubkeys, &[])
                    .await?
                    .into_iter()
                    .map(|row| json!({
                        "pubkey": row.get::<_, String>(0),
                        "reason": row.get::<_, String>(1),
                    }))
                    .collect::<Vec<_>>()
            ),
            ManagementRequest::AllowPubkey { pubkey, reason } => {
                transaction
                    .execute(&statements.allow_pubkey_mutation, &[&pubkey, &reason])
                    .await?;
                json!(true)
            }
            ManagementRequest::UnallowPubkey { pubkey } => {
                transaction
                    .execute(&statements.unallow_pubkey, &[&pubkey])
                    .await?;
                json!(true)
            }
            ManagementRequest::ListAllowedPubkeys => json!(
                transaction
                    .query(&statements.list_allowed_pubkeys, &[])
                    .await?
                    .into_iter()
                    .map(|row| json!({
                        "pubkey": row.get::<_, String>(0),
                        "reason": row.get::<_, String>(1),
                    }))
                    .collect::<Vec<_>>()
            ),
            ManagementRequest::AllowKind { kind } => {
                let kind = i32::from(kind);
                transaction
                    .execute(&statements.allow_kind_mutation, &[&kind])
                    .await?;
                json!(true)
            }
            ManagementRequest::DisallowKind { kind } => {
                let kind = i32::from(kind);
                transaction
                    .execute(&statements.disallow_kind, &[&kind])
                    .await?;
                json!(true)
            }
            ManagementRequest::ListAllowedKinds => json!(
                transaction
                    .query(&statements.list_allowed_kinds, &[])
                    .await?
                    .into_iter()
                    .map(|row| row.get::<_, i32>(0))
                    .collect::<Vec<_>>()
            ),
            ManagementRequest::CreateGroup {
                id,
                metadata,
                admin_pubkey,
            } => {
                let signer = relay_signer.ok_or_else(|| {
                    StoreError::Management("relay signing key is not configured".into())
                })?;
                let kinds = metadata.supported_kinds.as_ref().map(|kinds| {
                    kinds
                        .iter()
                        .map(|kind| i32::from(*kind))
                        .collect::<Vec<_>>()
                });
                transaction
                    .query_one(&statements.advisory_lock, &[&format!("group:{id}")])
                    .await?;
                let inserted = transaction
                    .execute(
                        &statements.create_group,
                        &[
                            &id,
                            &metadata.name,
                            &metadata.about,
                            &metadata.picture,
                            &metadata.closed,
                            &kinds,
                        ],
                    )
                    .await?;
                if inserted == 0 {
                    return Err(StoreError::Management("group already exists".into()));
                }
                let roles = vec!["admin".to_owned()];
                transaction
                    .execute(&statements.put_group_member, &[&id, &admin_pubkey, &roles])
                    .await?;
                transaction.query_one(&statements.ingest_lock, &[]).await?;
                generate_group_metadata(&transaction, &statements, signer, &id, now).await?;
                json!(true)
            }
            ManagementRequest::DeleteGroup { id } => {
                transaction
                    .execute(&statements.delete_group, &[&id])
                    .await?;
                json!(true)
            }
            ManagementRequest::ListGroups => json!(
                transaction
                    .query(&statements.list_groups, &[])
                    .await?
                    .into_iter()
                    .map(|row| json!({
                        "id": row.get::<_, String>(0),
                        "name": row.get::<_, String>(1),
                        "about": row.get::<_, String>(2),
                        "picture": row.get::<_, String>(3),
                        "closed": row.get::<_, bool>(4),
                        "supported_kinds": row.get::<_, Option<Vec<i32>>>(5),
                    }))
                    .collect::<Vec<_>>()
            ),
            ManagementRequest::PutGroupUser { id, pubkey, roles } => {
                let signer = relay_signer.ok_or_else(|| {
                    StoreError::Management("relay signing key is not configured".into())
                })?;
                if load_group(&transaction, &statements, &id).await?.is_none() {
                    return Err(StoreError::Management("group does not exist".into()));
                }
                transaction
                    .execute(&statements.put_group_member, &[&id, &pubkey, &roles])
                    .await?;
                transaction.query_one(&statements.ingest_lock, &[]).await?;
                generate_group_metadata(&transaction, &statements, signer, &id, now).await?;
                json!(true)
            }
            ManagementRequest::RemoveGroupUser { id, pubkey } => {
                let signer = relay_signer.ok_or_else(|| {
                    StoreError::Management("relay signing key is not configured".into())
                })?;
                if load_group(&transaction, &statements, &id).await?.is_none() {
                    return Err(StoreError::Management("group does not exist".into()));
                }
                transaction
                    .execute(&statements.remove_group_member, &[&id, &pubkey])
                    .await?;
                transaction.query_one(&statements.ingest_lock, &[]).await?;
                generate_group_metadata(&transaction, &statements, signer, &id, now).await?;
                json!(true)
            }
        };
        transaction.commit().await?;
        Ok(result)
    }
}

fn record_import_rejection(report: &mut LegacyImportReport, reason: impl Into<String>) {
    *report.rejection_reasons.entry(reason.into()).or_default() += 1;
}

fn admission_rejection_code(rejection: &AdmissionRejection) -> &'static str {
    rejection.code()
}

fn domain_error_code(error: &DomainError) -> String {
    match error {
        DomainError::EmptyTag => "empty_tag".to_owned(),
        DomainError::InvalidHex { field, .. } => format!("invalid_hex:{field}"),
        DomainError::InvalidPublicKey => "invalid_pubkey".to_owned(),
        DomainError::EventIdMismatch { .. } => "event_id_mismatch".to_owned(),
        DomainError::InvalidSignature => "invalid_signature".to_owned(),
        DomainError::ExpiredEvent { .. } => "expired".to_owned(),
        DomainError::FutureTimestamp { .. } => "timestamp_future".to_owned(),
        DomainError::InvalidEvent(reason) => format!("invalid_event:{reason}"),
        DomainError::InvalidFilter(_) => "invalid_filter".to_owned(),
        DomainError::InvalidReplacementAddress(_) => "invalid_replacement_address".to_owned(),
        DomainError::ReplacementAddressMismatch => "replacement_address_mismatch".to_owned(),
        DomainError::NotReplaceable => "not_replaceable".to_owned(),
        DomainError::NotDeletionRequest => "not_deletion".to_owned(),
        DomainError::Serialization(_) => "domain_serialization".to_owned(),
    }
}

fn decode_nostr_effect_event(row: &Row) -> Result<Event, StoreError> {
    let created_at = row.get::<_, i64>(2);
    let created_at = u64::try_from(created_at)
        .map_err(|_| StoreError::CorruptRow("legacy event has negative created_at".to_owned()))?;
    let kind = row.get::<_, i32>(3);
    let kind = u16::try_from(kind)
        .map_err(|_| StoreError::CorruptRow("legacy event kind is out of range".to_owned()))?;
    let tags_json = row.get::<_, String>(4);
    let tags = match serde_json::from_str::<Vec<Tag>>(&tags_json) {
        Ok(tags) => tags,
        Err(_) => {
            let encoded = serde_json::from_str::<String>(&tags_json).map_err(|error| {
                StoreError::CorruptRow(format!("legacy event tags are not an array: {error}"))
            })?;
            serde_json::from_str::<Vec<Tag>>(&encoded).map_err(|error| {
                StoreError::CorruptRow(format!("legacy event tags are not an array: {error}"))
            })?
        }
    };
    Ok(Event {
        id: row.get(0),
        pubkey: row.get(1),
        created_at,
        kind,
        tags,
        content: row.get(5),
        sig: row.get(6),
    })
}

fn decode_media_row(row: Row) -> Result<MediaRecord, StoreError> {
    let size = row.get::<_, i64>(1);
    let uploaded_at = row.get::<_, i64>(3);
    Ok(MediaRecord {
        sha256: row.get(0),
        size: u64::try_from(size)
            .map_err(|_| StoreError::CorruptRow("negative media size".into()))?,
        media_type: row.get(2),
        uploaded_at: u64::try_from(uploaded_at)
            .map_err(|_| StoreError::CorruptRow("negative media upload timestamp".into()))?,
        storage_key: row.get(4),
    })
}

impl Drop for Store {
    fn drop(&mut self) {
        self.connection_task.abort();
    }
}

/// Dedicated bounded durable and ephemeral notification connection. A
/// malformed payload, incomplete protocol state, driver failure, or full
/// local queue marks it not current so the gateway can fail closed.
pub struct NotificationListener {
    receiver: mpsc::Receiver<StoreNotification>,
    connection_current: Arc<AtomicBool>,
    _client: Client,
    connection_task: JoinHandle<()>,
}

impl NotificationListener {
    pub async fn connect(config: &str, capacity: usize) -> Result<Self, StoreError> {
        let (client, mut connection) = tokio_postgres::connect(config, NoTls).await?;
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        let connection_current = Arc::new(AtomicBool::new(true));
        let task_current = Arc::clone(&connection_current);
        let connection_task = tokio::spawn(async move {
            let mut assemblies = HashMap::new();
            loop {
                match poll_fn(|context| connection.poll_message(context)).await {
                    Some(Ok(AsyncMessage::Notification(notification)))
                        if notification.channel() == EVENT_CHANNEL =>
                    {
                        let Ok(ingest_seq) = notification.payload().parse::<i64>() else {
                            break;
                        };
                        if ingest_seq <= 0
                            || sender
                                .try_send(StoreNotification::Stored(ingest_seq))
                                .is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(AsyncMessage::Notification(notification)))
                        if notification.channel() == EPHEMERAL_CHANNEL =>
                    {
                        match accept_ephemeral_chunk(notification.payload(), &mut assemblies) {
                            Ok(Some(event)) => {
                                if sender
                                    .try_send(StoreNotification::Ephemeral(event))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Ok(None) => {}
                            Err(()) => break,
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
            task_current.store(false, Ordering::Release);
        });

        let listen = client.prepare(LISTEN_EVENT_SQL).await?;
        client.execute(&listen, &[]).await?;
        let listen_ephemeral = client.prepare(LISTEN_EPHEMERAL_SQL).await?;
        client.execute(&listen_ephemeral, &[]).await?;
        Ok(Self {
            receiver,
            connection_current,
            _client: client,
            connection_task,
        })
    }

    pub fn is_current(&self) -> bool {
        self.connection_current.load(Ordering::Acquire)
    }

    pub async fn recv(&mut self) -> Option<i64> {
        loop {
            match self.receiver.recv().await? {
                StoreNotification::Stored(ingest_seq) => return Some(ingest_seq),
                StoreNotification::Ephemeral(_) => {}
            }
        }
    }

    pub async fn recv_notification(&mut self) -> Option<StoreNotification> {
        self.receiver.recv().await
    }
}

impl Drop for NotificationListener {
    fn drop(&mut self) {
        self.connection_task.abort();
    }
}

async fn open_client(
    config: &str,
) -> Result<(Client, Arc<AtomicBool>, JoinHandle<()>), StoreError> {
    let (client, connection) = tokio_postgres::connect(config, NoTls).await?;
    let connection_current = Arc::new(AtomicBool::new(true));
    let task_current = Arc::clone(&connection_current);
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
        task_current.store(false, Ordering::Release);
    });
    Ok((client, connection_current, connection_task))
}

fn admission_lock_keys(
    event: &Event,
    replacement: Option<&crate::domain::ReplacementAddress>,
    deletion: Option<&DeletionRequest>,
) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    keys.insert(format!("event:{}:{}", event.pubkey, event.id));
    if let Some(address) = replacement {
        keys.insert(format!("address:{address}"));
    }
    if let Some(request) = deletion {
        keys.extend(
            request
                .event_ids
                .iter()
                .map(|id| format!("event:{}:{id}", request.author)),
        );
        keys.extend(
            request
                .addresses
                .iter()
                .map(|address| format!("address:{address}")),
        );
    }
    if let Some(group_id) = group_scope(event) {
        keys.insert(format!("group:{group_id}"));
    }
    keys
}

fn group_scope(event: &Event) -> Option<&str> {
    (!BLOCK_GLOBAL_ONLY_KINDS.contains(&event.kind))
        .then(|| event.group_id())
        .flatten()
}

struct GroupRow {
    name: String,
    about: String,
    picture: String,
    closed: bool,
    supported_kinds: Option<Vec<u16>>,
    pins: Vec<Tag>,
}

async fn load_group(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    group_id: &str,
) -> Result<Option<GroupRow>, StoreError> {
    transaction
        .query_opt(&statements.group, &[&group_id])
        .await?
        .map(|row| {
            let supported_kinds = row
                .get::<_, Option<Vec<i32>>>(4)
                .map(|kinds| {
                    kinds
                        .into_iter()
                        .map(|kind| {
                            u16::try_from(kind).map_err(|_| {
                                StoreError::CorruptRow(
                                    "group has an out-of-range supported kind".into(),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            let pins = serde_json::from_str::<Vec<Tag>>(&row.get::<_, String>(5))
                .map_err(|error| StoreError::CorruptRow(format!("invalid group pins: {error}")))?;
            Ok(GroupRow {
                name: row.get(0),
                about: row.get(1),
                picture: row.get(2),
                closed: row.get(3),
                supported_kinds,
                pins,
            })
        })
        .transpose()
}

async fn group_member(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    group_id: &str,
    pubkey: &str,
) -> Result<Option<Vec<String>>, StoreError> {
    Ok(transaction
        .query_opt(&statements.group_member, &[&group_id, &pubkey])
        .await?
        .map(|row| row.get(0)))
}

async fn valid_group_invite(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    group_id: &str,
    code: Option<&str>,
) -> Result<bool, StoreError> {
    let Some(code) = code else {
        return Ok(false);
    };
    Ok(transaction
        .query_opt(&statements.group_invite, &[&group_id, &code])
        .await?
        .is_some())
}

async fn group_previous_references_are_current(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    event: &Event,
    group_id: &str,
    now: u64,
) -> Result<bool, StoreError> {
    let references = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("previous"))
        .flat_map(|tag| tag.as_slice().iter().skip(1))
        .collect::<Vec<_>>();
    if references.is_empty() {
        return Ok(true);
    }
    let now = pg_i64(now, "now")?;
    let prefixes = transaction
        .query(
            &statements.group_recent_ids,
            &[&group_id, &event.pubkey, &now],
        )
        .await?
        .into_iter()
        .map(|row| row.get::<_, String>(0)[..8].to_owned())
        .collect::<BTreeSet<_>>();
    Ok(references
        .into_iter()
        .all(|reference| prefixes.contains(reference)))
}

async fn apply_group_action(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    group_id: &str,
    author: &str,
    action: &GroupAction,
) -> Result<(), StoreError> {
    match action {
        GroupAction::PutUser { pubkey, roles } => {
            transaction
                .execute(&statements.put_group_member, &[&group_id, &pubkey, &roles])
                .await?;
        }
        GroupAction::RemoveUser { pubkey } => {
            transaction
                .execute(&statements.remove_group_member, &[&group_id, &pubkey])
                .await?;
        }
        GroupAction::EditMetadata(metadata) => {
            let supported_kinds = metadata.supported_kinds.as_ref().map(|kinds| {
                kinds
                    .iter()
                    .map(|kind| i32::from(*kind))
                    .collect::<Vec<_>>()
            });
            transaction
                .execute(
                    &statements.update_group_metadata,
                    &[
                        &group_id,
                        &metadata.name,
                        &metadata.about,
                        &metadata.picture,
                        &metadata.closed,
                        &supported_kinds,
                    ],
                )
                .await?;
        }
        GroupAction::DeleteEvent { event_id } => {
            transaction
                .execute(&statements.delete_group_event, &[&event_id, &group_id])
                .await?;
        }
        GroupAction::CreateGroup => {
            let empty = String::new();
            let closed = false;
            let supported: Option<Vec<i32>> = None;
            transaction
                .execute(
                    &statements.create_group,
                    &[&group_id, &empty, &empty, &empty, &closed, &supported],
                )
                .await?;
            let roles = vec!["admin".to_owned()];
            transaction
                .execute(&statements.put_group_member, &[&group_id, &author, &roles])
                .await?;
        }
        GroupAction::DeleteGroup => {
            transaction
                .execute(&statements.delete_group, &[&group_id])
                .await?;
        }
        GroupAction::CreateInvite { code } => {
            transaction
                .execute(&statements.create_group_invite, &[&group_id, &code])
                .await?;
        }
        GroupAction::UpdatePins { tags } => {
            let pins = serde_json::to_string(tags)
                .map_err(|error| StoreError::Serialization(error.to_string()))?;
            transaction
                .execute(&statements.update_group_pins, &[&group_id, &pins])
                .await?;
        }
        GroupAction::Join { .. } => {
            let roles: Vec<String> = Vec::new();
            transaction
                .execute(&statements.put_group_member, &[&group_id, &author, &roles])
                .await?;
        }
        GroupAction::Leave => {
            transaction
                .execute(&statements.remove_group_member, &[&group_id, &author])
                .await?;
        }
    }
    Ok(())
}

async fn generate_group_metadata(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    signer: &RelaySigner,
    group_id: &str,
    now: u64,
) -> Result<(), StoreError> {
    let group = load_group(transaction, statements, group_id)
        .await?
        .ok_or_else(|| StoreError::CorruptRow("group disappeared during metadata update".into()))?;
    let members = transaction
        .query(&statements.group_members, &[&group_id])
        .await?
        .into_iter()
        .map(|row| (row.get::<_, String>(0), row.get::<_, Vec<String>>(1)))
        .collect::<Vec<_>>();

    let mut metadata = vec![Tag::new(vec!["d".into(), group_id.into()])];
    for (name, value) in [
        ("name", group.name.as_str()),
        ("about", group.about.as_str()),
        ("picture", group.picture.as_str()),
    ] {
        if !value.is_empty() {
            metadata.push(Tag::new(vec![name.into(), value.into()]));
        }
    }
    metadata.push(Tag::new(vec!["restricted".into()]));
    if group.closed {
        metadata.push(Tag::new(vec!["closed".into()]));
    }
    if let Some(kinds) = &group.supported_kinds {
        let mut tag = vec!["supported_kinds".into()];
        tag.extend(kinds.iter().map(u16::to_string));
        metadata.push(Tag::new(tag));
    }

    let mut admins = vec![Tag::new(vec!["d".into(), group_id.into()])];
    for (pubkey, roles) in &members {
        if !roles.is_empty() {
            let mut tag = vec!["p".into(), pubkey.clone()];
            tag.extend(roles.iter().cloned());
            admins.push(Tag::new(tag));
        }
    }
    let mut member_tags = vec![Tag::new(vec!["d".into(), group_id.into()])];
    member_tags.extend(
        members
            .iter()
            .map(|(pubkey, _)| Tag::new(vec!["p".into(), pubkey.clone()])),
    );
    let roles = vec![
        Tag::new(vec!["d".into(), group_id.into()]),
        Tag::new(vec![
            "role".into(),
            "admin".into(),
            "full group moderation".into(),
        ]),
    ];
    let participants = vec![Tag::new(vec!["d".into(), group_id.into()])];
    let mut pins = vec![Tag::new(vec!["d".into(), group_id.into()])];
    pins.extend(group.pins);

    for (kind, tags, content) in [
        (39_000, metadata, String::new()),
        (39_001, admins, "group administrators".into()),
        (39_002, member_tags, "group members".into()),
        (39_003, roles, "relay-supported group roles".into()),
        (39_004, participants, String::new()),
        (39_005, pins, String::new()),
    ] {
        insert_group_metadata_event(
            transaction,
            statements,
            signer,
            group_id,
            now,
            kind,
            tags,
            content,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_group_metadata_event(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    signer: &RelaySigner,
    group_id: &str,
    now: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: String,
) -> Result<(), StoreError> {
    let kind_i32 = i32::from(kind);
    let head_params: &[&(dyn ToSql + Sync)] = &[&kind_i32, &signer.pubkey(), &group_id];
    let current = transaction.query_opt(&statements.head, head_params).await?;
    let created_at = current
        .as_ref()
        .map(|row| row.get::<_, i64>(1))
        .map(|timestamp| {
            u64::try_from(timestamp)
                .map_err(|_| StoreError::CorruptRow("negative metadata timestamp".into()))
        })
        .transpose()?
        .map_or(now, |timestamp| now.max(timestamp.saturating_add(1)));
    let event = signer.sign(created_at, kind, tags, content);
    let created_at_i64 = pg_i64(created_at, "group metadata timestamp")?;
    let tags_json = serde_json::to_string(&event.tags)
        .map_err(|error| StoreError::Serialization(error.to_string()))?;
    let identifier = Some(group_id);
    let expires_at: Option<i64> = None;
    let insert_params: &[&(dyn ToSql + Sync)] = &[
        &event.id,
        &event.pubkey,
        &created_at_i64,
        &kind_i32,
        &tags_json,
        &event.content,
        &event.sig,
        &identifier,
        &expires_at,
    ];
    let Some(row) = transaction
        .query_opt(&statements.insert_event, insert_params)
        .await?
    else {
        return Ok(());
    };
    let ingest_seq = row.get::<_, i64>(0);
    for (tag_name, tag_value) in event.indexed_tags() {
        let tag_name = tag_name.to_string();
        transaction
            .execute(
                &statements.insert_tag,
                &[&event.id, &tag_name, &tag_value, &created_at_i64],
            )
            .await?;
    }
    transaction
        .execute(
            &statements.upsert_head,
            &[
                &kind_i32,
                &event.pubkey,
                &group_id,
                &event.id,
                &created_at_i64,
            ],
        )
        .await?;
    if let Some(row) = current {
        let old_id = row.get::<_, String>(0);
        transaction
            .execute(&statements.delete_event, &[&old_id])
            .await?;
    }
    transaction
        .query_one(&statements.notify, &[&ingest_seq.to_string()])
        .await?;
    Ok(())
}

async fn insert_internal_regular_event(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    event: &Event,
) -> Result<(), StoreError> {
    let created_at = pg_i64(event.created_at, "internal event timestamp")?;
    let kind = i32::from(event.kind);
    let tags_json = serde_json::to_string(&event.tags)
        .map_err(|error| StoreError::Serialization(error.to_string()))?;
    let identifier: Option<&str> = None;
    let expires_at: Option<i64> = None;
    let insert_params: &[&(dyn ToSql + Sync)] = &[
        &event.id,
        &event.pubkey,
        &created_at,
        &kind,
        &tags_json,
        &event.content,
        &event.sig,
        &identifier,
        &expires_at,
    ];
    let Some(row) = transaction
        .query_opt(&statements.insert_event, insert_params)
        .await?
    else {
        return Ok(());
    };
    let ingest_seq = row.get::<_, i64>(0);
    for (tag_name, tag_value) in event.indexed_tags() {
        let tag_name = tag_name.to_string();
        transaction
            .execute(
                &statements.insert_tag,
                &[&event.id, &tag_name, &tag_value, &created_at],
            )
            .await?;
    }
    transaction
        .query_one(&statements.notify, &[&ingest_seq.to_string()])
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_internal_replaceable_event(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    signer: &RelaySigner,
    kind: u16,
    identifier: &str,
    now: u64,
    tags: Vec<Tag>,
    content: String,
) -> Result<(), StoreError> {
    let kind_i32 = i32::from(kind);
    let current = transaction
        .query_opt(
            &statements.head,
            &[&kind_i32, &signer.pubkey(), &identifier],
        )
        .await?;
    let created_at = current
        .as_ref()
        .map(|row| row.get::<_, i64>(1))
        .map(|value| {
            u64::try_from(value)
                .map_err(|_| StoreError::CorruptRow("negative internal event timestamp".into()))
        })
        .transpose()?
        .map_or(now, |previous| now.max(previous.saturating_add(1)));
    let event = signer.sign(created_at, kind, tags, content);
    let created_at = pg_i64(created_at, "internal replaceable event timestamp")?;
    let tags_json = serde_json::to_string(&event.tags)
        .map_err(|error| StoreError::Serialization(error.to_string()))?;
    let identifier_param = Some(identifier);
    let expires_at: Option<i64> = None;
    let Some(row) = transaction
        .query_opt(
            &statements.insert_event,
            &[
                &event.id,
                &event.pubkey,
                &created_at,
                &kind_i32,
                &tags_json,
                &event.content,
                &event.sig,
                &identifier_param,
                &expires_at,
            ],
        )
        .await?
    else {
        return Ok(());
    };
    let ingest_seq = row.get::<_, i64>(0);
    for (tag_name, tag_value) in event.indexed_tags() {
        let tag_name = tag_name.to_string();
        transaction
            .execute(
                &statements.insert_tag,
                &[&event.id, &tag_name, &tag_value, &created_at],
            )
            .await?;
    }
    transaction
        .execute(
            &statements.upsert_head,
            &[
                &kind_i32,
                &event.pubkey,
                &identifier,
                &event.id,
                &created_at,
            ],
        )
        .await?;
    if let Some(current) = current {
        let old_id = current.get::<_, String>(0);
        transaction
            .execute(&statements.delete_event, &[&old_id])
            .await?;
    }
    transaction
        .query_one(&statements.notify, &[&ingest_seq.to_string()])
        .await?;
    Ok(())
}

async fn notify_ephemeral(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    event: &Event,
) -> Result<(), StoreError> {
    let bytes =
        serde_json::to_vec(event).map_err(|error| StoreError::Serialization(error.to_string()))?;
    if bytes.len() > EPHEMERAL_MAX_BYTES {
        return Err(StoreError::EphemeralTooLarge(bytes.len()));
    }
    let total = bytes.len().div_ceil(EPHEMERAL_CHUNK_BYTES);
    for (index, chunk) in bytes.chunks(EPHEMERAL_CHUNK_BYTES).enumerate() {
        let payload = format!("{}:{index}:{total}:{}", event.id, encode_hex(chunk));
        transaction
            .query_one(&statements.notify_ephemeral, &[&payload])
            .await?;
    }
    Ok(())
}

struct EphemeralAssembly {
    chunks: Vec<Option<Vec<u8>>>,
    bytes: usize,
}

fn accept_ephemeral_chunk(
    payload: &str,
    assemblies: &mut HashMap<String, EphemeralAssembly>,
) -> Result<Option<Event>, ()> {
    let mut parts = payload.splitn(4, ':');
    let event_id = parts.next().ok_or(())?;
    let index = parts.next().ok_or(())?.parse::<usize>().map_err(|_| ())?;
    let total = parts.next().ok_or(())?.parse::<usize>().map_err(|_| ())?;
    let encoded = parts.next().ok_or(())?;
    let max_chunks = EPHEMERAL_MAX_BYTES.div_ceil(EPHEMERAL_CHUNK_BYTES);
    if event_id.len() != 64
        || !event_id
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        || total == 0
        || total > max_chunks
        || index >= total
        || encoded.len() > EPHEMERAL_CHUNK_BYTES * 2
    {
        return Err(());
    }
    let chunk = decode_hex(encoded)?;
    if assemblies.len() >= EPHEMERAL_MAX_ASSEMBLIES && !assemblies.contains_key(event_id) {
        return Err(());
    }
    let assembly = assemblies
        .entry(event_id.to_owned())
        .or_insert_with(|| EphemeralAssembly {
            chunks: vec![None; total],
            bytes: 0,
        });
    if assembly.chunks.len() != total || assembly.chunks[index].is_some() {
        return Err(());
    }
    assembly.bytes = assembly.bytes.checked_add(chunk.len()).ok_or(())?;
    if assembly.bytes > EPHEMERAL_MAX_BYTES {
        return Err(());
    }
    assembly.chunks[index] = Some(chunk);
    if assembly.chunks.iter().any(Option::is_none) {
        return Ok(None);
    }

    let assembly = assemblies.remove(event_id).ok_or(())?;
    let mut bytes = Vec::with_capacity(assembly.bytes);
    for chunk in assembly.chunks {
        bytes.extend(chunk.ok_or(())?);
    }
    let event = serde_json::from_slice::<Event>(&bytes).map_err(|_| ())?;
    event.validate_structure().map_err(|_| ())?;
    event.validate_crypto().map_err(|_| ())?;
    if event.id != event_id || event.class() != EventClass::Ephemeral || event.kind == 22_242 {
        return Err(());
    }
    Ok(Some(event))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(2) {
        return Err(());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = decode_nibble(pair[0])?;
            let low = decode_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn decode_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    }
}

async fn apply_deletion(
    transaction: &tokio_postgres::Transaction<'_>,
    statements: &Statements,
    request: &DeletionRequest,
) -> Result<(), StoreError> {
    for tombstone in request.tombstones() {
        match tombstone {
            DeletionTombstone::Event {
                event_id,
                author,
                request_id,
            } => {
                let params: &[&(dyn ToSql + Sync)] = &[&event_id, &author, &request_id];
                transaction
                    .execute(&statements.insert_event_tombstone, params)
                    .await?;
                transaction
                    .execute(&statements.delete_event_target, &[&event_id, &author])
                    .await?;
            }
            DeletionTombstone::Address {
                address,
                through,
                request_id,
            } => {
                let kind = i32::from(address.kind);
                let through = pg_i64(through, "deletion timestamp")?;
                let params: &[&(dyn ToSql + Sync)] = &[
                    &kind,
                    &address.pubkey,
                    &address.identifier,
                    &through,
                    &request_id,
                ];
                transaction
                    .execute(&statements.insert_address_tombstone, params)
                    .await?;
                let delete_params: &[&(dyn ToSql + Sync)] =
                    &[&kind, &address.pubkey, &address.identifier, &through];
                transaction
                    .execute(&statements.delete_address_target, delete_params)
                    .await?;
            }
        }
    }
    Ok(())
}

fn decode_event_row(row: Row) -> Result<StoredEvent, StoreError> {
    let created_at = row.get::<_, i64>(2);
    let created_at = u64::try_from(created_at)
        .map_err(|_| StoreError::CorruptRow("negative created_at".to_owned()))?;
    let kind = row.get::<_, i32>(3);
    let kind = u16::try_from(kind)
        .map_err(|_| StoreError::CorruptRow("kind outside NIP-01 range".to_owned()))?;
    let tags_json = row.get::<_, String>(4);
    let tags = serde_json::from_str(&tags_json)
        .map_err(|error| StoreError::CorruptRow(format!("invalid tags JSON: {error}")))?;
    let event = Event {
        id: row.get(0),
        pubkey: row.get(1),
        created_at,
        kind,
        tags,
        content: row.get(5),
        sig: row.get(6),
    };
    event
        .validate_structure()
        .map_err(|error| StoreError::CorruptRow(error.to_string()))?;
    event
        .validate_crypto()
        .map_err(|error| StoreError::CorruptRow(error.to_string()))?;
    Ok(StoredEvent {
        event,
        ingest_seq: row.get(7),
    })
}

struct AdmissionPolicy {
    closed_membership: bool,
    max_content_bytes: usize,
    max_tags: usize,
    max_future_seconds: u64,
    max_past_seconds: u64,
}

impl AdmissionPolicy {
    fn from_row(row: &Row) -> Result<Self, StoreError> {
        let max_content_bytes = row.get::<_, i64>(1);
        let max_tags = row.get::<_, i32>(2);
        let max_future_seconds = row.get::<_, i64>(3);
        let max_past_seconds = row.get::<_, i64>(4);
        Ok(Self {
            closed_membership: row.get(0),
            max_content_bytes: usize::try_from(max_content_bytes).map_err(|_| {
                StoreError::InvalidPolicy("max_content_bytes is outside usize range".to_owned())
            })?,
            max_tags: usize::try_from(max_tags).map_err(|_| {
                StoreError::InvalidPolicy("max_tags is outside usize range".to_owned())
            })?,
            max_future_seconds: u64::try_from(max_future_seconds).map_err(|_| {
                StoreError::InvalidPolicy("max_future_seconds is negative".to_owned())
            })?,
            max_past_seconds: u64::try_from(max_past_seconds).map_err(|_| {
                StoreError::InvalidPolicy("max_past_seconds is negative".to_owned())
            })?,
        })
    }
}

impl From<AdmissionPolicy> for RelayPolicy {
    fn from(policy: AdmissionPolicy) -> Self {
        Self {
            closed_membership: policy.closed_membership,
            max_content_bytes: policy.max_content_bytes,
            max_tags: policy.max_tags,
            max_future_seconds: policy.max_future_seconds,
            max_past_seconds: policy.max_past_seconds,
        }
    }
}

fn pg_i64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::TimestampOutOfRange { field, value })
}

fn pg_limit(value: usize) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidLimit(value))
}
