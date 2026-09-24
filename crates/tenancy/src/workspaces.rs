//! The workspace identity and membership model: records and rules, no
//! storage.
//!
//! Identity is what outlives the credential. A key rotates, an
//! invitation expires, a token is only ever a digest — what remains is
//! who acted, in which workspace, and under which role. This module is
//! that layer. A *workspace* is where money and keys live; a *user* is
//! who may touch them; a *membership* is the standing record of whether
//! they may.
//!
//! # What attaches where
//!
//! Billing and quota bind to the [`WorkspaceId`], never to a key or a
//! user: [`Workspace::billing`] answers the account every key the
//! workspace owns spends against, so rotating a key or removing a
//! member cannot mint a new budget. A [`UserId`] holds memberships, not
//! credentials — a credential resolves to a user through a key the
//! workspace issued, and the key belongs to the workspace.
//!
//! Removing a user must not free a budget or orphan a history.
//! [`Workspace::revoke`] flips a membership's state and
//! [`Workspace::keys_visible`] stops answering that user's keys, but
//! the membership record, the key records, and the billing attachment
//! all stay — revocation is a state, not a deletion.
//!
//! # What this is not
//!
//! There is no I/O here: no filesystem, no clock, no randomness. Every
//! timestamp arrives as an argument in Unix seconds, every id and
//! digest is the caller's, and persistence — which revision stands,
//! where records live — is the caller's business; `tenancy::accounts`
//! is one such store. And no plaintext secret appears anywhere: an
//! invitation token and a key's secret exist here only as the digests
//! the caller supplies.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

macro_rules! identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// The id as a string — for display and persistence, never
            /// a way to compare across types.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.to_string())
            }
        }
    };
}

identity! {
    /// A user's id — `u_<hex>` by convention, opaque here.
    ///
    /// A user holds memberships, never credentials: the credential that
    /// resolves to a user is a key the workspace issued, and its record
    /// belongs to the workspace, not the user.
    UserId
}

identity! {
    /// A workspace's id — `ws_<hex>` by convention, opaque here.
    ///
    /// The id is what quota and billing bind to, and it never changes:
    /// a rename, a seat change, a key rotation, and every membership
    /// write leave it standing.
    WorkspaceId
}

identity! {
    /// A key's id — the locator half of a credential.
    ///
    /// The secret half never appears in this module: a key record
    /// carries the secret's digest, and rotation replaces the secret
    /// without moving the workspace's billing attachment.
    KeyId
}

identity! {
    /// A billing account's id — the reference a workspace's charges
    /// settle against.
    ///
    /// Money attaches to the workspace, so the reference lives on
    /// [`Workspace::billing`] and nowhere else: no key and no user
    /// carries it.
    BillingAccountId
}

/// A member's role. The declaration order is the rank: member, then
/// admin, then owner — `actor.role > target.role` is the seniority
/// check the removal and suspension rules are built on.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Belongs to the workspace: its own keys and its own usage,
    /// nothing more.
    Member,
    /// Members plus the member list: an admin invites, suspends,
    /// reinstates, and removes members, and reaches every key and all
    /// usage. Not billing, not ownership.
    Admin,
    /// Everything, including the workspace itself: billing, seats, and
    /// the ownership transfer an admin can never make. Exactly one
    /// active member holds it at a time.
    Owner,
}

/// An action a role may cover — the questions the permission matrix
/// answers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    /// Read and move the workspace's billing account.
    ManageBilling,
    /// Hand the workspace to another member.
    TransferOwnership,
    /// Invite, suspend, reinstate, and remove members.
    ManageMembers,
    /// Issue, rotate, and revoke keys for any member.
    ManageKeys,
    /// Issue, rotate, and revoke the member's own keys.
    ManageOwnKeys,
    /// Read usage across the whole workspace.
    ViewUsage,
    /// Read the member's own usage.
    ViewOwnUsage,
}

impl Permission {
    /// Whether holding `self` also answers `asked` — a workspace-wide
    /// grant covers its member-scoped counterpart.
    fn covers(self, asked: Permission) -> bool {
        self == asked
            || matches!(
                (self, asked),
                (Self::ManageKeys, Self::ManageOwnKeys) | (Self::ViewUsage, Self::ViewOwnUsage)
            )
    }
}

impl Role {
    /// The permissions the role grants — this role's row of the
    /// matrix, as a table a caller can read rather than prose.
    ///
    /// - `member` — `manage-own-keys`, `view-own-usage`.
    /// - `admin` — `manage-members`, `manage-keys`, `view-usage`.
    /// - `owner` — every row, including `manage-billing` and
    ///   `transfer-ownership`.
    #[must_use]
    pub fn permissions(self) -> &'static [Permission] {
        match self {
            Self::Member => &[Permission::ManageOwnKeys, Permission::ViewOwnUsage],
            Self::Admin => &[
                Permission::ManageMembers,
                Permission::ManageKeys,
                Permission::ViewUsage,
            ],
            Self::Owner => &[
                Permission::ManageBilling,
                Permission::TransferOwnership,
                Permission::ManageMembers,
                Permission::ManageKeys,
                Permission::ManageOwnKeys,
                Permission::ViewUsage,
                Permission::ViewOwnUsage,
            ],
        }
    }

    /// Whether the role covers the action. An admin's `manage-keys`
    /// answers a `manage-own-keys` question, and `view-usage` answers
    /// `view-own-usage`; a member's own-scoped grants answer nothing
    /// wider.
    #[must_use]
    pub fn permits(self, permission: Permission) -> bool {
        self.permissions()
            .iter()
            .any(|granted| granted.covers(permission))
    }
}

/// Which membership rules a workspace runs — the same record either
/// way.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceKind {
    /// One user's own workspace. It holds exactly its owner forever:
    /// invitations, suspensions, transfers, and seat writes all refuse
    /// outright.
    Personal,
    /// A shared workspace with invitations, roles, and seats.
    Organization,
}

/// Where a membership stands. Removal is a state the record keeps, not
/// a deletion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemberState {
    /// Invited, not yet accepted. Counts for nothing — not seats, not
    /// keys; the invitation is what holds the seat.
    Invited,
    /// Belongs and may act.
    Active,
    /// Parked: counts for nothing until reinstated — no seat, no keys,
    /// no permissions.
    Suspended,
    /// Removed. The record stays so what the member did still has a
    /// name.
    Removed,
}

impl std::fmt::Display for MemberState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Removed => "removed",
        })
    }
}

/// Whether a key stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyStatus {
    /// Answers in `keys_visible`.
    Active,
    /// Rotated out or revoked. The record stays — it is the attribution
    /// for everything the key did.
    Revoked,
}

