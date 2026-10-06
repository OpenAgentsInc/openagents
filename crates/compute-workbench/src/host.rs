//! Authenticated host reads over the existing ledger and private execution journal.
use crate::{Account, Balance, PaymentState, Quote, Receipt, SCHEMA, TopUp};
use pay_ledger::{
    Ledger,
    compute::{Need, PurchaseState, credential_digest},
};
use retail_cloud::{authority::Current, journal::Journal, offer::RetailOffer};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use workbench::{
    Host,
    pane::{Description, PaneAdapter, PaneKind, PaneState, Subject},
};

/// Re-resolve the principal on every read. A cached pane or world membership grants nothing.
pub fn read(
    ledger: &Ledger,
    journal: &Journal,
    principal: &str,
    credential: &str,
    offers: &[RetailOffer],
    now: i64,
) -> retail_cloud::Result<Account> {
    read_observing(ledger, journal, principal, credential, offers, now, |_| {
        None
    })
}

/// Execution evidence additionally requires current observation authority from its host.
pub fn read_observing(
    ledger: &Ledger,
    journal: &Journal,
    principal: &str,
    credential: &str,
    offers: &[RetailOffer],
    now: i64,
    current: impl Fn(&str) -> Option<Current>,
) -> retail_cloud::Result<Account> {
    let principal = ledger.resolve_principal(principal, credential, Need::Read)?;
    let balance = ledger.compute_balance(&principal.account)?;
    let mut receipts = Vec::new();
    for funded in journal
        .all_funded()?
        .into_iter()
        .filter(|f| f.account == principal.account)
        .take(64)
    {
        let hold = ledger.hold_for_execution(&funded.execution)?;
        let usage = journal.usage(&funded.execution)?;
        let rights = current(&funded.execution);
        let cancellation = match &rights {
            Some(current) => retail_cloud::cancel::observe(journal, ledger, &funded, current, now)?,
            None => None,
        };
        let settlement = match &rights {
            Some(current) => retail_cloud::settle::observe(journal, &funded, current)?,
            None => None,
        };
        let retained = match rights {
            Some(current) => {
                retail_cloud::authority::check(
                    retail_cloud::authority::Step::Read,
                    &funded.admission,
                    &current,
                )
                .map_err(retail_cloud::Error::Denied)?;
                journal.retention_receipt(&funded.execution, now)?
            }
            None => None,
        };
        let check_failed = cancellation
            .as_ref()
            .and_then(|c| c.executor.as_ref())
            .and_then(|e| match &e.status {
                retail_cloud::dispatch::TaskStatus::Ended {
                    patch: Some(patch),
                    checks,
                    ..
                } => Some(
                    retail_cloud::dispatch::verdict(Some(patch), &funded.task.checks, checks)
                        != retail_cloud::dispatch::Verdict::Verified,
                ),
                _ => None,
            });
        let check_failed = settlement
            .as_ref()
            .and_then(|s| s.checks.as_ref())
            .and_then(|v| match v {
                retail_cloud::dispatch::Verdict::Verified => Some(false),
                retail_cloud::dispatch::Verdict::CheckFailed => Some(true),
                retail_cloud::dispatch::Verdict::Unchecked => None,
            })
            .or(check_failed);
        let charged = hold.as_ref().and_then(|h| h.charge_msat);
        receipts.push(Receipt {
            usage_digest: settlement.as_ref().and_then(|s| s.usage_digest.clone()),
            settled_at: hold.as_ref().and_then(|h| h.settled_at),
            account: funded.account.clone(),
            settlement_resource: pay_ledger::compute::hold::RETAIL_RESOURCE.into(),
            execution: funded.execution.clone(),
            request: funded.request.clone(),
            offer: funded.offer.clone(),
            quote: route_contract::digest_of(&funded.quote).as_str().into(),
            source_repository: funded.admission.source.repository.clone(),
            source_commit: funded.admission.source.commit.clone(),
            computer: funded.admission.computer_class.clone(),
            executor: retained
                .as_ref()
                .and_then(|r| r.manifest.as_ref())
                .map(|m| m.engine.clone()),
            model_payer: format!("{:?}", funded.admission.model_payer),
            verification: match check_failed {
                Some(true) => "Retained declared checks failed",
                Some(false) => "Retained declared checks on exact patch",
                None => "Financial projection; execution checks unverified",
            }
            .into(),
            quoted_msat: i64::try_from(
                funded
                    .quote
                    .max_sats
                    .checked_mul(1000)
                    .ok_or(retail_cloud::Error::Invalid("quote amount"))?,
            )
            .map_err(|_| retail_cloud::Error::Invalid("quote amount"))?,
            reserved_msat: hold.as_ref().map_or(0, |h| h.request.amount_msat),
            settled_msat: charged,
            released_msat: hold.as_ref().and_then(|h| h.released_msat()),
            usage_seconds: usage.as_ref().and_then(|u| u.seconds),
            cost_unknown: charged.is_none(),
            cancellation_requested: cancellation.is_some()
                || usage.as_ref().is_some_and(|u| u.stop_requested),
            cancellation_acknowledged: cancellation.as_ref().is_some_and(|c| c.executor.is_some())
                || usage.as_ref().is_some_and(|u| u.stop_acknowledged),
            check_failed,
            settlement_source: charged
                .filter(|charge| *charge > 0)
                .map(|_| format!("debit:{}", funded.request)),
        });
    }
    let quotes = offers
        .iter()
        .filter(|o| {
            o.admission.account == principal.account && o.quote.max_sats <= (i64::MAX / 1000) as u64
        })
        .take(32)
        .map(|o| {
            let current = retail_cloud::offer::make_offer(
                &retail_cloud::contract::price_book(),
                &principal.account,
                &o.offer.id,
                &o.request,
                retail_cloud::offer::Capacity {
                    running: 0,
                    plan_starts_left: None,
                },
                o.offer.created_at,
            )
            .ok();
            Quote {
                offer: o.offer.id.clone(),
                digest: o.offer.digest.as_str().into(),
                book: o.quote.version.clone(),
                expires_at: o.offer.expires_at,
                maximum_msat: i64::try_from(o.quote.max_sats)
                    .ok()
                    .and_then(|s| s.checked_mul(1000))
                    .unwrap_or(i64::MAX),
                confirmable: now >= 0
                    && (now as u64) >= o.offer.created_at
                    && (now as u64) < o.offer.expires_at
                    && o.offer.intact()
                    && current.as_ref() == Some(o),
            }
        })
        .collect();
    let topups = ledger
        .top_ups(&principal.account)?
        .into_iter()
        .take(64)
        .map(|p| TopUp {
            purchase: p.top_up.id,
            payment_hash: p.top_up.payment_hash,
            amount_msat: p.top_up.amount_msat,
            expires_at: p.top_up.expires_at,
            state: match p.state {
                PurchaseState::Pending if now >= p.top_up.expires_at => PaymentState::Expired,
                PurchaseState::Pending => PaymentState::Pending,
                PurchaseState::Paid => PaymentState::Paid,
                PurchaseState::Expired => PaymentState::Expired,
                PurchaseState::Unknown => PaymentState::Unknown,
            },
        })
        .collect();
    Ok(Account {
        schema: SCHEMA.into(),
        account: principal.account,
        observed_at: now,
        balance: Balance {
            credited_msat: balance.credited_msat,
            available_msat: balance.available_msat,
            held_msat: balance.held_msat,
            settled_msat: balance.settled_msat,
            released_msat: balance.released_msat,
        },
        topups,
        quotes,
        receipts,
    })
}

