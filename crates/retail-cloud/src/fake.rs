//! Simulated collaborators for tests and the fake-payment acceptance run.
//! Everything here is labeled a simulation; none of it moves money, starts
//! a machine, or reads a credential.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use openagents_wallet::{
    Balance, Channel, IssuedInvoice, LightningWallet, PaymentDirection, PaymentRecord,
    PaymentStatus, Proof, WalletError,
};

use crate::sha256_hex;

/// A receiver wallet that issues fake invoices and reports whatever the test
/// says happened to them.
#[derive(Default)]
pub struct FakeWallet {
    state: Mutex<WalletState>,
}

#[derive(Default)]
struct WalletState {
    issued: u64,
    invoices: BTreeMap<String, Invoice>,
    /// Make the next lookups fail as the node would.
    unreachable: bool,
}

#[derive(Clone)]
struct Invoice {
    amount_msat: u64,
    bolt11: String,
    status: Option<PaymentStatus>,
    received_msat: Option<u64>,
}

impl FakeWallet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn with<T>(&self, f: impl FnOnce(&mut WalletState) -> T) -> T {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut state)
    }

    /// The customer pays the invoice in full.
    pub fn pay_in_full(&self, payment_hash: &str) {
        self.with(|s| {
            if let Some(invoice) = s.invoices.get_mut(payment_hash) {
                invoice.status = Some(PaymentStatus::Succeeded);
                invoice.received_msat = Some(invoice.amount_msat);
            }
        });
    }

    /// The wallet receives a different amount than the invoice asked.
    pub fn pay_amount(&self, payment_hash: &str, received_msat: u64) {
        self.with(|s| {
            if let Some(invoice) = s.invoices.get_mut(payment_hash) {
                invoice.status = Some(PaymentStatus::Succeeded);
                invoice.received_msat = Some(received_msat);
            }
        });
    }

    /// The wallet fails the invoice (for example at expiry).
    pub fn fail(&self, payment_hash: &str) {
        self.with(|s| {
            if let Some(invoice) = s.invoices.get_mut(payment_hash) {
                invoice.status = Some(PaymentStatus::Failed);
            }
        });
    }

    /// The wallet forgets the invoice, as a restored node might.
    pub fn forget(&self, payment_hash: &str) {
        self.with(|s| {
            s.invoices.remove(payment_hash);
        });
    }

    /// Lookups fail until set back.
    pub fn set_unreachable(&self, unreachable: bool) {
        self.with(|s| s.unreachable = unreachable);
    }

    /// How many invoices the wallet issued.
    #[must_use]
    pub fn issued(&self) -> u64 {
        self.with(|s| s.issued)
    }
}

impl LightningWallet for FakeWallet {
    fn node_id(&self) -> String {
        format!("02{}", "ab".repeat(32))
    }

    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        if amount_msat == 0 {
            return Err(WalletError::Invalid("amount".into()));
        }
        Ok(self.with(|s| {
            s.issued += 1;
            let payment_hash = sha256_hex(
                format!("fake-invoice:{}:{}", s.issued, hex::encode(request_hash)).as_bytes(),
            );
            let bolt11 = format!("lnfake{amount_msat}n1{}", &payment_hash[..16]);
            s.invoices.insert(
                payment_hash.clone(),
                Invoice {
                    amount_msat,
                    bolt11: bolt11.clone(),
                    status: Some(PaymentStatus::Pending),
                    received_msat: None,
                },
            );
            IssuedInvoice {
                bolt11,
                payment_hash,
                amount_msat,
                description_hash: hex::encode(request_hash),
                expiry_secs,
                pay_to: self.node_id(),
            }
        }))
    }

    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        Err(WalletError::Node("the fake receiver never pays".into()))
    }

    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        let hash = hex::encode(payment_hash);
        self.with(|s| {
            if s.unreachable {
                return Err(WalletError::Node("fake node unreachable".into()));
            }
            Ok(s.invoices.get(&hash).map(|invoice| PaymentRecord {
                payment_hash: hash.clone(),
                direction: PaymentDirection::Inbound,
                status: invoice.status.unwrap_or(PaymentStatus::Pending),
                amount_msat: invoice.received_msat,
                fee_msat: None,
                preimage: None,
                bolt11: Some(invoice.bolt11.clone()),
                updated_at: 0,
            }))
        })
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        Err(WalletError::Node("the fake receiver has no balance".into()))
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        Ok(Vec::new())
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        Err(WalletError::Node("the fake receiver has no chain".into()))
    }

    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        Err(WalletError::Node(
            "the fake receiver opens no channels".into(),
        ))
    }

    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        Err(WalletError::Node(
            "the fake receiver has no channels".into(),
        ))
    }
}

/// A simulated Boat: sandboxes are records in memory, and the test says
/// when they become ready, fail, or vanish.
#[derive(Default)]
pub struct FakeProvider {
    state: Mutex<ProviderState>,
}