/// Where an invitation stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvitationState {
    /// Issued, not yet answered.
    Pending,
    /// Consumed. A second acceptance is a replay, not a join.
    Accepted,
    /// Withdrawn, or voided when its member was removed.
    Revoked,
}

impl std::fmt::Display for InvitationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
        })
    }
}

/// The join between a user and a workspace: the record every
/// authorization and mutation check reads.
///
/// The timestamps mark the last time each transition ran — a member
/// who was suspended and reinstated still carries `suspended_at`, and a
/// re-invited member still carries `removed_at`. They are the record's
/// own history, in Unix seconds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Membership {
    /// The user the membership belongs to.
    pub user: UserId,
    /// The workspace the membership belongs to.
    pub workspace: WorkspaceId,
    /// The member's role.
    pub role: Role,
    /// `invited`, `active`, `suspended`, or `removed`.
    pub state: MemberState,
    /// When the membership was granted, as Unix seconds.
    pub granted_at: u64,
    /// When the membership last became active, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<u64>,
    /// When the membership was last suspended, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspended_at: Option<u64>,
    /// When the membership was last removed, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_at: Option<u64>,
}

impl Membership {
    /// Whether the member may act — the state every permission check
    /// reads first.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == MemberState::Active
    }

    /// Whether the member's role covers the action. A membership that
    /// is not active permits nothing.
    #[must_use]
    pub fn permits(&self, permission: Permission) -> bool {
        self.is_active() && self.role.permits(permission)
    }
}

/// A key the workspace owns. The secret is never here — only its
/// digest, the same discipline `tenancy::keys` keeps.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceKey {
    /// The key's id.
    pub id: KeyId,
    /// The member the key was issued to — attribution, so what the key
    /// did keeps a name after the member is gone.
    pub owner: UserId,
    /// SHA-256 of the secret, hex.
    pub secret_digest: String,
    /// `active` or `revoked`.
    pub status: KeyStatus,
    /// The key this one replaced, when it is a rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotated_from: Option<KeyId>,
    /// When the key was issued, as Unix seconds.
    pub created_at: u64,
}

/// An invitation: a single-use, expiring bearer token's record.
///
/// The token itself is never here — only its digest, exactly as
/// `keys.rs` keeps a key's. An expired invitation is dead whether or
/// not anything marked it; an accepted or revoked one refuses every
/// later presentation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Invitation {
    /// SHA-256 of the bearer token, hex — and the invitation's lookup
    /// key, so an unknown id and a wrong secret are the same answer.
    pub token_digest: String,
    /// Unix seconds after which the invitation refuses acceptance.
    pub expires_at: u64,
    /// Whether the invitation holds one of the workspace's seats while
    /// it is live. Issued invitations always do; the field is the
    /// record's answer to what holds the seat.
    pub seat: bool,
    /// The user the invitation joins.
    pub user: UserId,
    /// The role acceptance grants — `admin` or `member`, never `owner`.
    /// Ownership moves only through [`Workspace::transfer_ownership`].
    pub role: Role,
    /// The member that issued the invitation.
    pub invited_by: UserId,
    /// When the invitation was issued, as Unix seconds.
    pub invited_at: u64,
    /// `pending`, `accepted`, or `revoked`.
    pub state: InvitationState,
    /// When the invitation was answered, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<u64>,
}

/// A workspace: one record for both kinds, the `kind` marker deciding
/// which membership rules apply.
///
/// The workspace owns its billing account, its keys, and its seats; a
/// user owns nothing here but a membership. Every collection is the
/// full record — suspended and removed members and dead keys stay in
/// the maps, because the maps are the history as well as the state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    /// The workspace's id — stable, and the identity quota and billing
    /// bind to.
    pub id: WorkspaceId,
    /// `personal` or `organization`.
    pub kind: WorkspaceKind,
    /// The workspace's display name.
    pub name: String,
    /// The billing account every key this workspace owns spends
    /// against. A reference, never a credential.
    pub billing: BillingAccountId,
    /// How many members the workspace may hold, when it is bounded. A
    /// live invitation holds a seat until it is accepted, withdrawn, or
    /// expired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seats: Option<u32>,
    /// User id to membership. Invited, suspended, and removed
    /// memberships stay in the map — their states are answers, not
    /// absences.
    #[serde(default)]
    pub members: BTreeMap<UserId, Membership>,
    /// Key id to key record — every key the workspace ever issued,
    /// revoked ones included.
    #[serde(default)]
    pub keys: BTreeMap<KeyId, WorkspaceKey>,
    /// Token digest to invitation — live and answered alike.
    #[serde(default)]
    pub invitations: BTreeMap<String, Invitation>,
    /// When the workspace was created, as Unix seconds.
    pub created_at: u64,
}