/// Private host configuration. Credentials remain in this file, never process arguments.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub ledger: PathBuf,
    pub journal: PathBuf,
    pub principal: String,
    pub credential: String,
    #[serde(default)]
    pub offers: Vec<RetailOffer>,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        private_file(path)?;
        let metadata =
            std::fs::metadata(path).map_err(|_| "cannot inspect compute configuration")?;
        if metadata.len() > 256 * 1024 {
            return Err("compute configuration is too large".into());
        }
        let bytes = std::fs::read(path).map_err(|_| "cannot read compute configuration")?;
        serde_json::from_slice(&bytes).map_err(|_| "invalid compute configuration".into())
    }
    pub fn read(&self) -> Result<Account, String> {
        private_file(&self.ledger)?;
        private_file(&self.journal)?;
        let ledger = Ledger::open(&self.ledger).map_err(|_| "cannot read compute ledger")?;
        let journal = Journal::open(&self.journal).map_err(|_| "cannot read compute journal")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "invalid clock")?
            .as_secs() as i64;
        read(
            &ledger,
            &journal,
            &self.principal,
            &self.credential,
            &self.offers,
            now,
        )
        .map_err(|_| "compute account read denied or unavailable".into())
    }
}
fn private_file(path: &Path) -> Result<(), String> {
    let m = std::fs::symlink_metadata(path).map_err(|_| "compute file does not exist")?;
    if !m.is_file() || m.file_type().is_symlink() {
        return Err("compute file must be a regular private file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if m.permissions().mode() & 0o077 != 0 {
            return Err("compute file must exclude group and other access".into());
        }
    }
    Ok(())
}
struct Adapter {
    config: Config,
    host: Host,
    kind: PaneKind,
}
impl PaneAdapter for Adapter {
    fn kind(&self) -> PaneKind {
        self.kind
    }
    fn describe(&self, subject: &Subject) -> Description {
        if subject.host() != &self.host {
            return Description::only(PaneState::Missing, "Compute account");
        }
        let Ok(account) = self.config.read() else {
            return Description::only(PaneState::Revoked, "Compute read denied or unavailable");
        };
        let (title, detail) = if self.kind == PaneKind::Account && subject.id() == account.account {
            (
                format!("Compute account {}", account.account),
                account.lines(),
            )
        } else if self.kind == PaneKind::Receipt {
            let Some(receipt) = account
                .receipts
                .iter()
                .find(|r| r.execution == subject.id())
            else {
                return Description::only(PaneState::Missing, "Compute receipt");
            };
            (
                format!("Compute receipt {}", receipt.execution),
                receipt.lines(),
            )
        } else {
            return Description::only(PaneState::Missing, "Compute record");
        };
        let mut detail = detail
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect::<String>();
        while detail.len() > 2048 {
            detail.pop();
        }
        if subject.revision().is_some() {
            return Description::only(
                PaneState::Stale { current: None },
                "Compute refresh required",
            );
        }
        Description {
            state: PaneState::Ready,
            title,
            detail,
            actions: Vec::new(),
        }
    }
}
/// Both native window and Grid mount these exact read-only account and receipt adapters.
pub fn mount(application: &mut terminal_core::Application, config: Config) -> Result<(), String> {
    mount_products(&mut application.products, config)?;
    application.paper.on = true;
    Ok(())
}

