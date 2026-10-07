//! Explicit commercial attribution over stable native accounts and workspaces.
//! A binding names records; it grants no product, host, wallet, or payout right.

use super::{Accounts, MemberRef, Refusal, Role, Store, Trouble, active_member};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "openagents.commercial-binding.v1";
const MAX_BINDINGS: usize = 1024;
const MAX_REVISIONS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Product {
    Gateway,
    Plugin,
    Retail,
}

/// The issuer scopes a native source identity. Labels, keys, and wallets are
/// deliberately absent: none of them establishes commercial ownership.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub product: Product,
    pub issuer: String,
    pub account: String,
    pub workspace: Option<String>,
}
impl Source {
    fn native_account_key(&self) -> (bool, &str, &str, Option<&str>) {
        (
            self.product == Product::Retail,
            &self.issuer,
            &self.account,
            self.workspace.as_deref(),
        )
    }
    fn same_native_account(&self, other: &Self) -> bool {
        // Gateway and plugin purchases share tenancy identity. Separate product
        // tags cannot assign that native identity to two commercial customers.
        self.native_account_key() == other.native_account_key()
    }
}

/// A native adapter's current source authorization, obtained under the
/// operator's explicit mapping policy. Read access alone cannot produce it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAuthority {
    pub source: Source,
    /// Native record identity, independent of configured issuer aliases.
    pub native_identity: String,
    pub principal: String,
    pub generation: u64,
    pub policy_digest: String,
    pub previous_authority: Option<String>,
}
impl SourceAuthority {
    /// Exact predecessor reference for a separately reviewed source rotation.
    pub fn digest(&self) -> String {
        digest(self)
    }
}

