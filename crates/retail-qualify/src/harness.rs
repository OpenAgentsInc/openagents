//! An isolated retail world on fakes: a scratch ledger and journal in a
//! temporary directory, a fake wallet, a fake Boat, a fake sandbox, and a fake
//! task owner. Nothing here reads the real home, a keychain, or a
//! credential, and nothing moves money or starts a machine.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use pay_ledger::Ledger;
use pay_ledger::compute::{Binding, PrincipalKind, Rights, credential_digest};
use retail_cloud::authority::{
    Current, DisclosureConsent, ExecuteGrant, GrantSource, ObserveGrant, Source, SpendRight,
};
use retail_cloud::cancel::{StopEvidence, StopOwner};
use retail_cloud::contract::{self, TaskRequest};
use retail_cloud::dispatch::{OwnerError, TaskStatus};
use retail_cloud::fake::{FakeProvider, FakeSandbox, FakeTaskOwner, FakeWallet};
use retail_cloud::journal::Journal;
use retail_cloud::material::{Credential, CustomerSecret};
use retail_cloud::offer::{self, Capacity, ConfirmedVia, FundedRequest};
use retail_cloud::provision::{self, ProvisionState};
use retail_cloud::retain::{Artifact, Artifacts, Kind, Manifest};
use retail_cloud::{Error, Result, sha256_hex, topup};

/// The simulated start time.
pub const START: i64 = 1_792_000_000;
/// The daily template the fake Boat starts from.
pub const TEMPLATE: &str = "oa-coder-main-20261006";
/// The customer's fake provider key; never a real key.
pub const FAKE_KEY: &str = "sk-fake-retail-acceptance-0000";

/// A fake task owner's stop path.
#[derive(Default)]
pub struct FakeStopper {
    stored: RefCell<Option<StopEvidence>>,
    pub calls: RefCell<u32>,
    /// The next stop's reply is lost.
    pub lose_next: RefCell<bool>,
    /// When the stop is acknowledged.
    pub at: RefCell<i64>,
}

impl StopOwner for FakeStopper {
    fn stop(&self, _: &str, _: &str, _: &str) -> std::result::Result<StopEvidence, OwnerError> {
        *self.calls.borrow_mut() += 1;
        let evidence = StopEvidence {
            at: *self.at.borrow(),
            started: true,
            status: TaskStatus::Cancelled,
            effects: vec![sha256_hex(b"partial edit")],
        };
        *self.stored.borrow_mut() = Some(evidence.clone());
        if self.lose_next.replace(false) {
            return Err(OwnerError::Unknown("stop reply lost".into()));
        }
        Ok(evidence)
    }

    fn stopped(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> std::result::Result<Option<StopEvidence>, OwnerError> {
        Ok(self.stored.borrow().clone())
    }
}

/// The sandbox task owner's retained, scrubbed artifacts.
#[derive(Default)]
pub struct FakeArtifacts {
    manifests: RefCell<BTreeMap<String, (Manifest, BTreeMap<String, Vec<u8>>)>>,
}

impl FakeArtifacts {
    /// Declare a task's patch, check output, and log.
    pub fn declare(&self, funded: &FundedRequest, task: &str, resource: &str) -> String {
        let bytes: BTreeMap<String, Vec<u8>> = [
            (
                "patch",
                b"diff --git a/src/parse.rs b/src/parse.rs\n".to_vec(),
            ),
            ("checks", b"cargo test -p parser: passed\n".to_vec()),
            ("log", b"simulated run: completed\n".to_vec()),
        ]
        .into_iter()
        .map(|(n, b)| (n.to_owned(), b))
        .collect();
        let patch = sha256_hex(&bytes["patch"]);
        let manifest = Manifest {
            execution: funded.execution.clone(),
            task: task.into(),
            resource: resource.into(),
            source: funded.admission.source.clone(),
            engine: "codex".into(),
            artifacts: bytes
                .iter()
                .map(|(name, b)| Artifact {
                    name: name.clone(),
                    kind: match name.as_str() {
                        "patch" => Kind::Patch,
                        "checks" => Kind::Checks,
                        _ => Kind::Log,
                    },
                    digest: sha256_hex(b),
                    size: b.len(),
                })
                .collect(),
        };
        self.manifests
            .borrow_mut()
            .insert(resource.into(), (manifest, bytes));
        patch
    }
}

impl Artifacts for FakeArtifacts {
    fn manifest(&self, resource: &str, _: &str) -> Result<Manifest> {
        self.manifests
            .borrow()
            .get(resource)
            .map(|(m, _)| m.clone())
            .ok_or(Error::Invalid("no declared artifacts"))
    }