/// Why a membership, key, or invitation operation was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The user holds no membership the workspace knows.
    NotMember {
        workspace: WorkspaceId,
        user: UserId,
    },
    /// The membership exists but is not active; `state` says where it
    /// stands — a removed member is a different answer from a missing
    /// one.
    NotActive {
        workspace: WorkspaceId,
        user: UserId,
        state: MemberState,
    },
    /// The acting member's role does not cover the action.
    Forbidden {
        workspace: WorkspaceId,
        user: UserId,
        action: &'static str,
    },
    /// The user already belongs — invited or active.
    AlreadyMember {
        workspace: WorkspaceId,
        user: UserId,
    },
    /// The operation does not apply to a personal workspace.
    PersonalWorkspace(WorkspaceId),
    /// The operation would leave the workspace without an active owner
    /// — transfer ownership first.
    LastOwner { workspace: WorkspaceId },
    /// Ownership moves only through `transfer_ownership`, never through
    /// an invitation.
    OwnershipByTransfer { workspace: WorkspaceId },
    /// Active members plus live invitations already fill the seats.
    SeatLimit { workspace: WorkspaceId, seats: u32 },
    /// A seat count below the active membership would orphan members.
    SeatsBelowMembers {
        workspace: WorkspaceId,
        seats: u32,
        members: u64,
    },
    /// The digest names no invitation the workspace holds — an unknown
    /// id and a wrong secret are the same answer.
    UnknownInvitation,
    /// An invitation already carries this token digest.
    DuplicateInvitation,
    /// The invitation's deadline has passed.
    InvitationExpired,
    /// The invitation was already answered — a second use is a replay,
    /// not a join.
    InvitationClosed { state: InvitationState },
    /// The id already names a key the workspace holds.
    DuplicateKey { key: KeyId },
    /// The id names no key the workspace holds.
    UnknownKey { key: KeyId },
    /// The key exists but is not active.
    KeyClosed { key: KeyId },
    /// A digest field is not 64 hex characters — a SHA-256, never the
    /// secret; the field name says which.
    MalformedDigest(&'static str),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotMember { workspace, user } => {
                write!(f, "user `{user}` isn't a member of workspace `{workspace}`")
            }
            Self::NotActive {
                workspace,
                user,
                state,
            } => write!(
                f,
                "user `{user}`'s membership in workspace `{workspace}` is {state}"
            ),
            Self::Forbidden {
                workspace,
                user,
                action,
            } => write!(
                f,
                "user `{user}`'s role in workspace `{workspace}` doesn't allow {action}"
            ),
            Self::AlreadyMember { workspace, user } => write!(
                f,
                "user `{user}` already belongs to workspace `{workspace}`"
            ),
            Self::PersonalWorkspace(workspace) => write!(
                f,
                "workspace `{workspace}` is a personal workspace, so only its owner can be a member"
            ),
            Self::LastOwner { workspace } => write!(
                f,
                "this change would leave workspace `{workspace}` without an active owner; \
                 transfer ownership to another member first"
            ),
            Self::OwnershipByTransfer { workspace } => write!(
                f,
                "to make someone an owner of workspace `{workspace}`, transfer ownership to them"
            ),
            Self::SeatLimit { workspace, seats } => write!(
                f,
                "workspace `{workspace}` has used all {seats} seats. Remove a member \
                 or cancel an invitation to free a seat"
            ),
            Self::SeatsBelowMembers {
                workspace,
                seats,
                members,
            } => write!(
                f,
                "workspace `{workspace}` has {members} active members, so you can't \
                 reduce its seats to {seats}"
            ),
            Self::UnknownInvitation => {
                write!(f, "this workspace has no matching invitation")
            }
            Self::DuplicateInvitation => {
                write!(f, "an invitation with this token already exists")
            }
            Self::InvitationExpired => write!(f, "the invitation has expired"),
            Self::InvitationClosed { state } => write!(
                f,
                "the invitation is already {state}; each invitation works only once"
            ),
            Self::DuplicateKey { key } => {
                write!(f, "API key `{key}` already exists in this workspace")
            }
            Self::UnknownKey { key } => {
                write!(f, "API key `{key}` doesn't belong to this workspace")
            }
            Self::KeyClosed { key } => write!(f, "API key `{key}` is no longer active"),
            Self::MalformedDigest(field) => write!(
                f,
                "{field} must be a SHA-256 hash (64 hex characters), not the secret itself"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// Whether a value is shaped like a SHA-256 digest: 64 hex characters.
fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl Workspace {
    /// A workspace record with its first owner — the one membership a
    /// workspace is born with, and the member no rule removes last.
    #[must_use]
    pub fn new(
        id: WorkspaceId,
        kind: WorkspaceKind,
        name: String,
        billing: BillingAccountId,
        owner: UserId,
        seats: Option<u32>,
        now: u64,
    ) -> Self {
        let mut members = BTreeMap::new();
        members.insert(
            owner.clone(),
            Membership {
                user: owner,
                workspace: id.clone(),
                role: Role::Owner,
                state: MemberState::Active,
                granted_at: now,
                activated_at: Some(now),
                suspended_at: None,
                removed_at: None,
            },
        );
        Self {
            id,
            kind,
            name,
            billing,
            seats,
            members,
            keys: BTreeMap::new(),
            invitations: BTreeMap::new(),
            created_at: now,
        }
    }

    /// A personal workspace — one owner, no seats to bound.
    #[must_use]
    pub fn personal(
        id: WorkspaceId,
        name: String,
        billing: BillingAccountId,
        owner: UserId,
        now: u64,
    ) -> Self {
        Self::new(id, WorkspaceKind::Personal, name, billing, owner, None, now)
    }

    /// An organization workspace — invitations, roles, and seats.
    #[must_use]
    pub fn organization(
        id: WorkspaceId,
        name: String,
        billing: BillingAccountId,
        owner: UserId,
        seats: Option<u32>,
        now: u64,
    ) -> Self {
        Self::new(
            id,
            WorkspaceKind::Organization,
            name,
            billing,
            owner,
            seats,
            now,
        )
    }

    /// The account this workspace's quota and billing bind to.
    ///
    /// The reference never moves with membership or keys: rotating a
    /// key issues a new key against the same account, and removing a
    /// member frees no budget — the attachment is the workspace's, not
    /// theirs.
    #[must_use]
    pub fn billing(&self) -> &BillingAccountId {
        &self.billing
    }

    /// The membership a user holds, in whatever state it stands.
    #[must_use]
    pub fn membership(&self, user: &UserId) -> Option<&Membership> {
        self.members.get(user)
    }

    /// The membership a user acts through — `active`, or the typed
    /// reason it is not: `NotMember` when the user was never here,
    /// `NotActive` naming the state the membership stands in.
    pub fn active_membership(&self, user: &UserId) -> Result<&Membership, Refusal> {
        match self.members.get(user) {
            Some(membership) if membership.is_active() => Ok(membership),
            Some(membership) => Err(Refusal::NotActive {
                workspace: self.id.clone(),
                user: user.clone(),
                state: membership.state,
            }),
            None => Err(Refusal::NotMember {
                workspace: self.id.clone(),
                user: user.clone(),
            }),
        }
    }

    /// How many memberships are active — the count the seat bound
    /// reads.
    #[must_use]
    pub fn active_members(&self) -> u64 {
        self.members
            .values()
            .filter(|membership| membership.is_active())
            .count() as u64
    }

    /// How many invitations still hold a seat: pending, marked `seat`,
    /// and not yet expired.
    fn live_invitations(&self, now: u64) -> u64 {
        self.invitations
            .values()
            .filter(|invitation| {
                invitation.state == InvitationState::Pending
                    && invitation.seat
                    && invitation.expires_at > now
            })
            .count() as u64
    }

    /// The live keys a user's membership answers for.
    ///
    /// An active membership answers its active keys; an invited,
    /// suspended, or removed member's keys stay in [`Workspace::keys`]
    /// — attribution does not disappear — but stop answering here.
    #[must_use]
    pub fn keys_visible(&self, user: &UserId) -> Vec<KeyId> {
        if self
            .members
            .get(user)
            .is_none_or(|membership| !membership.is_active())
        {
            return Vec::new();
        }
        self.keys
            .values()
            .filter(|key| key.owner == *user && key.status == KeyStatus::Active)
            .map(|key| key.id.clone())
            .collect()
    }

    /// Issue an invitation to a user, granting `admin` or `member`.
    ///
    /// The acting member must be an active owner or admin; ownership is
    /// never granted this way. The invitation arrives as a digest — the
    /// token is the caller's to generate, hash, and deliver — and it
    /// holds a seat until it is accepted, withdrawn, or expired. A user
    /// whose earlier membership was removed may be invited again; the
    /// record keeps its `removed_at`.
    pub fn invite(
        &mut self,
        actor: &UserId,
        user: &UserId,
        role: Role,
        token_digest: String,
        expires_at: u64,
        now: u64,
    ) -> Result<Invitation, Refusal> {
        let member = self.active_membership(actor)?;
        if self.kind == WorkspaceKind::Personal {
            return Err(Refusal::PersonalWorkspace(self.id.clone()));
        }
        if !member.permits(Permission::ManageMembers) {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "invite",
            });
        }
        if role == Role::Owner {
            return Err(Refusal::OwnershipByTransfer {
                workspace: self.id.clone(),
            });
        }
        if self
            .members
            .get(user)
            .is_some_and(|membership| membership.state != MemberState::Removed)
        {
            return Err(Refusal::AlreadyMember {
                workspace: self.id.clone(),
                user: user.clone(),
            });
        }
        if !is_digest(&token_digest) {
            return Err(Refusal::MalformedDigest("token-digest"));
        }
        if expires_at <= now {
            return Err(Refusal::InvitationExpired);
        }
        if self.invitations.contains_key(&token_digest) {
            return Err(Refusal::DuplicateInvitation);
        }
        if self.seats.is_some_and(|seats| {
            self.active_members() + self.live_invitations(now) >= u64::from(seats)
        }) {
            return Err(Refusal::SeatLimit {
                workspace: self.id.clone(),
                seats: self.seats.unwrap_or_default(),
            });
        }
        let mut membership = Membership {
            user: user.clone(),
            workspace: self.id.clone(),
            role,
            state: MemberState::Invited,
            granted_at: now,
            activated_at: None,
            suspended_at: None,
            removed_at: None,
        };
        if let Some(prior) = self.members.get(user) {
            membership.removed_at = prior.removed_at;
        }
        self.members.insert(user.clone(), membership);
        let invitation = Invitation {
            token_digest: token_digest.clone(),
            expires_at,
            seat: true,
            user: user.clone(),
            role,
            invited_by: actor.clone(),
            invited_at: now,
            state: InvitationState::Pending,
            answered_at: None,
        };
        self.invitations.insert(token_digest, invitation.clone());
        Ok(invitation)
    }

    /// Accept an invitation. The token's digest is the authority —
    /// whoever holds it joins, which is why the token is a secret the
    /// inviter delivers out of band.
    ///
    /// Acceptance is single-use: a consumed or withdrawn invitation
    /// answers `InvitationClosed`, an expired one `InvitationExpired`,
    /// and a digest that names nothing `UnknownInvitation`. The
    /// invited membership must still stand — a removed invitee's token
    /// joins no one — and a seat that filled between issue and
    /// acceptance refuses with `SeatLimit`.
    pub fn accept(&mut self, token_digest: &str, now: u64) -> Result<Membership, Refusal> {
        let user = {
            let invitation = self
                .invitations
                .get(token_digest)
                .ok_or(Refusal::UnknownInvitation)?;
            if invitation.state != InvitationState::Pending {
                return Err(Refusal::InvitationClosed {
                    state: invitation.state,
                });
            }
            if invitation.expires_at <= now {
                return Err(Refusal::InvitationExpired);
            }
            invitation.user.clone()
        };
        match self.members.get(&user) {
            Some(membership) if membership.state == MemberState::Invited => {}
            Some(membership) => {
                return Err(Refusal::NotActive {
                    workspace: self.id.clone(),
                    user,
                    state: membership.state,
                });
            }
            None => {
                return Err(Refusal::NotMember {
                    workspace: self.id.clone(),
                    user,
                });
            }
        }
        if self
            .seats
            .is_some_and(|seats| self.active_members() >= u64::from(seats))
        {
            return Err(Refusal::SeatLimit {
                workspace: self.id.clone(),
                seats: self.seats.unwrap_or_default(),
            });
        }
        let record = self.members.get_mut(&user).unwrap();
        record.state = MemberState::Active;
        record.activated_at = Some(now);
        let membership = record.clone();
        let invitation = self.invitations.get_mut(token_digest).unwrap();
        invitation.state = InvitationState::Accepted;
        invitation.answered_at = Some(now);
        Ok(membership)
    }

    /// Withdraw a pending invitation. The record stays — a revoked
    /// invitation is a state, not an absence — and the invited
    /// membership closes with it.
    pub fn withdraw_invitation(
        &mut self,
        actor: &UserId,
        token_digest: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        if !self
            .active_membership(actor)?
            .permits(Permission::ManageMembers)
        {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "withdraw-invitation",
            });
        }
        let user = {
            let invitation = self
                .invitations
                .get_mut(token_digest)
                .ok_or(Refusal::UnknownInvitation)?;
            if invitation.state != InvitationState::Pending {
                return Err(Refusal::InvitationClosed {
                    state: invitation.state,
                });
            }
            invitation.state = InvitationState::Revoked;
            invitation.answered_at = Some(now);
            invitation.user.clone()
        };
        if let Some(membership) = self.members.get_mut(&user)
            && membership.state == MemberState::Invited
        {
            membership.state = MemberState::Removed;
            membership.removed_at = Some(now);
        }
        Ok(())
    }

    /// Remove a member — or let a member leave, when actor and target
    /// are the same user.
    ///
    /// The seniority rule is rank: an owner removes any other member,
    /// an admin removes members only, and a member removes no one but
    /// itself. The owner never exits — removing the last active owner
    /// is `LastOwner`, and ownership moves through transfer first. The
    /// record stays with `state: removed`, so the next authorization
    /// names the removal.
    pub fn remove_member(
        &mut self,
        actor: &UserId,
        target: &UserId,
        now: u64,
    ) -> Result<Membership, Refusal> {
        let actor_role = self.active_membership(actor)?.role;
        let target_member = match self.members.get(target) {
            Some(membership) if membership.state != MemberState::Removed => membership.clone(),
            Some(membership) => {
                return Err(Refusal::NotActive {
                    workspace: self.id.clone(),
                    user: target.clone(),
                    state: membership.state,
                });
            }
            None => {
                return Err(Refusal::NotMember {
                    workspace: self.id.clone(),
                    user: target.clone(),
                });
            }
        };
        let permitted = if actor == target {
            target_member.role != Role::Owner
        } else {
            match actor_role {
                Role::Owner => true,
                Role::Admin => target_member.role == Role::Member,
                Role::Member => false,
            }
        };
        if !permitted {
            return Err(
                if target_member.role == Role::Owner && target_member.state == MemberState::Active {
                    Refusal::LastOwner {
                        workspace: self.id.clone(),
                    }
                } else {
                    Refusal::Forbidden {
                        workspace: self.id.clone(),
                        user: actor.clone(),
                        action: "remove-member",
                    }
                },
            );
        }
        self.revoke(target, now)?;
        Ok(self.members[target].clone())
    }

    /// Revoke a membership outright — the removal the record keeps.
    ///
    /// The state flips to `removed` and `keys_visible` stops answering
    /// the user's keys, immediately and for good. Nothing is deleted:
    /// the membership, the keys, and every invitation the user held all
    /// stay in the record. Removing the last active owner refuses —
    /// a workspace always stands under one.
    pub fn revoke(&mut self, user: &UserId, now: u64) -> Result<(), Refusal> {
        match self.members.get(user) {
            Some(membership)
                if membership.state == MemberState::Active && membership.role == Role::Owner =>
            {
                return Err(Refusal::LastOwner {
                    workspace: self.id.clone(),
                });
            }
            Some(membership) if membership.state != MemberState::Removed => {}
            Some(membership) => {
                return Err(Refusal::NotActive {
                    workspace: self.id.clone(),
                    user: user.clone(),
                    state: membership.state,
                });
            }
            None => {
                return Err(Refusal::NotMember {
                    workspace: self.id.clone(),
                    user: user.clone(),
                });
            }
        }
        // A removed member's pending invitations die with it — they can
        // no longer be accepted, and they stop holding seats.
        for invitation in self.invitations.values_mut() {
            if invitation.user == *user && invitation.state == InvitationState::Pending {
                invitation.state = InvitationState::Revoked;
                invitation.answered_at = Some(now);
            }
        }
        let record = self.members.get_mut(user).unwrap();
        record.state = MemberState::Removed;
        record.removed_at = Some(now);
        Ok(())
    }

    /// Park a member: the membership stands but counts for nothing —
    /// no seat, no keys, no permissions — until reinstated.
    ///
    /// The rank rule is removal's: an owner suspends any other member,
    /// an admin suspends members only, and suspending the owner is
    /// `LastOwner` because a suspended owner acts for no one.
    pub fn suspend(&mut self, actor: &UserId, target: &UserId, now: u64) -> Result<(), Refusal> {
        let actor_role = self.active_membership(actor)?.role;
        if !actor_role.permits(Permission::ManageMembers) {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "suspend",
            });
        }
        let target_role = self.active_membership(target)?.role;
        let permitted = match actor_role {
            Role::Owner => target_role != Role::Owner,
            Role::Admin => target_role == Role::Member,
            Role::Member => false,
        };
        if !permitted {
            return Err(if target_role == Role::Owner {
                Refusal::LastOwner {
                    workspace: self.id.clone(),
                }
            } else {
                Refusal::Forbidden {
                    workspace: self.id.clone(),
                    user: actor.clone(),
                    action: "suspend",
                }
            });
        }
        let record = self.members.get_mut(target).unwrap();
        record.state = MemberState::Suspended;
        record.suspended_at = Some(now);
        Ok(())
    }

    /// Return a suspended member to active — provided a seat is still
    /// there to take.
    pub fn reinstate(&mut self, actor: &UserId, target: &UserId, now: u64) -> Result<(), Refusal> {
        if !self
            .active_membership(actor)?
            .permits(Permission::ManageMembers)
        {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "reinstate",
            });
        }
        match self.members.get(target) {
            Some(membership) if membership.state == MemberState::Suspended => {}
            Some(membership) => {
                return Err(Refusal::NotActive {
                    workspace: self.id.clone(),
                    user: target.clone(),
                    state: membership.state,
                });
            }
            None => {
                return Err(Refusal::NotMember {
                    workspace: self.id.clone(),
                    user: target.clone(),
                });
            }
        }
        if self
            .seats
            .is_some_and(|seats| self.active_members() >= u64::from(seats))
        {
            return Err(Refusal::SeatLimit {
                workspace: self.id.clone(),
                seats: self.seats.unwrap_or_default(),
            });
        }
        let record = self.members.get_mut(target).unwrap();
        record.state = MemberState::Active;
        record.activated_at = Some(now);
        Ok(())
    }

    /// Hand ownership to another member, atomically.
    ///
    /// The acting member must be the workspace's active owner — no one
    /// else can grant ownership, and an owner that is not active grants
    /// nothing. The acting owner becomes an admin and the target — an
    /// active member of any other role — becomes the owner, so the
    /// workspace never passes through zero or two owners.
    pub fn transfer_ownership(&mut self, actor: &UserId, target: &UserId) -> Result<(), Refusal> {
        let member = self.active_membership(actor)?;
        if self.kind == WorkspaceKind::Personal {
            return Err(Refusal::PersonalWorkspace(self.id.clone()));
        }
        if member.role != Role::Owner || actor == target {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "transfer-ownership",
            });
        }
        self.active_membership(target)?;
        self.members.get_mut(actor).unwrap().role = Role::Admin;
        self.members.get_mut(target).unwrap().role = Role::Owner;
        Ok(())
    }

    /// Set the workspace's seat bound — or lift it with `None`.
    ///
    /// Seats bound active members plus live invitations. Setting a
    /// bound below the active membership is refused rather than
    /// orphaning members the next validation would refuse to read.
    pub fn set_seats(&mut self, actor: &UserId, seats: Option<u32>) -> Result<(), Refusal> {
        let member = self.active_membership(actor)?;
        if self.kind == WorkspaceKind::Personal {
            return Err(Refusal::PersonalWorkspace(self.id.clone()));
        }
        if member.role != Role::Owner {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "set-seats",
            });
        }
        let active = self.active_members();
        if seats.is_some_and(|seats| u64::from(seats) < active) {
            return Err(Refusal::SeatsBelowMembers {
                workspace: self.id.clone(),
                seats: seats.unwrap_or_default(),
                members: active,
            });
        }
        self.seats = seats;
        Ok(())
    }

    /// Issue a key to a member. The workspace owns the key — the
    /// `owner` field is attribution, so what the key does keeps a name
    /// after the member is gone.
    ///
    /// An admin issues for anyone; a member issues for itself alone.
    /// The secret arrives as a digest and never exists here in any
    /// other form.
    pub fn issue_key(
        &mut self,
        actor: &UserId,
        owner: &UserId,
        id: KeyId,
        secret_digest: String,
        now: u64,
    ) -> Result<WorkspaceKey, Refusal> {
        let member = self.active_membership(actor)?;
        if !(member.permits(Permission::ManageKeys)
            || (actor == owner && member.permits(Permission::ManageOwnKeys)))
        {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "issue-key",
            });
        }
        self.active_membership(owner)?;
        if !is_digest(&secret_digest) {
            return Err(Refusal::MalformedDigest("secret-digest"));
        }
        if self.keys.contains_key(&id) {
            return Err(Refusal::DuplicateKey { key: id });
        }
        let key = WorkspaceKey {
            id: id.clone(),
            owner: owner.clone(),
            secret_digest,
            status: KeyStatus::Active,
            rotated_from: None,
            created_at: now,
        };
        self.keys.insert(id, key.clone());
        Ok(key)
    }

    /// Rotate a key: revoke it and issue its successor in one step.
    ///
    /// The new key belongs to the same member and carries
    /// `rotated_from` back to the old id, so the chain of credentials
    /// reads as one line of attribution. Nothing else moves — a
    /// rotation is a new secret, never a new quota.
    pub fn rotate_key(
        &mut self,
        actor: &UserId,
        key: &KeyId,
        new_id: KeyId,
        secret_digest: String,
        now: u64,
    ) -> Result<WorkspaceKey, Refusal> {
        let (owner, status) = self
            .keys
            .get(key)
            .map(|record| (record.owner.clone(), record.status))
            .ok_or_else(|| Refusal::UnknownKey { key: key.clone() })?;
        let member = self.active_membership(actor)?;
        if !(member.permits(Permission::ManageKeys)
            || (actor == &owner && member.permits(Permission::ManageOwnKeys)))
        {
            return Err(Refusal::Forbidden {
                workspace: self.id.clone(),
                user: actor.clone(),
                action: "rotate-key",
            });
        }
        if status != KeyStatus::Active {
            return Err(Refusal::KeyClosed { key: key.clone() });
        }
        if !is_digest(&secret_digest) {
            return Err(Refusal::MalformedDigest("secret-digest"));
        }
        if self.keys.contains_key(&new_id) {
            return Err(Refusal::DuplicateKey { key: new_id });
        }
        self.keys.get_mut(key).unwrap().status = KeyStatus::Revoked;
        let rotated = WorkspaceKey {
            id: new_id.clone(),
            owner,
            secret_digest,
            status: KeyStatus::Active,
            rotated_from: Some(key.clone()),
            created_at: now,
        };
        self.keys.insert(new_id, rotated.clone());
        Ok(rotated)
    }

    /// The checks a workspace record must pass before anything trusts
    /// it.
    ///
    /// This is the fail-closed boundary for a record the caller
    /// persisted and read back: a membership filed under the wrong user
    /// or naming another workspace, a workspace without exactly one
    /// active owner, a personal workspace holding more than its owner,
    /// a seat count below the membership, a key whose owner was never a
    /// member, a digest that is not 64 hex characters, a pending
    /// invitation with no invited membership — any of them refuses the
    /// whole record rather than serving the part that parsed.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err(format!("workspace `{}` carries an empty name", self.id));
        }
        if self.billing.as_str().is_empty() {
            return Err(format!(
                "workspace `{}` carries no billing account",
                self.id
            ));
        }
        let mut owners = 0_u64;
        let mut active = 0_u64;
        for (user, membership) in &self.members {
            if membership.user != *user {
                return Err(format!(
                    "workspace `{}` files a membership for `{}` under `{user}`",
                    self.id, membership.user
                ));
            }
            if membership.workspace != self.id {
                return Err(format!(
                    "workspace `{}` files a membership naming `{}`",
                    self.id, membership.workspace
                ));
            }
            if membership.is_active() {
                active += 1;
                if membership.role == Role::Owner {
                    owners += 1;
                }
            }
        }
        if owners != 1 {
            return Err(format!(
                "workspace `{}` has {owners} active owners — a workspace stands \
                 under exactly one",
                self.id
            ));
        }
        if self.kind == WorkspaceKind::Personal && (self.members.len() != 1 || active != 1) {
            return Err(format!(
                "personal workspace `{}` holds more than its owner",
                self.id
            ));
        }
        if self.seats.is_some_and(|seats| u64::from(seats) < active) {
            return Err(format!(
                "workspace `{}` seats {} below its {active} active members",
                self.id,
                self.seats.unwrap_or_default()
            ));
        }
        for key in self.keys.values() {
            if !self.members.contains_key(&key.owner) {
                return Err(format!(
                    "workspace `{}` holds key `{}` for `{}`, who was never a member",
                    self.id, key.id, key.owner
                ));
            }
            if !is_digest(&key.secret_digest) {
                return Err(format!(
                    "workspace `{}` key `{}` carries a digest that is not 64 hex \
                     characters",
                    self.id, key.id
                ));
            }
        }
        for (digest, invitation) in &self.invitations {
            if invitation.token_digest != *digest {
                return Err(format!(
                    "workspace `{}` files an invitation under the wrong digest",
                    self.id
                ));
            }
            if !is_digest(&invitation.token_digest) {
                return Err(format!(
                    "workspace `{}` carries an invitation digest that is not 64 hex \
                     characters",
                    self.id
                ));
            }
            if invitation.state == InvitationState::Pending
                && self
                    .members
                    .get(&invitation.user)
                    .is_none_or(|membership| membership.state != MemberState::Invited)
            {
                return Err(format!(
                    "workspace `{}` holds a pending invitation for `{}` with no \
                     invited membership",
                    self.id, invitation.user
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-hex stand-in for a SHA-256 digest.
    fn digest(byte: u8) -> String {
        format!("{byte:064x}")
    }

    fn user(name: &str) -> UserId {
        UserId::from(name)
    }

    fn organization(seats: Option<u32>) -> (Workspace, UserId) {
        let owner = user("u_owner");
        let workspace = Workspace::organization(
            WorkspaceId::from("ws_eng"),
            "engineering".to_string(),
            BillingAccountId::from("bill_acme"),
            owner.clone(),
            seats,
            1_000,
        );
        (workspace, owner)
    }

    /// Invite a user and accept — the whole join, at times that leave
    /// the invitation live.
    fn join(workspace: &mut Workspace, actor: &UserId, member: &UserId, role: Role, token: u8) {
        workspace
            .invite(actor, member, role, digest(token), 9_999, 2_000)
            .unwrap();
        workspace.accept(&digest(token), 2_100).unwrap();
    }

    #[test]
    fn the_role_matrix_answers_for_each_role() {
        // member: its own keys and its own usage, nothing wider.
        assert!(Role::Member.permits(Permission::ManageOwnKeys));
        assert!(Role::Member.permits(Permission::ViewOwnUsage));
        for permission in [
            Permission::ManageBilling,
            Permission::TransferOwnership,
            Permission::ManageMembers,
            Permission::ManageKeys,
            Permission::ViewUsage,
        ] {
            assert!(!Role::Member.permits(permission), "{permission:?}");
        }
        // admin: members, every key, all usage — never billing or
        // transfer. The workspace-wide grants cover the own-scoped
        // questions.
        assert!(Role::Admin.permits(Permission::ManageMembers));
        assert!(Role::Admin.permits(Permission::ManageKeys));
        assert!(Role::Admin.permits(Permission::ViewUsage));
        assert!(Role::Admin.permits(Permission::ManageOwnKeys));
        assert!(Role::Admin.permits(Permission::ViewOwnUsage));
        assert!(!Role::Admin.permits(Permission::ManageBilling));
        assert!(!Role::Admin.permits(Permission::TransferOwnership));
        // owner: every row.
        for permission in [
            Permission::ManageBilling,
            Permission::TransferOwnership,
            Permission::ManageMembers,
            Permission::ManageKeys,
            Permission::ManageOwnKeys,
            Permission::ViewUsage,
            Permission::ViewOwnUsage,
        ] {
            assert!(Role::Owner.permits(permission), "{permission:?}");
        }
        // the matrix is a table, not prose — the rows themselves answer.
        assert_eq!(Role::Member.permissions().len(), 2);
        assert_eq!(Role::Admin.permissions().len(), 3);
        assert_eq!(Role::Owner.permissions().len(), 7);
    }

    #[test]
    fn removing_the_last_owner_refuses() {
        let (mut workspace, owner) = organization(Some(4));
        let member = user("u_member");
        join(&mut workspace, &owner, &member, Role::Member, 0x11);
        // the owner cannot leave, and no one can remove the owner —
        // either path names the same reason.
        assert!(matches!(
            workspace.remove_member(&owner, &owner, 3_000),
            Err(Refusal::LastOwner { .. })
        ));
        assert!(matches!(
            workspace.remove_member(&member, &owner, 3_000),
            Err(Refusal::LastOwner { .. })
        ));
        assert!(matches!(
            workspace.revoke(&owner, 3_000),
            Err(Refusal::LastOwner { .. })
        ));
        // ordinary members still leave normally.
        workspace.remove_member(&owner, &member, 3_000).unwrap();
        assert_eq!(workspace.members[&member].state, MemberState::Removed);
        workspace.validate().unwrap();
    }

    #[test]
    fn ownership_moves_only_from_an_active_owner() {
        let (mut workspace, owner) = organization(Some(4));
        let admin = user("u_admin");
        let member = user("u_member");
        join(&mut workspace, &owner, &admin, Role::Admin, 0x21);
        join(&mut workspace, &owner, &member, Role::Member, 0x22);
        // a member cannot grant ownership; an admin cannot either.
        assert!(matches!(
            workspace.transfer_ownership(&member, &admin),
            Err(Refusal::Forbidden { .. })
        ));
        assert!(matches!(
            workspace.transfer_ownership(&admin, &member),
            Err(Refusal::Forbidden { .. })
        ));
        // the grant needs an active owner — a suspended one cannot act.
        workspace.members.get_mut(&owner).unwrap().state = MemberState::Suspended;
        assert!(matches!(
            workspace.transfer_ownership(&owner, &admin),
            Err(Refusal::NotActive { .. })
        ));
        workspace.members.get_mut(&owner).unwrap().state = MemberState::Active;
        // nor can ownership land on someone who does not stand as a member.
        assert!(matches!(
            workspace.transfer_ownership(&owner, &user("u_nobody")),
            Err(Refusal::NotMember { .. })
        ));
        workspace.transfer_ownership(&owner, &admin).unwrap();
        assert_eq!(workspace.members[&admin].role, Role::Owner);
        assert_eq!(workspace.members[&owner].role, Role::Admin);
        // the old owner is an admin now — the grant does not repeat.
        assert!(matches!(
            workspace.transfer_ownership(&owner, &member),
            Err(Refusal::Forbidden { .. })
        ));
        workspace.validate().unwrap();
    }

    #[test]
    fn revocation_hides_keys_and_keeps_history() {
        let (mut workspace, owner) = organization(Some(4));
        let member = user("u_member");
        join(&mut workspace, &owner, &member, Role::Member, 0x31);
        workspace
            .issue_key(
                &member,
                &member,
                KeyId::from("key_one"),
                digest(0x01),
                3_000,
            )
            .unwrap();
        workspace
            .issue_key(
                &member,
                &member,
                KeyId::from("key_two"),
                digest(0x02),
                3_000,
            )
            .unwrap();
        assert_eq!(workspace.keys_visible(&member).len(), 2);
        workspace.revoke(&member, 4_000).unwrap();
        // the state flips and the user's keys stop answering —
        // immediately, not at some later sweep.
        assert_eq!(workspace.members[&member].state, MemberState::Removed);
        assert!(workspace.keys_visible(&member).is_empty());
        // nothing is deleted: the membership and both keys still
        // attribute to the user.
        assert!(workspace.members.contains_key(&member));
        assert_eq!(workspace.keys.len(), 2);
        assert!(workspace.keys.values().all(|key| key.owner == member));
        // a removed member is a different answer from a missing one.
        assert!(matches!(
            workspace.remove_member(&owner, &member, 5_000),
            Err(Refusal::NotActive {
                state: MemberState::Removed,
                ..
            })
        ));
        workspace.validate().unwrap();
    }

    #[test]
    fn expired_and_reused_invitations_refuse() {
        let (mut workspace, owner) = organization(Some(4));
        let late = user("u_late");
        workspace
            .invite(&owner, &late, Role::Member, digest(0x41), 2_500, 2_000)
            .unwrap();
        // past the deadline the token is dead whether or not anything
        // marked it.
        assert!(matches!(
            workspace.accept(&digest(0x41), 3_000),
            Err(Refusal::InvitationExpired)
        ));
        // an invitation consumed once is closed to every later
        // presentation.
        let member = user("u_member");
        join(&mut workspace, &owner, &member, Role::Member, 0x42);
        assert!(matches!(
            workspace.accept(&digest(0x42), 3_500),
            Err(Refusal::InvitationClosed {
                state: InvitationState::Accepted
            })
        ));
        // and an unknown id and a wrong secret are the same answer.
        assert!(matches!(
            workspace.accept(&digest(0x43), 3_500),
            Err(Refusal::UnknownInvitation)
        ));
        workspace.validate().unwrap();
    }

    #[test]
    fn the_seat_bound_counts_active_members_and_live_invitations() {
        let (mut workspace, owner) = organization(Some(2));
        // the owner fills one seat; one live invitation fills the other.
        let first = user("u_first");
        workspace
            .invite(&owner, &first, Role::Member, digest(0x51), 9_999, 2_000)
            .unwrap();
        assert!(matches!(
            workspace.invite(
                &owner,
                &user("u_second"),
                Role::Member,
                digest(0x52),
                9_999,
                2_000
            ),
            Err(Refusal::SeatLimit { seats: 2, .. })
        ));
        workspace.accept(&digest(0x51), 2_500).unwrap();
        assert_eq!(workspace.active_members(), 2);
        // the bound will not drop below the members it holds.
        assert!(matches!(
            workspace.set_seats(&owner, Some(1)),
            Err(Refusal::SeatsBelowMembers { .. })
        ));
        // a bound tightened outside the rules still binds at
        // acceptance — a seat that filled in between refuses.
        let (mut tight, tight_owner) = organization(Some(2));
        tight
            .invite(
                &tight_owner,
                &user("u_new"),
                Role::Member,
                digest(0x54),
                9_999,
                2_000,
            )
            .unwrap();
        tight.seats = Some(1);
        assert!(matches!(
            tight.accept(&digest(0x54), 2_500),
            Err(Refusal::SeatLimit { seats: 1, .. })
        ));
        // a freed seat is usable again.
        workspace.revoke(&first, 3_000).unwrap();
        let third = user("u_third");
        workspace
            .invite(&owner, &third, Role::Member, digest(0x53), 9_999, 3_100)
            .unwrap();
        workspace.accept(&digest(0x53), 3_200).unwrap();
        workspace.validate().unwrap();
    }

    #[test]
    fn key_rotation_keeps_the_same_billing_attachment() {
        let (mut workspace, owner) = organization(Some(4));
        let member = user("u_member");
        join(&mut workspace, &owner, &member, Role::Member, 0x61);
        workspace
            .issue_key(
                &member,
                &member,
                KeyId::from("key_old"),
                digest(0x01),
                3_000,
            )
            .unwrap();
        // a member's reach is its own keys — issuing for another member
        // is not in its row.
        assert!(matches!(
            workspace.issue_key(&member, &owner, KeyId::from("key_bad"), digest(0x02), 3_000),
            Err(Refusal::Forbidden { .. })
        ));
        let billing_before = workspace.billing().clone();
        workspace
            .rotate_key(
                &member,
                &KeyId::from("key_old"),
                KeyId::from("key_new"),
                digest(0x03),
                4_000,
            )
            .unwrap();
        // a new secret, the same account — rotation cannot mint a quota.
        assert_eq!(workspace.billing(), &billing_before);
        assert_eq!(
            workspace.keys[&KeyId::from("key_old")].status,
            KeyStatus::Revoked
        );
        let rotated = &workspace.keys[&KeyId::from("key_new")];
        assert_eq!(rotated.rotated_from, Some(KeyId::from("key_old")));
        assert_eq!(rotated.owner, member);
        // removing the member cannot mint one either — the billing
        // reference is the workspace's, not theirs.
        workspace.revoke(&member, 5_000).unwrap();
        assert_eq!(workspace.billing(), &billing_before);
        workspace.validate().unwrap();
    }

    #[test]
    fn suspension_parks_a_member_without_freeing_it() {
        let (mut workspace, owner) = organization(Some(4));
        let member = user("u_member");
        join(&mut workspace, &owner, &member, Role::Member, 0x71);
        workspace
            .issue_key(&member, &member, KeyId::from("key_m"), digest(0x01), 3_000)
            .unwrap();
        workspace.suspend(&owner, &member, 4_000).unwrap();
        // parked: no seat, no keys, no permissions — but still a member.
        assert_eq!(workspace.members[&member].state, MemberState::Suspended);
        assert!(workspace.keys_visible(&member).is_empty());
        assert_eq!(workspace.active_members(), 1);
        workspace.reinstate(&owner, &member, 5_000).unwrap();
        assert_eq!(workspace.keys_visible(&member).len(), 1);
        // a member cannot suspend another member.
        let other = user("u_other");
        join(&mut workspace, &owner, &other, Role::Member, 0x72);
        assert!(matches!(
            workspace.suspend(&other, &member, 6_000),
            Err(Refusal::Forbidden { .. })
        ));
        workspace.validate().unwrap();
    }

    #[test]
    fn a_personal_workspace_holds_its_owner_alone() {
        let owner = user("u_owner");
        let mut workspace = Workspace::personal(
            WorkspaceId::from("ws_home"),
            "home".to_string(),
            BillingAccountId::from("bill_home"),
            owner.clone(),
            1_000,
        );
        assert!(matches!(
            workspace.invite(
                &owner,
                &user("u_guest"),
                Role::Member,
                digest(0x81),
                9_999,
                2_000
            ),
            Err(Refusal::PersonalWorkspace(_))
        ));
        assert!(matches!(
            workspace.transfer_ownership(&owner, &user("u_guest")),
            Err(Refusal::PersonalWorkspace(_))
        ));
        workspace.validate().unwrap();
    }

    #[test]
    fn the_four_identities_never_smudge() {
        let types = [
            std::any::TypeId::of::<UserId>(),
            std::any::TypeId::of::<WorkspaceId>(),
            std::any::TypeId::of::<KeyId>(),
            std::any::TypeId::of::<BillingAccountId>(),
        ];
        for (index, one) in types.iter().enumerate() {
            for other in &types[index + 1..] {
                assert_ne!(one, other);
            }
        }
        // the same text under four types is four identities: billing
        // answers a BillingAccountId, members are keyed by UserId, keys
        // by KeyId — and none of them can stand in for another.
        let (workspace, owner) = organization(None);
        assert_eq!(workspace.id, WorkspaceId::from("ws_eng"));
        assert_eq!(workspace.billing(), &BillingAccountId::from("bill_acme"));
        assert!(workspace.members.contains_key(&owner));
        assert_eq!(workspace.members[&owner].user.as_str(), "u_owner");
    }
}