/// Trusted adapters must re-read their native store and explicit operator
/// policy on every call. This is dependency injection inside the operator;
/// it is never a client-supplied proof or a remotely implemented trait.
pub trait Sources {
    fn authorize(
        &self,
        source: &Source,
        customer: &str,
        workspace: &str,
    ) -> Result<SourceAuthority, String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    pub account: String,
    pub membership_epoch: u64,
    pub workspace_members_epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub schema: String,
    pub binding: String,
    pub revision: u64,
    pub supersedes: Option<String>,
    pub customer: String,
    pub workspace: String,
    pub owner: Owner,
    pub sources: Vec<SourceAuthority>,
    /// An ownership change requires both native workspace owners. Credential
    /// recovery under the same account needs no customer reassignment.
    pub previous_owner: Option<Owner>,
    pub active: bool,
    pub reviewed_at: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub bindings: BTreeMap<String, Vec<Revision>>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
    pub(super) fn validate(&self, store: &Store) -> Result<(), String> {
        if self.bindings.len() > MAX_BINDINGS {
            return Err("Commercial binding limit exceeded.".into());
        }
        let mut occupied = BTreeSet::new();
        let mut associations = BTreeMap::new();
        let mut native_associations = BTreeMap::new();
        for (id, history) in &self.bindings {
            if history.is_empty() || history.len() > MAX_REVISIONS {
                return Err("Commercial binding history is invalid.".into());
            }
            let mut previous: Option<&Revision> = None;
            for r in history {
                r.validate()?;
                if r.binding != *id
                    || r.revision != previous.map_or(1, |p| p.revision + 1)
                    || r.supersedes.as_deref() != previous.map(|p| p.digest.as_str())
                    || !store.accounts.contains_key(&r.customer)
                    || !store.accounts.contains_key(&r.owner.account)
                    || !store.workspaces.contains_key(&r.workspace)
                    || r.previous_owner
                        .as_ref()
                        .is_some_and(|o| !store.accounts.contains_key(&o.account))
                {
                    return Err("Commercial binding lineage is invalid.".into());
                }
                // Retired source identities remain occupied. Reusing an old
                // source for another customer would relabel retained records.
                for source in &r.sources {
                    if let Some(preceding) =
                        previous.and_then(|p| p.sources.iter().find(|s| s.source == source.source))
                    {
                        if source != preceding
                            && (source.native_identity != preceding.native_identity
                                || source.generation < preceding.generation
                                || source.previous_authority.as_deref()
                                    != Some(digest(preceding).as_str()))
                        {
                            return Err("Commercial source authority lineage is invalid.".into());
                        }
                    } else if source.previous_authority.is_some() {
                        return Err("Commercial source authority has no predecessor.".into());
                    }
                    if native_associations
                        .insert(&source.native_identity, id)
                        .is_some_and(|other| other != id)
                    {
                        return Err(
                            "Native records belong to conflicting commercial bindings.".into()
                        );
                    }
                    if associations
                        .insert(source.source.native_account_key(), id)
                        .is_some_and(|other| other != id)
                    {
                        return Err(
                            "A product account belongs to conflicting commercial bindings.".into(),
                        );
                    }
                }
                previous = Some(r);
            }
            if history.last().unwrap().active
                && !occupied.insert((
                    &history.last().unwrap().customer,
                    &history.last().unwrap().workspace,
                ))
            {
                return Err("Commercial customer/workspace selection is ambiguous.".into());
            }
        }
        Ok(())
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
}
fn hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn digest<T: Serialize>(value: &T) -> String {
    let mut v = serde_json::to_value(value).expect("commercial record serializes");
    v.as_object_mut()
        .expect("commercial record is an object")
        .remove("digest");
    format!(
        "sha256:{:x}",
        Sha256::digest(super::canonicalize(&v).as_bytes())
    )
}
impl Revision {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || !identifier(&self.binding)
            || !identifier(&self.customer)
            || !identifier(&self.workspace)
            || !identifier(&self.owner.account)
            || self.revision == 0
            || self.sources.is_empty()
            || self.sources.len() > 8
            || self.digest != digest(self)
            || self.supersedes.as_ref().is_some_and(|v| !hash(v))
        {
            return Err("Invalid commercial binding revision.".into());
        }
        let mut seen = BTreeSet::new();
        for s in &self.sources {
            if !identifier(&s.source.issuer)
                || !identifier(&s.source.account)
                || s.source.workspace.as_ref().is_some_and(|v| !identifier(v))
                || !hash(&s.native_identity)
                || !identifier(&s.principal)
                || !hash(&s.policy_digest)
                || s.previous_authority.as_ref().is_some_and(|v| !hash(v))
                || !seen.insert(&s.source)
            {
                return Err("Invalid or repeated native product account reference.".into());
            }
        }
        Ok(())
    }
}
fn denied(message: impl Into<String>) -> Refusal {
    Refusal::Store(Trouble::Invalid(message.into()))
}
fn owner(store: &Store, workspace: &str, actor: &str) -> Result<Owner, Refusal> {
    let ws = store
        .workspaces
        .get(workspace)
        .ok_or_else(|| Refusal::UnknownWorkspace(workspace.into()))?;
    let m = active_member(ws, actor)?;
    if m.role != Role::Owner {
        return Err(Refusal::Forbidden {
            workspace: workspace.into(),
            account: actor.into(),
            action: "commercial-binding",
        });
    }
    Ok(Owner {
        account: actor.into(),
        membership_epoch: m.epoch,
        workspace_members_epoch: ws.members_epoch,
    })
}

impl Accounts {
    /// Prepare exact references for the operator's separate review control.
    /// No source can enter a binding without its current native authorization.
    pub fn review_commercial(
        &self,
        binding: &str,
        customer: &str,
        workspace: &str,
        actor: &str,
        previous_actor: Option<&str>,
        sources: &[Source],
        adapters: &impl Sources,
    ) -> Result<Revision, Refusal> {
        let store = self.store().map_err(Refusal::Store)?;
        prepare(
            &store,
            binding,
            customer,
            workspace,
            actor,
            previous_actor,
            sources,
            adapters,
            super::unix_now(),
        )
    }