    fn read(&self, resource: &str, _: &str, name: &str, _: usize) -> Result<Vec<u8>> {
        self.manifests
            .borrow()
            .get(resource)
            .and_then(|(_, b)| b.get(name).cloned())
            .ok_or(Error::Invalid("artifact unavailable"))
    }
}

/// One isolated world.
pub struct World {
    _dir: tempfile::TempDir,
    ledger_path: PathBuf,
    journal_path: PathBuf,
    pub ledger: Ledger,
    pub journal: Journal,
    pub wallet: FakeWallet,
    pub provider: FakeProvider,
    pub sandbox: FakeSandbox,
    pub owner: FakeTaskOwner,
    pub stopper: FakeStopper,
    pub artifacts: FakeArtifacts,
    pub now: i64,
    /// Process restarts injected so far.
    pub restarts: u32,
}

impl World {
    /// A fresh world in its own temporary directory.
    ///
    /// # Errors
    ///
    /// A filesystem or SQLite failure.
    pub fn new() -> Result<Self> {
        let dir = tempfile::tempdir().map_err(|_| Error::Invalid("temporary directory"))?;
        let ledger_path = dir.path().join("ledger.sqlite");
        let journal_path = dir.path().join("journal.sqlite");
        Ok(Self {
            ledger: Ledger::open(&ledger_path)?,
            journal: Journal::open(&journal_path)?,
            _dir: dir,
            ledger_path,
            journal_path,
            wallet: FakeWallet::new(),
            provider: FakeProvider::new(),
            sandbox: FakeSandbox::new(),
            owner: FakeTaskOwner::new(),
            stopper: FakeStopper::default(),
            artifacts: FakeArtifacts::default(),
            now: START,
            restarts: 0,
        })
    }

    /// Advance the simulated clock.
    pub fn tick(&mut self, seconds: i64) -> i64 {
        self.now += seconds;
        self.now
    }

    /// Crash and restart the service: reopen the ledger and journal from
    /// disk. The fakes outside the process keep their state.
    ///
    /// # Errors
    ///
    /// A SQLite failure.
    pub fn restart(&mut self) -> Result<()> {
        self.ledger = Ledger::open(&self.ledger_path)?;
        self.journal = Journal::open(&self.journal_path)?;
        self.restarts += 1;
        Ok(())
    }

    /// An account with window, workshop, and CLI principals, topped up by
    /// `sats` through the fake wallet.
    ///
    /// # Errors
    ///
    /// A ledger or wallet failure.
    pub fn account(&mut self, account: &str, sats: u64) -> Result<()> {
        self.ledger.create_compute_account(account, self.now)?;
        for (principal, kind) in principals(account) {
            self.ledger.bind_principal(&Binding {
                principal: principal.clone(),
                account: account.into(),
                kind,
                credential: credential_digest(&principal),
                rights: Rights {
                    read: true,
                    spend: true,
                },
                at: self.now,
            })?;
        }
        let cli = format!("cli:{account}");
        let purchase = topup::request_top_up(
            &mut self.ledger,
            &self.wallet,
            &topup::TopUpRequest {
                principal: cli.clone(),
                credential: credential_digest(&cli),
                purchase: format!("buy-{account}"),
                amount_sats: sats,
                now: self.now,
            },
        )?;
        self.wallet.pay_in_full(&purchase.top_up.payment_hash);
        topup::reconcile(&mut self.ledger, &self.wallet, self.now + 1)?;
        Ok(())
    }