#[derive(Default)]
struct ProviderState {
    next: u64,
    sandboxes: BTreeMap<String, Sandbox>,
    create_calls: u64,
    delete_calls: u64,
    lose_next_ack: bool,
    refuse_next: Option<crate::provision::StartRefusal>,
    listing_fails: bool,
    calls_fail: bool,
    restore_fails: bool,
    ready_after_polls: u32,
    unreadable_usage: bool,
}

#[derive(Clone)]
struct Sandbox {
    resource: crate::provision::Resource,
    spec: crate::provision::CreateSpec,
    polls: u32,
    state: crate::provision::ResourceState,
    usage_seconds: Option<u64>,
}

impl FakeProvider {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn with<T>(&self, f: impl FnOnce(&mut ProviderState) -> T) -> T {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut state)
    }

    /// The next create succeeds but its answer is lost.
    pub fn lose_next_ack(&self) {
        self.with(|s| s.lose_next_ack = true);
    }
    /// The next create is refused.
    pub fn refuse_next(&self, reason: crate::provision::StartRefusal) {
        self.with(|s| s.refuse_next = Some(reason));
    }
    /// Listing sandboxes fails until set back.
    pub fn set_listing_fails(&self, fails: bool) {
        self.with(|s| s.listing_fails = fails);
    }
    /// Every call fails until set back, as an unreachable provider would.
    pub fn set_unreachable(&self, fails: bool) {
        self.with(|s| s.calls_fail = fails);
    }
    /// New sandboxes fail their restore.
    pub fn set_restore_fails(&self, fails: bool) {
        self.with(|s| s.restore_fails = fails);
    }
    /// New sandboxes become ready after this many state polls.
    pub fn set_ready_after_polls(&self, polls: u32) {
        self.with(|s| s.ready_after_polls = polls);
    }
    /// Usage reads return nothing until set back.
    pub fn set_usage_unreadable(&self, unreadable: bool) {
        self.with(|s| s.unreadable_usage = unreadable);
    }
    /// Set a sandbox's billed seconds.
    pub fn set_usage(&self, id: &str, seconds: u64) {
        self.with(|s| {
            if let Some(sandbox) = s.sandboxes.get_mut(id) {
                sandbox.usage_seconds = Some(seconds);
            }
        });
    }
    /// The provider stops or loses a sandbox on its own.
    pub fn lose(&self, id: &str) {
        self.with(|s| {
            if let Some(sandbox) = s.sandboxes.get_mut(id) {
                sandbox.state = crate::provision::ResourceState::Stopped;
            }
        });
    }
    /// Sandboxes that are not deleted.
    #[must_use]
    pub fn active(&self) -> Vec<String> {
        self.with(|s| {
            s.sandboxes
                .iter()
                .filter(|(_, b)| b.state != crate::provision::ResourceState::Deleted)
                .map(|(id, _)| id.clone())
                .collect()
        })
    }
    /// Active sandboxes for one account.
    #[must_use]
    pub fn active_for(&self, account: &str) -> usize {
        self.with(|s| {
            s.sandboxes
                .values()
                .filter(|b| {
                    b.resource.account == account
                        && b.state != crate::provision::ResourceState::Deleted
                })
                .count()
        })
    }
    /// The specification a sandbox was created from.
    #[must_use]
    pub fn spec_of(&self, id: &str) -> Option<crate::provision::CreateSpec> {
        self.with(|s| s.sandboxes.get(id).map(|b| b.spec.clone()))
    }
    #[must_use]
    pub fn create_calls(&self) -> u64 {
        self.with(|s| s.create_calls)
    }
    #[must_use]
    pub fn delete_calls(&self) -> u64 {
        self.with(|s| s.delete_calls)
    }
}

impl crate::provision::Provider for FakeProvider {
    fn create(
        &self,
        spec: &crate::provision::CreateSpec,
    ) -> Result<crate::provision::Resource, crate::provision::ProviderError> {
        use crate::provision::{ProviderError, Resource, ResourceState};
        self.with(|s| {
            if s.calls_fail {
                return Err(ProviderError::Unknown("fake provider unreachable".into()));
            }
            s.create_calls += 1;
            if let Some(reason) = s.refuse_next.take() {
                return Err(ProviderError::Refused(reason));
            }
            s.next += 1;
            let id = format!("sbx-{}", s.next);
            let resource = Resource {
                id: id.clone(),
                provisioning: spec.provisioning.clone(),
                account: spec.account.clone(),
            };
            let state = if s.restore_fails {
                ResourceState::RestoreFailed
            } else {
                ResourceState::Starting
            };
            s.sandboxes.insert(
                id,
                Sandbox {
                    resource: resource.clone(),
                    spec: spec.clone(),
                    polls: 0,
                    state,
                    usage_seconds: Some(0),
                },
            );
            if std::mem::take(&mut s.lose_next_ack) {
                return Err(ProviderError::Unknown("acknowledgment lost".into()));
            }
            Ok(resource)
        })
    }