    /// Commit only the exact reviewed native snapshots, revalidated inside
    /// the account lock. A retry of the same committed revision writes nothing.
    pub fn admit_commercial(
        &self,
        review: &Revision,
        approved_digest: &str,
        adapters: &impl Sources,
    ) -> Result<Revision, Refusal> {
        review.validate().map_err(denied)?;
        if approved_digest != review.digest {
            return Err(denied("Approve the exact commercial review digest."));
        }
        let _lock = super::Lock::acquire(&self.dir).map_err(Refusal::Store)?;
        let mut store = super::load(&self.dir).map_err(Refusal::Store)?;
        if store
            .commercial
            .bindings
            .get(&review.binding)
            .and_then(|h| h.last())
            .is_some_and(|r| r == review)
        {
            return Ok(review.clone());
        }
        let refs = review
            .sources
            .iter()
            .map(|s| s.source.clone())
            .collect::<Vec<_>>();
        let current = prepare(
            &store,
            &review.binding,
            &review.customer,
            &review.workspace,
            &review.owner.account,
            review.previous_owner.as_ref().map(|o| o.account.as_str()),
            &refs,
            adapters,
            review.reviewed_at,
        )?;
        let now = super::unix_now();
        if &current != review
            || review.reviewed_at > now
            || now.saturating_sub(review.reviewed_at) > 300
        {
            return Err(denied(
                "Commercial sources, policy, owner, or lineage changed; review again.",
            ));
        }
        let history = store
            .commercial
            .bindings
            .entry(review.binding.clone())
            .or_default();
        if history.len() >= MAX_REVISIONS {
            return Err(denied("Commercial revision limit exceeded."));
        }
        history.push(review.clone());
        let previous = store.digest.clone();
        store.sequence += 1;
        store.supersedes = Some(previous);
        store.seal();
        store
            .validate("commercial account revision")
            .map_err(denied)?;
        super::save(&self.dir, &store).map_err(Refusal::Store)?;
        Ok(review.clone())
    }

    /// Resolve only under fresh membership and native source authorization.
    /// Retained revisions are attribution; this read grants no spending right.
    pub fn commercial_selection(
        &self,
        member: &MemberRef,
        source: &Source,
        adapters: &impl Sources,
    ) -> Result<Option<Revision>, Refusal> {
        let store = self.store().map_err(Refusal::Store)?;
        let ws = store
            .workspaces
            .get(&member.workspace)
            .ok_or_else(|| Refusal::UnknownWorkspace(member.workspace.clone()))?;
        let current = active_member(ws, &member.account)?;
        if current.epoch != member.epoch
            || ws.members_epoch != member.members_epoch
            || current.role != member.role
        {
            return Err(denied("Commercial membership changed."));
        }
        let mut matching = store
            .commercial
            .bindings
            .values()
            .filter_map(|h| h.last())
            .filter(|r| {
                r.active
                    && r.workspace == member.workspace
                    && r.sources.iter().any(|s| &s.source == source)
            });
        let Some(revision) = matching.next() else {
            return Ok(None);
        };
        if matching.next().is_some() {
            return Err(denied("Ambiguous commercial account selection."));
        }
        let actual = adapters
            .authorize(source, &revision.customer, &revision.workspace)
            .map_err(denied)?;
        if !revision.sources.contains(&actual) {
            return Err(denied(
                "Commercial source authority changed; review its lineage again.",
            ));
        }
        Ok(Some(revision.clone()))
    }

    /// Stop new commercial selections while preserving every historical source.
    pub fn retire_commercial(
        &self,
        binding: &str,
        actor: &str,
        expected: &str,
    ) -> Result<Revision, Refusal> {
        self.mutate(|store, now| {
            let old = store
                .commercial
                .bindings
                .get(binding)
                .and_then(|h| h.last())
                .cloned()
                .ok_or_else(|| denied("Unknown commercial binding."))?;
            if old.digest != expected || !old.active {
                return Err(denied("Commercial binding revision changed."));
            }
            let authority = owner(store, &old.workspace, actor)?;
            let mut next = old.clone();
            next.revision += 1;
            next.supersedes = Some(old.digest);
            next.owner = authority;
            next.previous_owner = None;
            next.active = false;
            next.reviewed_at = now;
            next.digest = digest(&next);
            store
                .commercial
                .bindings
                .get_mut(binding)
                .unwrap()
                .push(next.clone());
            Ok(next)
        })
    }
}