    /// Quote and confirm one offer.
    ///
    /// # Errors
    ///
    /// A refusal.
    pub fn confirm(
        &mut self,
        account: &str,
        offer_id: &str,
        request: &TaskRequest,
    ) -> Result<FundedRequest> {
        let book = contract::price_book();
        let capacity = Capacity {
            running: self.provider.active().len(),
            plan_starts_left: Some(10),
        };
        let now = u64::try_from(self.now).unwrap_or(0);
        let made = offer::make_offer(&book, account, offer_id, request, capacity, now)
            .map_err(Error::Refused)?;
        offer::confirm(
            &mut self.journal,
            &made,
            &made.offer.digest,
            ConfirmedVia::OfferControl,
            &book,
            capacity,
            now + 1,
        )
    }

    /// Reserve and provision until ready.
    ///
    /// # Errors
    ///
    /// A refusal, or a sandbox that never becomes ready.
    pub fn ready(&mut self, funded: &FundedRequest) -> Result<String> {
        retail_cloud::reserve::reserve(&mut self.ledger, funded, &rights(funded), self.now)?;
        for _ in 0..8 {
            let now = self.tick(5);
            let record = provision::advance(
                &mut self.journal,
                &self.ledger,
                &self.provider,
                funded,
                &rights(funded),
                TEMPLATE,
                now,
            )?;
            if let ProvisionState::Ready { resource, .. } = record.state {
                return Ok(resource);
            }
        }
        Err(Error::Invalid("the fake sandbox never became ready"))
    }

    /// Deliver the source and the fake customer key.
    ///
    /// # Errors
    ///
    /// A refusal.
    pub fn deliver(&mut self, funded: &FundedRequest, resource: &str) -> Result<()> {
        let now = self.tick(5);
        retail_cloud::material::deliver(
            &mut self.journal,
            &self.sandbox,
            funded,
            &rights(funded),
            resource,
            &funded.admission.source,
            &Credential::ApiKey {
                provider: "openai".into(),
                secret: CustomerSecret::new(FAKE_KEY.into()),
            },
            now,
        )?;
        Ok(())
    }

    /// Dispatch with metering.
    ///
    /// # Errors
    ///
    /// A refusal.
    pub fn dispatch(&mut self, funded: &FundedRequest) -> Result<String> {
        let now = self.tick(5);
        let dispatch = retail_cloud::meter::dispatch_metered(
            &mut self.journal,
            &self.ledger,
            &self.provider,
            &self.owner,
            funded,
            &rights(funded),
            &contract::price_book(),
            now,
        )?;
        Ok(dispatch.task)
    }
}

/// The principals every account binds: the standalone window, the Grid
/// workshop, and the CLI.
#[must_use]
pub fn principals(account: &str) -> Vec<(String, PrincipalKind)> {
    vec![
        (format!("window:{account}"), PrincipalKind::Window),
        (format!("workshop:{account}"), PrincipalKind::Workshop),
        (format!("cli:{account}"), PrincipalKind::Cli),
    ]
}

/// The v1 request the acceptance run uses.
#[must_use]
pub fn request(seconds: u64) -> TaskRequest {
    TaskRequest {
        source: Source {
            repository: "https://github.com/OpenAgentsInc/example".into(),
            commit: "c".repeat(40),
        },
        task: "Make the parser accept trailing commas.".into(),
        checks: vec!["cargo test -p parser".into()],
        max_seconds: seconds,
        ceiling_sats: None,
    }
}

/// Every right the funded request needs.
#[must_use]
pub fn rights(funded: &FundedRequest) -> Current {
    Current {
        observe: Some(ObserveGrant {
            account: funded.account.clone(),
            execution: funded.execution.clone(),
            revoked: false,
        }),
        execute: Some(ExecuteGrant {
            source: GrantSource::Retail,
            execution: funded.execution.clone(),
            generation: funded.admission.grant_generation,
            revoked: false,
        }),
        disclose: Some(DisclosureConsent {
            admission: funded.admission.digest(),
            withdrawn: false,
        }),
        spend: Some(SpendRight {
            account: funded.account.clone(),
        }),
        ..Current::default()
    }
}