    fn find(
        &self,
        provisioning: &str,
    ) -> Result<Option<crate::provision::Resource>, crate::provision::ProviderError> {
        self.with(|s| {
            if s.listing_fails || s.calls_fail {
                return Err(crate::provision::ProviderError::Unknown(
                    "fake listing failed".into(),
                ));
            }
            Ok(s.sandboxes
                .values()
                .find(|b| {
                    b.resource.provisioning == provisioning
                        && b.state != crate::provision::ResourceState::Deleted
                })
                .map(|b| b.resource.clone()))
        })
    }

    fn state(
        &self,
        id: &str,
    ) -> Result<crate::provision::ResourceState, crate::provision::ProviderError> {
        use crate::provision::{ProviderError, ResourceState};
        self.with(|s| {
            if s.calls_fail {
                return Err(ProviderError::Unknown("fake provider unreachable".into()));
            }
            let ready_after = s.ready_after_polls;
            let Some(sandbox) = s.sandboxes.get_mut(id) else {
                return Ok(ResourceState::Deleted);
            };
            if sandbox.state == ResourceState::Starting {
                sandbox.polls += 1;
                if sandbox.polls > ready_after {
                    sandbox.state = ResourceState::Ready {
                        address: format!("{id}.fake.boat.invalid"),
                    };
                }
            }
            Ok(sandbox.state.clone())
        })
    }

    fn delete(&self, id: &str) -> Result<(), crate::provision::ProviderError> {
        self.with(|s| {
            if s.calls_fail {
                return Err(crate::provision::ProviderError::Unknown(
                    "fake provider unreachable".into(),
                ));
            }
            s.delete_calls += 1;
            if let Some(sandbox) = s.sandboxes.get_mut(id) {
                sandbox.state = crate::provision::ResourceState::Deleted;
            }
            Ok(())
        })
    }

    fn usage_seconds(&self, id: &str) -> Result<Option<u64>, crate::provision::ProviderError> {
        self.with(|s| {
            if s.calls_fail {
                return Err(crate::provision::ProviderError::Unknown(
                    "fake provider unreachable".into(),
                ));
            }
            if s.unreadable_usage {
                return Ok(None);
            }
            Ok(s.sandboxes.get(id).and_then(|b| b.usage_seconds))
        })
    }
}

/// A simulated sandbox filesystem and command log.
#[derive(Default)]
pub struct FakeSandbox {
    state: Mutex<SandboxState>,
}

#[derive(Default)]
struct SandboxState {
    /// (resource, path) -> contents.
    files: BTreeMap<(String, String), String>,
    /// Every command line the sandbox ran, per resource.
    commands: Vec<(String, String)>,
    head_override: Option<String>,
    dirty: bool,
    no_private_files: bool,
}

impl FakeSandbox {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn with<T>(&self, f: impl FnOnce(&mut SandboxState) -> T) -> T {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut state)
    }

    /// The clone checks out this commit instead of the requested one.
    pub fn set_head(&self, head: &str) {
        self.with(|s| s.head_override = Some(head.into()));
    }
    /// The clone's tree is dirty.
    pub fn set_dirty(&self, dirty: bool) {
        self.with(|s| s.dirty = dirty);
    }
    /// The sandbox cannot keep a private file.
    pub fn set_no_private_files(&self, no: bool) {
        self.with(|s| s.no_private_files = no);
    }
    /// Every file in `resource`, path and contents.
    #[must_use]
    pub fn files(&self, resource: &str) -> Vec<(String, String)> {
        self.with(|s| {
            s.files
                .iter()
                .filter(|((r, _), _)| r == resource)
                .map(|((_, p), c)| (p.clone(), c.clone()))
                .collect()
        })
    }
    /// Every command line the sandbox ran.
    #[must_use]
    pub fn commands(&self) -> Vec<String> {
        self.with(|s| s.commands.iter().map(|(_, c)| c.clone()).collect())
    }
}

impl crate::material::Sandbox for FakeSandbox {
    fn write_private(&self, resource: &str, path: &str, contents: &str) -> crate::Result<bool> {
        Ok(self.with(|s| {
            if s.no_private_files {
                return false;
            }
            s.commands.push((
                resource.into(),
                format!("install -m 0600 /dev/stdin {path}"),
            ));
            s.files
                .insert((resource.into(), path.into()), contents.into());
            true
        }))
    }

    fn clone_source(
        &self,
        resource: &str,
        source: &crate::authority::Source,
    ) -> crate::Result<(String, bool)> {
        Ok(self.with(|s| {
            s.commands.push((
                resource.into(),
                format!(
                    "git clone --no-recurse-submodules {} work && git -C work checkout {}",
                    source.repository, source.commit
                ),
            ));
            (
                s.head_override.clone().unwrap_or(source.commit.clone()),
                !s.dirty,
            )
        }))
    }

    fn remove(&self, resource: &str, path: &str) -> crate::Result<()> {
        self.with(|s| {
            s.commands.push((resource.into(), format!("rm -f {path}")));
            s.files.remove(&(resource.to_owned(), path.to_owned()));
        });
        Ok(())
    }

    fn exists(&self, resource: &str, path: &str) -> crate::Result<bool> {
        Ok(self.with(|s| {
            s.files
                .contains_key(&(resource.to_owned(), path.to_owned()))
        }))
    }
}