fn prepare(
    store: &Store,
    binding: &str,
    customer: &str,
    workspace: &str,
    actor: &str,
    previous_actor: Option<&str>,
    sources: &[Source],
    adapters: &impl Sources,
    reviewed_at: u64,
) -> Result<Revision, Refusal> {
    if !store.accounts.contains_key(customer) {
        return Err(Refusal::UnknownAccount(customer.into()));
    }
    let authority = owner(store, workspace, actor)?;
    // The customer is an active account in the selected workspace, never an
    // inferred wallet, device, profile, or mutable display name.
    active_member(store.workspaces.get(workspace).unwrap(), customer)?;
    let old = store
        .commercial
        .bindings
        .get(binding)
        .and_then(|h| h.last());
    let previous_owner = if let Some(old) = old {
        if !old.active {
            return Err(denied("A retired binding cannot be resurrected."));
        }
        if old.customer != customer || old.workspace != workspace || old.owner.account != actor {
            Some(owner(
                store,
                &old.workspace,
                previous_actor.ok_or_else(|| {
                    denied("Both current workspace owners must approve migration.")
                })?,
            )?)
        } else {
            None
        }
    } else {
        if previous_actor.is_some() {
            return Err(denied("A new binding has no migration lineage."));
        }
        if store.commercial.bindings.len() >= MAX_BINDINGS {
            return Err(denied("Commercial binding limit exceeded."));
        }
        None
    };
    let mut authorized = Vec::new();
    for source in sources {
        if store.commercial.bindings.iter().any(|(id, history)| {
            id != binding
                && history.iter().any(|r| {
                    r.sources
                        .iter()
                        .any(|s| s.source.same_native_account(source))
                })
        }) {
            return Err(denied(
                "Native product account already belongs to another binding.",
            ));
        }
        let proof = adapters
            .authorize(source, customer, workspace)
            .map_err(denied)?;
        if proof.source != *source {
            return Err(denied("Native adapter returned another product identity."));
        }
        if store.commercial.bindings.iter().any(|(id, history)| {
            id != binding
                && history.iter().any(|r| {
                    r.sources
                        .iter()
                        .any(|s| s.native_identity == proof.native_identity)
                })
        }) {
            return Err(denied(
                "Native records already belong to another commercial binding.",
            ));
        }
        let preceding = old.and_then(|r| r.sources.iter().find(|s| &s.source == source));
        if let Some(preceding) = preceding {
            if proof != *preceding
                && (proof.native_identity != preceding.native_identity
                    || proof.generation < preceding.generation
                    || proof.previous_authority.as_deref() != Some(digest(preceding).as_str()))
            {
                return Err(denied(
                    "Changed native authority requires exact reviewed predecessor lineage.",
                ));
            }
        } else if proof.previous_authority.is_some() {
            return Err(denied("A new source has no prior commercial authority."));
        }
        authorized.push(proof);
    }
    authorized.sort_by(|a, b| a.source.cmp(&b.source));
    // A migration cannot drop or replace a native product account. New source
    // joins are explicit; old records remain bound to their original revision.
    if old.is_some_and(|old| {
        old.sources
            .iter()
            .any(|s| !authorized.iter().any(|n| n.source == s.source))
    }) {
        return Err(denied(
            "Commercial migration cannot discard a retained product identity.",
        ));
    }
    let mut review = Revision {
        schema: SCHEMA.into(),
        binding: binding.into(),
        revision: old.map_or(1, |r| r.revision + 1),
        supersedes: old.map(|r| r.digest.clone()),
        customer: customer.into(),
        workspace: workspace.into(),
        owner: authority,
        sources: authorized,
        previous_owner,
        active: true,
        reviewed_at,
        digest: String::new(),
    };
    review.digest = digest(&review);
    review.validate().map_err(denied)?;
    Ok(review)
}

#[cfg(test)]
mod tests {
    use super::super::WorkspaceKind;
    use super::*;
    use std::cell::RefCell;