/// Register and open the production panes without constructing a shell or window.
pub fn mount_products(
    products: &mut terminal_core::resources::Products,
    config: Config,
) -> Result<(), String> {
    let account = config.read()?;
    let host = Host::Local {
        instance: credential_digest(&format!("compute-account:{}", account.account)),
    };
    products.panes = std::mem::take(&mut products.panes)
        .adapter(Box::new(Adapter {
            config: config.clone(),
            host: host.clone(),
            kind: PaneKind::Account,
        }))
        .adapter(Box::new(Adapter {
            config,
            host: host.clone(),
            kind: PaneKind::Receipt,
        }));
    for receipt in account.receipts.iter().take(16) {
        products.open(
            PaneKind::Receipt,
            &Subject::Record {
                host: host.clone(),
                id: receipt.execution.clone(),
                revision: None,
            },
        )?;
    }
    products.open(
        PaneKind::Account,
        &Subject::Record {
            host,
            id: account.account,
            revision: None,
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "../../retail-cloud/tests/common/mod.rs"]
mod common;
#[cfg(test)]
mod tests {
    use super::*;
    use pay_ledger::compute::{Binding, PrincipalKind, Rights, credential_digest};
    #[test]
    fn native_grid_reconnect_and_revocation_resolve_existing_money_only() {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut journal = Journal::in_memory().unwrap();
        common::funded_account(&mut ledger, "acct", 1000);
        for (id, kind) in [
            ("window", PrincipalKind::Window),
            ("grid", PrincipalKind::Workshop),
        ] {
            ledger
                .bind_principal(&Binding {
                    principal: id.into(),
                    account: "acct".into(),
                    kind,
                    credential: credential_digest(id),
                    rights: Rights {
                        read: true,
                        spend: false,
                    },
                    at: common::NOW,
                })
                .unwrap();
        }
        let funded = common::confirmed(&mut journal, "acct", "offer", &common::request(60));
        retail_cloud::reserve::reserve(&mut ledger, &funded, &common::rights(&funded), common::NOW)
            .unwrap();
        let window = read(
            &ledger,
            &journal,
            "window",
            &credential_digest("window"),
            &[],
            common::NOW,
        )
        .unwrap();
        let grid = read(
            &ledger,
            &journal,
            "grid",
            &credential_digest("grid"),
            &[],
            common::NOW,
        )
        .unwrap();
        assert_eq!(window, grid);
        assert_eq!(window.receipts[0].offer, "offer");
        assert!(window.balance.held_msat > 0);
        assert_eq!(window.receipts[0].settled_msat, None);
        assert_eq!(window.receipts[0].check_failed, None);
        let original = ledger.compute_balance("acct").unwrap();
        assert_eq!(
            window,
            read(
                &ledger,
                &journal,
                "window",
                &credential_digest("window"),
                &[],
                common::NOW
            )
            .unwrap()
        );
        assert_eq!(original, ledger.compute_balance("acct").unwrap());
        ledger.mark_hold_unknown(&funded.request).unwrap();
        assert!(
            read(
                &ledger,
                &journal,
                "grid",
                &credential_digest("grid"),
                &[],
                common::NOW
            )
            .unwrap()
            .receipts[0]
                .cost_unknown
        );
        let (settled, _) = ledger
            .settle_hold(&funded.request, 1000, common::NOW + 1)
            .unwrap();
        let after = read(
            &ledger,
            &journal,
            "grid",
            &credential_digest("grid"),
            &[],
            common::NOW + 1,
        )
        .unwrap();
        assert_eq!(after.receipts[0].settled_msat, Some(1000));
        assert_eq!(after.receipts[0].released_msat, settled.released_msat());
        assert_eq!(
            after.receipts[0].settlement_source,
            Some(format!("debit:{}", funded.request))
        );
        assert!(!after.receipts[0].cost_unknown);
        ledger.revoke_principal("window", common::NOW + 1).unwrap();
        assert!(
            read(
                &ledger,
                &journal,
                "window",
                &credential_digest("window"),
                &[],
                common::NOW
            )
            .is_err()
        );
        assert!(read(&ledger, &journal, "grid", "wrong", &[], common::NOW).is_err());
        let mut rights = common::rights(&funded);
        rights.observe.as_mut().unwrap().revoked = true;
        assert!(
            read_observing(
                &ledger,
                &journal,
                "grid",
                &credential_digest("grid"),
                &[],
                common::NOW,
                |_| Some(rights.clone())
            )
            .is_err()
        );
    }
    #[test]
    fn quotes_refuse_expired_changed_terms_and_foreign_accounts_without_holds() {
        let mut ledger = Ledger::in_memory().unwrap();
        let journal = Journal::in_memory().unwrap();
        common::funded_account(&mut ledger, "a", 1000);
        common::funded_account(&mut ledger, "b", 1000);
        let offer = retail_cloud::offer::make_offer(
            &retail_cloud::contract::price_book(),
            "a",
            "quote",
            &common::request(60),
            common::FREE,
            common::NOW as u64,
        )
        .unwrap();
        let snapshot = |offer: &RetailOffer, now| {
            read(
                &ledger,
                &journal,
                "cli:a",
                &credential_digest("a"),
                std::slice::from_ref(offer),
                now,
            )
            .unwrap()
        };
        assert!(snapshot(&offer, common::NOW).quotes[0].confirmable);
        assert!(!snapshot(&offer, offer.offer.expires_at as i64).quotes[0].confirmable);
        let mut changed = offer.clone();
        changed.admission.source.commit = "d".repeat(40);
        assert!(!snapshot(&changed, common::NOW).quotes[0].confirmable);
        assert!(
            read(
                &ledger,
                &journal,
                "cli:b",
                &credential_digest("b"),
                &[offer],
                common::NOW
            )
            .unwrap()
            .quotes
            .is_empty()
        );
        assert_eq!(ledger.compute_balance("a").unwrap().held_msat, 0);
    }
    #[test]
    fn pending_invoice_is_explicit_and_does_not_increase_balance() {
        let mut ledger = Ledger::in_memory().unwrap();
        let journal = Journal::in_memory().unwrap();
        common::funded_account(&mut ledger, "acct", 0);
        let wallet = retail_cloud::fake::FakeWallet::new();
        let p = retail_cloud::topup::request_top_up(
            &mut ledger,
            &wallet,
            &retail_cloud::topup::TopUpRequest {
                principal: "cli:acct".into(),
                credential: credential_digest("acct"),
                purchase: "pending".into(),
                amount_sats: 100,
                now: common::NOW,
            },
        )
        .unwrap();
        let pending = read(
            &ledger,
            &journal,
            "cli:acct",
            &credential_digest("acct"),
            &[],
            common::NOW,
        )
        .unwrap();
        assert_eq!(pending.topups[0].state, PaymentState::Pending);
        assert_eq!(pending.balance.available_msat, 0);
        let expired = read(
            &ledger,
            &journal,
            "cli:acct",
            &credential_digest("acct"),
            &[],
            p.top_up.expires_at,
        )
        .unwrap();
        assert_eq!(expired.topups[0].state, PaymentState::Expired);
        assert_eq!(expired.balance.available_msat, 0);
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use retail_cloud::{
        cancel::{StopEvidence, StopOwner},
        dispatch::{CheckRun, ExecutorEnd, OwnerError, TaskStatus},
        retain::{Artifacts, Manifest},
    };
    struct Stopped;
    impl StopOwner for Stopped {
        fn stop(&self, _: &str, _: &str, _: &str) -> Result<StopEvidence, OwnerError> {
            self.stopped("", "", "").map(|e| e.unwrap())
        }
        fn stopped(&self, _: &str, _: &str, _: &str) -> Result<Option<StopEvidence>, OwnerError> {
            Ok(Some(StopEvidence {
                at: common::NOW + 30,
                started: true,
                status: TaskStatus::Ended {
                    end: ExecutorEnd::Failed,
                    patch: Some("a".repeat(64)),
                    checks: vec![CheckRun {
                        command: "cargo test -p parser".into(),
                        candidate: "a".repeat(64),
                        exit_status: 1,
                    }],
                },
                effects: Vec::new(),
            }))
        }
    }
    struct NoArtifacts;
    impl Artifacts for NoArtifacts {
        fn manifest(&self, _: &str, _: &str) -> retail_cloud::Result<Manifest> {
            Err(retail_cloud::Error::Invalid("no retained artifact fixture"))
        }
        fn read(&self, _: &str, _: &str, _: &str, _: usize) -> retail_cloud::Result<Vec<u8>> {
            Err(retail_cloud::Error::Invalid("no artifact fixture"))
        }
    }
    #[test]
    fn failed_check_and_acknowledged_cancellation_require_execution_observation() {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut journal = Journal::in_memory().unwrap();
        common::funded_account(&mut ledger, "acct", 1000);
        let funded = common::confirmed(&mut journal, "acct", "offer", &common::request(600));
        let provider = retail_cloud::fake::FakeProvider::new();
        let resource = common::delivered(
            &mut journal,
            &mut ledger,
            &provider,
            &retail_cloud::fake::FakeSandbox::new(),
            &funded,
        );
        provider.set_usage(&resource, 45);
        retail_cloud::meter::dispatch_metered(
            &mut journal,
            &mut ledger,
            &provider,
            &retail_cloud::fake::FakeTaskOwner::new(),
            &funded,
            &common::rights(&funded),
            &retail_cloud::contract::price_book(),
            common::NOW + 20,
        )
        .unwrap();
        provider.set_usage(&resource, 55);
        retail_cloud::cancel::request(
            &mut journal,
            &funded,
            &common::rights(&funded),
            common::NOW + 21,
        )
        .unwrap();
        for t in 0..4 {
            retail_cloud::cancel::advance(
                &mut journal,
                &ledger,
                &provider,
                &Stopped,
                &NoArtifacts,
                &funded.execution,
                common::NOW + 22 + t,
            )
            .unwrap();
        }
        let projected = read_observing(
            &ledger,
            &journal,
            "cli:acct",
            &credential_digest("acct"),
            &[],
            common::NOW + 30,
            |_| Some(common::rights(&funded)),
        )
        .unwrap();
        assert!(projected.receipts[0].cancellation_acknowledged);
        assert_eq!(projected.receipts[0].check_failed, Some(true));
        assert!(projected.receipts[0].lines().contains("failed check"));
        let financial = read(
            &ledger,
            &journal,
            "cli:acct",
            &credential_digest("acct"),
            &[],
            common::NOW + 30,
        )
        .unwrap();
        assert_eq!(financial.receipts[0].check_failed, None);
    }
}

#[cfg(all(test, unix))]
mod pane_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn both_product_mounts_render_same_private_records_and_refresh_denies_revocation() {
        let dir = tempfile::tempdir().unwrap();
        let ledger_path = dir.path().join("ledger.sqlite");
        let journal_path = dir.path().join("journal.sqlite");
        let mut ledger = Ledger::open(&ledger_path).unwrap();
        let mut journal = Journal::open(&journal_path).unwrap();
        common::funded_account(&mut ledger, "acct", 1000);
        let funded = common::confirmed(&mut journal, "acct", "offer", &common::request(60));
        retail_cloud::reserve::reserve(&mut ledger, &funded, &common::rights(&funded), common::NOW)
            .unwrap();
        std::fs::set_permissions(&ledger_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config = Config {
            ledger: ledger_path,
            journal: journal_path,
            principal: "cli:acct".into(),
            credential: credential_digest("acct"),
            offers: Vec::new(),
        };
        let host = Host::Local {
            instance: credential_digest("compute-account:acct"),
        };
        let subject = Subject::Record {
            host: host.clone(),
            id: funded.execution,
            revision: None,
        };
        let make = || {
            let mut products = terminal_core::resources::Products::default();
            products.panes = std::mem::take(&mut products.panes).adapter(Box::new(Adapter {
                config: config.clone(),
                host: host.clone(),
                kind: PaneKind::Receipt,
            }));
            products.open(PaneKind::Receipt, &subject).unwrap();
            products
        };
        let mut native = make();
        let grid = make();
        assert_eq!(native.open, grid.open);
        assert!(native.open[0].detail.contains("Reserved"));
        assert!(native.open[0].actions.is_empty());
        ledger
            .revoke_principal("cli:acct", common::NOW + 1)
            .unwrap();
        native.refresh();
        assert_eq!(native.open[0].state, PaneState::Revoked);
        assert!(native.open[0].detail.is_empty());
    }
}

#[cfg(test)]
mod settlement_tests {
    use super::*;
    #[test]
    fn original_settlement_releases_unused_hold_without_a_refund_or_zero_debit() {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut journal = Journal::in_memory().unwrap();
        common::funded_account(&mut ledger, "acct", 1000);
        let funded = common::confirmed(&mut journal, "acct", "offer", &common::request(60));
        retail_cloud::reserve::reserve(&mut ledger, &funded, &common::rights(&funded), common::NOW)
            .unwrap();
        retail_cloud::settle::settle(
            &mut journal,
            &mut ledger,
            &funded,
            route_contract::price_book::Ending::NotStarted,
            common::NOW + 1,
        )
        .unwrap();
        let view = read_observing(
            &ledger,
            &journal,
            "cli:acct",
            &credential_digest("acct"),
            &[],
            common::NOW + 1,
            |_| Some(common::rights(&funded)),
        )
        .unwrap();
        assert_eq!(view.receipts[0].settled_msat, Some(0));
        assert_eq!(
            view.receipts[0].released_msat,
            Some(view.receipts[0].reserved_msat)
        );
        assert_eq!(view.receipts[0].settlement_source, None);
        assert_eq!(view.receipts[0].settled_at, Some(common::NOW + 1));
        assert_eq!(view.balance.available_msat, 1_000_000);
    }
}
