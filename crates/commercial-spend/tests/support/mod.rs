//! Isolated native stores and a real Unix resident protocol. No owner state or funds.
use commercial_accounts::{Entry, NativeSources, NativeStore, Policy};
use commercial_spend::{Config, Controller, Grant, Native, digest, now};
use openagents_wallet::{
    Balance, Channel, IssuedInvoice, LightningWallet, PaymentDirection, PaymentRecord,
    PaymentStatus, Proof, WalletError,
    resident::{Served, Server},
};
use pay_ledger::{
    Ledger,
    compute::{Binding as PrincipalBinding, PrincipalKind, Rights, credential_digest},
    shared::{Binding, Client, ClientConfig, Operation},
};
use receipts::{
    funding_units::{Conversion, FeePayer, Rounding, Unit},
    purchase::{CommercialProduct, CommercialRef, CommercialSource},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tenancy::{
    Accounts, Manifest, Registry, Tenant, WorkspaceKind,
    accounts::commercial::{Product, Source},
};
pub fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
pub fn mkdir(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}
pub fn signed_invoice(
    secret: [u8; 32],
    amount: u64,
    hash: [u8; 32],
    request: [u8; 32],
    expiry: u32,
) -> String {
    use nostr::x402::test_invoice::{number, signed_by, tag, words};
    let mut fields = tag(1, &words(&hash));
    fields.extend(tag(23, &words(&request)));
    fields.extend(tag(16, &words(&[4; 32])));
    fields.extend(tag(6, &number(expiry.into())));
    signed_by(
        secret,
        &format!("lnbc{}p", amount.checked_mul(10).unwrap()),
        fields,
        false,
        false,
        now(),
    )
}
type PaymentHook = Arc<dyn Fn(&str, u64) -> Result<Proof, WalletError> + Send + Sync>;
#[derive(Default)]
pub struct FakeWallet {
    payment_hook: Mutex<Option<PaymentHook>>,
    pub records: Mutex<BTreeMap<String, PaymentRecord>>,
    pub outgoing: AtomicU64,
    pub incoming: AtomicU64,
    pub unknown: AtomicBool,
    pub pause: AtomicBool,
    pub release: AtomicBool,
}
impl FakeWallet {
    pub fn set_payment_hook(&self, hook: PaymentHook) {
        *self.payment_hook.lock().unwrap() = Some(hook);
    }
    pub fn node() -> String {
        let invoice = signed_invoice([19; 32], 1000, [3; 32], [4; 32], 900);
        hex::encode(nostr::x402::decode_invoice(&invoice).unwrap().payee())
    }
}
impl LightningWallet for FakeWallet {
    fn node_id(&self) -> String {
        Self::node()
    }
    fn receive_exact(
        &self,
        amount: u64,
        request: [u8; 32],
        expiry: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        let nonce = self.incoming.fetch_add(1, Ordering::SeqCst) + 1;
        let hash =
            sha2::Sha256::digest(format!("{nonce}:{}:{amount}", hex::encode(request)).as_bytes());
        let mut bytes = [0; 32];
        bytes.copy_from_slice(&hash);
        let bolt11 = signed_invoice([19; 32], amount, bytes, request, expiry);
        let payment_hash = hex::encode(bytes);
        self.records.lock().unwrap().insert(
            payment_hash.clone(),
            PaymentRecord {
                payment_hash: payment_hash.clone(),
                direction: PaymentDirection::Inbound,
                status: PaymentStatus::Succeeded,
                amount_msat: Some(amount),
                fee_msat: None,
                preimage: None,
                bolt11: None,
                updated_at: now(),
            },
        );
        Ok(IssuedInvoice {
            bolt11,
            payment_hash,
            amount_msat: amount,
            description_hash: hex::encode(request),
            expiry_secs: expiry,
            pay_to: Self::node(),
        })
    }
    fn pay(&self, bolt11: &str, max_fee: u64, _: Duration) -> Result<Proof, WalletError> {
        let parsed = nostr::x402::decode_invoice(bolt11)
            .map_err(|_| WalletError::Invalid("Fixture invoice invalid.".into()))?;
        let hash = hex::encode(parsed.payment_hash());
        let hook = self.payment_hook.lock().unwrap().clone();
        let proof = if let Some(hook) = hook {
            hook(bolt11, max_fee)?
        } else {
            let preimage = [[7; 32], [5; 32], [4; 32]]
                .into_iter()
                .find(|p| digest(p) == hash)
                .ok_or_else(|| WalletError::Node("Fixture preimage unknown.".into()))?;
            Proof {
                payment_hash: hash.clone(),
                preimage: hex::encode(preimage),
                amount_msat: parsed.amount_msat(),
                fee_msat: 3.min(max_fee),
                bolt11: bolt11.into(),
            }
        };
        let preimage = hex::decode(&proof.preimage).unwrap();
        assert_eq!(digest(&preimage), hash);
        assert_eq!(proof.payment_hash, hash);
        assert_eq!(proof.bolt11, bolt11);
        assert_eq!(proof.amount_msat, parsed.amount_msat());
        assert!(proof.fee_msat <= max_fee);
        let fee = proof.fee_msat;
        self.outgoing.fetch_add(1, Ordering::SeqCst);
        self.records.lock().unwrap().insert(
            hash.clone(),
            PaymentRecord {
                payment_hash: hash.clone(),
                direction: PaymentDirection::Outbound,
                status: PaymentStatus::Succeeded,
                amount_msat: Some(parsed.amount_msat()),
                fee_msat: Some(fee),
                preimage: Some(proof.preimage.clone()),
                bolt11: Some(bolt11.into()),
                updated_at: now(),
            },
        );
        while self.pause.load(Ordering::SeqCst) && !self.release.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.unknown.load(Ordering::SeqCst) {
            return Err(WalletError::Pending {
                payment_hash: hash,
                waited_secs: 1,
            });
        }
        Ok(proof)
    }
    fn lookup(&self, hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .get(&hex::encode(hash))
            .cloned())
    }
    fn payments(&self) -> Result<Vec<PaymentRecord>, WalletError> {
        Ok(self.records.lock().unwrap().values().cloned().collect())
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        Ok(Balance {
            onchain_total_sats: 0,
            onchain_spendable_sats: 0,
            lightning_total_sats: 0,
            anchor_reserve_sats: 0,
        })
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        Ok(vec![])
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        Err(WalletError::Node("No fixture funding address.".into()))
    }
    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        Err(WalletError::Node("No fixture channel authority.".into()))
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        Err(WalletError::Node("No fixture channel authority.".into()))
    }
}
impl Served for FakeWallet {
    fn status(&self) -> Value {
        json!({"running":true,"network":"bitcoin","node_id":Self::node()})
    }
    fn buy_channel(&self, _: u64, _: u64, _: u32, _: bool) -> Result<Value, WalletError> {
        Err(WalletError::Node("No fixture channel authority.".into()))
    }
    fn channel_order(&self, _: &str) -> Result<Value, WalletError> {
        Err(WalletError::Node("No fixture channel authority.".into()))
    }
    fn send_onchain(&self, _: &str, _: u64) -> Result<String, WalletError> {
        Err(WalletError::Node("No fixture send authority.".into()))
    }
}
use sha2::Digest;
pub struct NativeInput {
    pub directory: PathBuf,
    pub account: String,
    pub workspace: String,
    pub tenant: String,
    pub token: String,
    pub money: PathBuf,
}
pub struct Fixture {
    pub root: tempfile::TempDir,
    pub native: NativeInput,
    pub canonical: PathBuf,
    pub ledger: PathBuf,
    pub config: PathBuf,
    pub wallet_home: PathBuf,
    pub buyer_root: PathBuf,
    pub fake: Arc<FakeWallet>,
    pub gateway: Client,
    pub retail: Client,
    pub plugin: Client,
    pub gateway_binding: Binding,
    pub retail_binding: Binding,
    pub plugin_binding: Binding,
    pub controller: Option<Arc<Controller>>,
    controller_stop: Arc<AtomicBool>,
    wallet_stop: Arc<AtomicBool>,
    controller_thread: Option<std::thread::JoinHandle<()>>,
    wallet_thread: Option<std::thread::JoinHandle<()>>,
}
impl Fixture {
    pub fn new(input: Option<NativeInput>) -> Self {
        Self::new_scoped(input, None)
    }
    pub fn new_scoped(input: Option<NativeInput>, scopes: Option<tenancy::keys::Scopes>) -> Self {
        let temporary = std::fs::canonicalize("/tmp").unwrap();
        let root = tempfile::Builder::new()
            .prefix("sp")
            .tempdir_in(temporary)
            .unwrap();
        mkdir(root.path());
        let canonical = root.path().join("canonical");
        mkdir(&canonical);
        let canonical_accounts = Accounts::install(&canonical).unwrap();
        let owner = canonical_accounts
            .create_account("Canonical customer", &["key:aaaaaaaaaaaaaaaa".into()])
            .unwrap();
        let workspace = canonical_accounts
            .create_workspace(
                &owner.id,
                "Canonical personal",
                WorkspaceKind::Personal,
                "fixture",
                None,
            )
            .unwrap();
        let native = input.unwrap_or_else(|| {
            let directory = root.path().join("native");
            mkdir(&directory);
            let registry = Registry::install(
                &directory,
                Manifest {
                    v: tenancy::SCHEMA.into(),
                    sequence: 0,
                    supersedes: None,
                    shared: BTreeMap::new(),
                    tenants: BTreeMap::from([(
                        "fixture".into(),
                        Tenant {
                            credential: "key-ref:fixture".into(),
                            principals: vec![],
                            doors: BTreeMap::new(),
                            quota: None,
                        },
                    )]),
                    digest: String::new(),
                },
            )
            .unwrap();
            let key = tenancy::keys::issue_scoped(
                &directory,
                registry.manifest(),
                "fixture",
                None,
                scopes,
            )
            .unwrap();
            let accounts = Accounts::install(&directory).unwrap();
            let account = accounts
                .create_account("Native customer", &[format!("key:{}", key.key.id)])
                .unwrap();
            let workspace = accounts
                .create_workspace(
                    &account.id,
                    "Native personal",
                    WorkspaceKind::Personal,
                    "fixture",
                    None,
                )
                .unwrap();
            let money = directory.join("money.jsonl");
            let mut journal = tenancy::money::Ledger::open(&money).unwrap();
            journal
                .apply(tenancy::money::Mutation {
                    workspace: workspace.id.clone(),
                    source: "fixture:create".into(),
                    audit: "Synthetic native account".into(),
                    operation: tenancy::money::Operation::Create {
                        currency: "USD".into(),
                        spend_limit: 1_000_000,
                        topups_allowed: false,
                    },
                })
                .unwrap();
            drop(journal);
            NativeInput {
                directory,
                account: account.id,
                workspace: workspace.id,
                tenant: "fixture".into(),
                token: key.token,
                money,
            }
        });
        mkdir(&native.directory);
        std::fs::set_permissions(
            native.directory.join("keys.json"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let registry = Registry::open(&native.directory).unwrap();
        let auth =
            tenancy::keys::authenticate(&native.directory, registry.manifest(), &native.token)
                .unwrap();
        let principal = format!("key:{}", auth.key_id);
        let native_accounts = Accounts::open(&native.directory).unwrap();
        let member = native_accounts
            .authenticate_key(registry.manifest(), &native.workspace, &native.token)
            .unwrap();
        assert_eq!(member.account, native.account);
        let native_credential = root.path().join("native.credential");
        write(&native_credential, native.token.as_bytes());
        let retail_credential = root.path().join("retail.credential");
        write(&retail_credential, b"synthetic-retail");
        let ledger = root.path().join("ledger.sqlite");
        let mut book = Ledger::open(&ledger).unwrap();
        std::fs::set_permissions(&ledger, std::fs::Permissions::from_mode(0o600)).unwrap();
        book.create_compute_account("retail", now() as i64).unwrap();
        book.bind_principal(&PrincipalBinding {
            principal: "cli:retail".into(),
            account: "retail".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("synthetic-retail"),
            rights: Rights {
                read: true,
                spend: true,
            },
            at: now() as i64,
        })
        .unwrap();
        let origin = book.origin().unwrap();
        drop(book);
        let sources = [
            Source {
                product: Product::Gateway,
                issuer: "native".into(),
                account: native.account.clone(),
                workspace: Some(native.workspace.clone()),
            },
            Source {
                product: Product::Plugin,
                issuer: "native".into(),
                account: native.account.clone(),
                workspace: Some(native.workspace.clone()),
            },
            Source {
                product: Product::Retail,
                issuer: "retail".into(),
                account: "retail".into(),
                workspace: None,
            },
        ];
        let canonical_owner = canonical_accounts
            .authorize(&workspace.id, &owner.id)
            .unwrap();
        let entries = sources
            .iter()
            .map(|source| Entry {
                operator: "synthetic-operator".into(),
                source: source.clone(),
                customer: owner.id.clone(),
                workspace: workspace.id.clone(),
                canonical_owner: owner.id.clone(),
                canonical_owner_epoch: canonical_owner.epoch,
                canonical_members_epoch: canonical_owner.members_epoch,
                principal: if source.product == Product::Retail {
                    "cli:retail".into()
                } else {
                    principal.clone()
                },
                credential_file: if source.product == Product::Retail {
                    retail_credential.clone()
                } else {
                    native_credential.clone()
                },
                generation: 1,
                native_owner: (source.product != Product::Retail).then(|| native.account.clone()),
                native_owner_epoch: (source.product != Product::Retail).then_some(member.epoch),
                native_members_epoch: (source.product != Product::Retail)
                    .then_some(member.members_epoch),
                reviewed_at: now(),
                valid_until: now() + 3600,
                previous_authority: None,
            })
            .collect();
        let policy = root.path().join("mapping.json");
        write(
            &policy,
            &serde_json::to_vec(&Policy {
                schema: commercial_accounts::SCHEMA.into(),
                operator: "synthetic-operator".into(),
                entries,
            })
            .unwrap(),
        );
        let commercial = commercial_accounts::Config {
            policy,
            stores: vec![
                NativeStore::Tenancy {
                    issuer: "native".into(),
                    directory: native.directory.clone(),
                },
                NativeStore::Retail {
                    issuer: "retail".into(),
                    ledger: ledger.clone(),
                },
            ],
        };
        let adapter = NativeSources::open(&canonical, &commercial).unwrap();
        let revision = canonical_accounts
            .review_commercial(
                "canonical-review",
                &owner.id,
                &workspace.id,
                &owner.id,
                None,
                &sources,
                &adapter,
            )
            .unwrap();
        canonical_accounts
            .admit_commercial(&revision, &revision.digest, &adapter)
            .unwrap();
        drop(adapter);
        let wallet_home = root.path().join("wallet");
        mkdir(&wallet_home);
        let buyer_root = root.path().join("buyer");
        mkdir(&buyer_root);
        let server = Server::bind(&wallet_home).unwrap();
        let wallet_stop = server.stop_flag();
        let fake = Arc::new(FakeWallet::default());
        let served = fake.clone();
        let wallet_thread = std::thread::spawn(move || server.run(served));
        let socket = root.path().join("c.sock");
        let writer_file = root.path().join("writer");
        write(&writer_file, "ab".repeat(32).as_bytes());
        let mut clients = vec![];
        let mut bindings = vec![];
        let mut grants = vec![];
        for (index, source) in sources.iter().enumerate() {
            let product = match source.product {
                Product::Gateway => CommercialProduct::Gateway,
                Product::Plugin => CommercialProduct::Plugin,
                Product::Retail => CommercialProduct::Retail,
            };
            let portable = CommercialSource {
                product,
                issuer: source.issuer.clone(),
                account: source.account.clone(),
                workspace: source.workspace.clone(),
            };
            let conversion = Conversion {
                version: format!("synthetic-unit-review-{index}"),
                source: if product == CommercialProduct::Gateway {
                    Unit::CurrencyMillionths {
                        currency: "USD".into(),
                    }
                } else {
                    Unit::Millisatoshis
                },
                target: Unit::Millisatoshis,
                numerator: 1,
                denominator: 1,
                source_ref: "Explicit synthetic conversion; no market rate or owner qualification."
                    .into(),
                valid_from: now() - 1,
                valid_until: now() + 3600,
                rounding: Rounding::Exact,
                fee_payer: FeePayer::Operator,
                max_fee_units: 0,
            };
            let binding = Binding {
                id: format!("source-{index}"),
                source: portable.clone(),
                native_origin: if product == CommercialProduct::Retail {
                    origin.clone()
                } else {
                    digest(
                        &serde_json::to_vec(&(
                            "tenancy",
                            &native.tenant,
                            &native.account,
                            &native.workspace,
                        ))
                        .unwrap(),
                    )
                },
                commercial: CommercialRef {
                    binding: revision.binding.clone(),
                    revision: revision.revision,
                    digest: revision.digest.clone(),
                    customer: revision.customer.clone(),
                    workspace: revision.workspace.clone(),
                    source: portable,
                },
                pool: "canonical-pool".into(),
                ledger_origin: origin.clone(),
                custodian_node: FakeWallet::node(),
                controller: socket.clone(),
                conversion,
                operator_review: "Explicit isolated source and conversion review".into(),
            };
            let token = format!("{}", index + 1).repeat(64);
            let token_file = root.path().join(format!("client-{index}"));
            write(&token_file, token.as_bytes());
            let client = Client {
                config: ClientConfig {
                    binding: binding.id.clone(),
                    socket: socket.clone(),
                    token_file,
                    origin: origin.clone(),
                },
            };
            grants.push(Grant {
                binding: binding.clone(),
                credential_digest: digest(token.as_bytes()),
                native_credential_file: if product == CommercialProduct::Retail {
                    retail_credential.clone()
                } else {
                    native_credential.clone()
                },
                native: if product == CommercialProduct::Retail {
                    Native::Retail {
                        principal: "cli:retail".into(),
                        generation: 1,
                    }
                } else {
                    Native::Tenancy {
                        directory: native.directory.clone(),
                        principal: principal.clone(),
                        tenant: native.tenant.clone(),
                        member_epoch: member.epoch,
                        members_epoch: member.members_epoch,
                        money_ledger: (product == CommercialProduct::Gateway)
                            .then(|| native.money.clone()),
                        buyer_root: (product == CommercialProduct::Plugin)
                            .then(|| buyer_root.clone()),
                    }
                },
                reviewed_at: now(),
                valid_until: now() + 3600,
                previous_binding: None,
            });
            clients.push(client);
            bindings.push(binding);
        }
        let config = root.path().join("controller.json");
        write(
            &config,
            &serde_json::to_vec(&Config {
                schema: pay_ledger::shared::SCHEMA.into(),
                ledger: ledger.clone(),
                origin,
                wallet_home: wallet_home.clone(),
                socket: socket.clone(),
                writer_file,
                canonical_directory: canonical.clone(),
                commercial,
                grants,
                refunds: vec![],
                funding_reversals: vec![],
            })
            .unwrap(),
        );
        let controller = Arc::new(Controller::open(&config).unwrap());
        let controller_stop = Arc::new(AtomicBool::new(false));
        let running = controller.clone();
        let stop = controller_stop.clone();
        let controller_thread = std::thread::spawn(move || running.run(stop).unwrap());
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !socket.exists() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let gateway = clients.remove(0);
        let plugin = clients.remove(0);
        let retail = clients.remove(0);
        let gateway_binding = bindings.remove(0);
        let plugin_binding = bindings.remove(0);
        let retail_binding = bindings.remove(0);
        let client_path = native.directory.join("shared-spend").join(format!(
            "{}.client.json",
            digest(native.workspace.as_bytes())
        ));
        write(&client_path, &serde_json::to_vec(&gateway.config).unwrap());
        write(
            &wallet_home.join("shared-client.json"),
            &serde_json::to_vec(&plugin.config).unwrap(),
        );
        Self {
            root,
            native,
            canonical,
            ledger,
            config,
            wallet_home,
            buyer_root,
            fake,
            gateway,
            retail,
            plugin,
            gateway_binding,
            retail_binding,
            plugin_binding,
            controller: Some(controller),
            controller_stop,
            wallet_stop,
            controller_thread: Some(controller_thread),
            wallet_thread: Some(wallet_thread),
        }
    }
    pub fn fund(&self, purchase: &str, sats: u64) -> Value {
        self.retail
            .call(Operation::Binding {})
            .expect("current native funding admission");
        self.retail
            .call(Operation::Funding {
                purchase: purchase.into(),
                amount_sats: sats,
            })
            .unwrap_or_else(|e| {
                let id = format!(
                    "shared-funding:{}",
                    digest(
                        &serde_json::to_vec(&(self.retail_binding.native_identity(), purchase))
                            .unwrap()
                    )
                );
                let retained = Ledger::open_read_only(&self.ledger)
                    .unwrap()
                    .shared_funding_record(&id)
                    .unwrap();
                panic!(
                    "funding refused: {e}; incoming={}; retained_invoice={}",
                    self.fake.incoming.load(Ordering::SeqCst),
                    retained.is_some_and(|r| r.2.is_some())
                );
            })
    }
    pub fn stop_controller(&mut self) {
        self.controller_stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.controller_thread.take() {
            t.join().unwrap();
        }
        self.controller.take();
    }
    pub fn restart_controller(&mut self) {
        assert!(self.controller.is_none());
        let controller = Arc::new(Controller::open(&self.config).unwrap());
        self.controller_stop = Arc::new(AtomicBool::new(false));
        let running = controller.clone();
        let stop = self.controller_stop.clone();
        self.controller_thread = Some(std::thread::spawn(move || running.run(stop).unwrap()));
        self.controller = Some(controller);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while self.plugin.call(Operation::Identity {}).is_err() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.fake.release.store(true, Ordering::SeqCst);
        self.stop_controller();
        self.wallet_stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.wallet_thread.take() {
            t.join().unwrap();
        }
    }
}