    struct ApprovedSources(RefCell<BTreeMap<Source, (String, String, SourceAuthority)>>);
    impl Sources for ApprovedSources {
        fn authorize(
            &self,
            source: &Source,
            customer: &str,
            workspace: &str,
        ) -> Result<SourceAuthority, String> {
            self.0
                .borrow()
                .get(source)
                .filter(|(c, w, _)| c == customer && w == workspace)
                .map(|(_, _, proof)| proof.clone())
                .ok_or_else(|| "No current reviewed source-owner mapping.".into())
        }
    }
    fn fixture() -> (
        tempfile::TempDir,
        Accounts,
        String,
        String,
        Source,
        ApprovedSources,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let alice = accounts
            .create_account("Alice", &["key:aaaaaaaaaaaaaaaa".into()])
            .unwrap();
        let workspace = accounts
            .create_workspace(&alice.id, "Alice", WorkspaceKind::Personal, "alice", None)
            .unwrap();
        let source = Source {
            product: Product::Retail,
            issuer: "retail-fixture".into(),
            account: "compute-alice".into(),
            workspace: None,
        };
        let proof = SourceAuthority {
            source: source.clone(),
            native_identity: format!("sha256:{}", "b".repeat(64)),
            principal: "key:alice-retail".into(),
            generation: 1,
            policy_digest: format!("sha256:{}", "a".repeat(64)),
            previous_authority: None,
        };
        let sources = ApprovedSources(RefCell::new(BTreeMap::from([(
            source.clone(),
            (alice.id.clone(), workspace.id.clone(), proof),
        )])));
        (dir, accounts, alice.id, workspace.id, source, sources)
    }
    #[test]
    fn exact_review_replay_and_two_customer_collision_preserve_source() {
        let (_dir, accounts, alice, workspace, source, sources) = fixture();
        let review = accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &workspace,
                &alice,
                None,
                std::slice::from_ref(&source),
                &sources,
            )
            .unwrap();
        assert!(
            accounts
                .admit_commercial(&review, "sha256:wrong", &sources)
                .is_err()
        );
        accounts
            .admit_commercial(&review, &review.digest, &sources)
            .unwrap();
        let sequence = accounts.store().unwrap().sequence;
        accounts
            .admit_commercial(&review, &review.digest, &sources)
            .unwrap();
        assert_eq!(accounts.store().unwrap().sequence, sequence);
        let bob = accounts
            .create_account("Bob", &["key:bbbbbbbbbbbbbbbb".into()])
            .unwrap();
        let bob_ws = accounts
            .create_workspace(&bob.id, "Bob", WorkspaceKind::Personal, "bob", None)
            .unwrap();
        assert!(
            accounts
                .review_commercial(
                    "commercial-bob",
                    &bob.id,
                    &bob_ws.id,
                    &bob.id,
                    None,
                    &[source],
                    &sources
                )
                .is_err()
        );
        assert_eq!(
            accounts.store().unwrap().commercial.bindings["commercial-alice"],
            vec![review]
        );
    }
    #[test]
    fn rotation_requires_exact_predecessor_and_preserves_history() {
        let (_dir, accounts, alice, workspace, source, sources) = fixture();
        let old = accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &workspace,
                &alice,
                None,
                std::slice::from_ref(&source),
                &sources,
            )
            .unwrap();
        accounts
            .admit_commercial(&old, &old.digest, &sources)
            .unwrap();
        let member = accounts.authorize(&workspace, &alice).unwrap();
        let original = sources.0.borrow()[&source].2.clone();
        let mut rotated = original.clone();
        rotated.principal = "key:alice-retail-rotated".into();
        rotated.generation += 1;
        sources.0.borrow_mut().get_mut(&source).unwrap().2 = rotated.clone();
        assert!(
            accounts
                .commercial_selection(&member, &source, &sources)
                .is_err()
        );
        assert!(
            accounts
                .review_commercial(
                    "commercial-alice",
                    &alice,
                    &workspace,
                    &alice,
                    None,
                    std::slice::from_ref(&source),
                    &sources
                )
                .is_err()
        );
        rotated.previous_authority = Some(digest(&original));
        sources.0.borrow_mut().get_mut(&source).unwrap().2 = rotated;
        let new = accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &workspace,
                &alice,
                None,
                std::slice::from_ref(&source),
                &sources,
            )
            .unwrap();
        accounts
            .admit_commercial(&new, &new.digest, &sources)
            .unwrap();
        assert_eq!(
            accounts.store().unwrap().commercial.bindings["commercial-alice"],
            vec![old, new.clone()]
        );
        assert_eq!(
            accounts
                .commercial_selection(&member, &source, &sources)
                .unwrap(),
            Some(new)
        );
    }
    #[test]
    fn plugin_tag_cannot_reassign_an_existing_gateway_account() {
        let (_dir, accounts, alice, workspace, mut source, sources) = fixture();
        let (_, _, mut proof) = sources.0.borrow_mut().remove(&source).unwrap();
        source.product = Product::Gateway;
        proof.source = source.clone();
        sources.0.borrow_mut().insert(
            source.clone(),
            (alice.clone(), workspace.clone(), proof.clone()),
        );
        let review = accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &workspace,
                &alice,
                None,
                std::slice::from_ref(&source),
                &sources,
            )
            .unwrap();
        accounts
            .admit_commercial(&review, &review.digest, &sources)
            .unwrap();
        let bob = accounts
            .create_account("Bob", &["key:bbbbbbbbbbbbbbbb".into()])
            .unwrap();
        let bob_ws = accounts
            .create_workspace(&bob.id, "Bob", WorkspaceKind::Personal, "bob", None)
            .unwrap();
        source.product = Product::Plugin;
        proof.source = source.clone();
        sources
            .0
            .borrow_mut()
            .insert(source.clone(), (bob.id.clone(), bob_ws.id.clone(), proof));
        assert!(
            accounts
                .review_commercial(
                    "commercial-bob",
                    &bob.id,
                    &bob_ws.id,
                    &bob.id,
                    None,
                    &[source],
                    &sources
                )
                .is_err()
        );
    }
    #[test]
    fn approved_team_conversion_and_retirement_do_not_reuse_old_sources() {
        let (_dir, accounts, alice, workspace, source, sources) = fixture();
        let old = accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &workspace,
                &alice,
                None,
                std::slice::from_ref(&source),
                &sources,
            )
            .unwrap();
        accounts
            .admit_commercial(&old, &old.digest, &sources)
            .unwrap();
        let team = accounts
            .create_workspace(&alice, "Team", WorkspaceKind::Organization, "team", None)
            .unwrap();
        let mut proof = old.sources[0].clone();
        proof.policy_digest = format!("sha256:{}", "b".repeat(64));
        proof.previous_authority = Some(digest(&old.sources[0]));
        sources
            .0
            .borrow_mut()
            .insert(source.clone(), (alice.clone(), team.id.clone(), proof));
        assert!(
            accounts
                .review_commercial(
                    "commercial-alice",
                    &alice,
                    &team.id,
                    &alice,
                    None,
                    std::slice::from_ref(&source),
                    &sources
                )
                .is_err()
        );
        let new = accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &team.id,
                &alice,
                Some(&alice),
                std::slice::from_ref(&source),
                &sources,
            )
            .unwrap();
        accounts
            .admit_commercial(&new, &new.digest, &sources)
            .unwrap();
        assert_eq!(
            accounts.store().unwrap().commercial.bindings["commercial-alice"][0],
            old
        );
        assert!(
            accounts
                .commercial_selection(
                    &accounts.authorize(&workspace, &alice).unwrap(),
                    &source,
                    &sources
                )
                .unwrap()
                .is_none()
        );
        let retired = accounts
            .retire_commercial("commercial-alice", &alice, &new.digest)
            .unwrap();
        assert!(!retired.active);
        assert!(
            accounts
                .commercial_selection(
                    &accounts.authorize(&team.id, &alice).unwrap(),
                    &source,
                    &sources
                )
                .unwrap()
                .is_none()
        );
        assert!(
            accounts
                .review_commercial(
                    "replacement",
                    &alice,
                    &team.id,
                    &alice,
                    None,
                    &[source],
                    &sources
                )
                .is_err()
        );
    }
}
