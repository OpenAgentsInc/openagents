//! Installed CLI purchase adapter over canonical customer and x402 contracts.
use crate::{
    Args, Output,
    pay_plugin::{self, PluginSource, Resolved},
};
use coder::customer::{
    Selection, Store,
    plugins::{Charge, Offer, Packet, Payer, Phase, View},
};
use nostr::x402::{SupportedProfiles, binding_hash, http_binding, validate_challenge};
use openagents_wallet::{LightningWallet, resident::RemoteWallet};
use openagents_x402::{
    PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentPayload, execution,
    front::Quote,
    policy::{Entry, Flags, Ledger, Policy},
    wire,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Duration};

fn now() -> u64 {
    openagents_x402::unix_now().saturating_mul(1000)
}
fn required<'a>(a: &'a Args, k: &str) -> Result<&'a str, String> {
    a.option(k)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("--{k} is required"))
}
fn parse(words: &[String]) -> Result<Args, String> {
    let a = Args::parse(words, &[])?;
    if a.positional().len() != 1 {
        return Err("Choose one plugin purchase command.".into());
    }
    let allowed: &[&str] = match a.positional()[0].as_str() {
        "quote" => &[
            "root",
            "purchase",
            "plugin",
            "input",
            "file",
            "wallet-home",
            "max-msat",
            "max-fee-msat",
            "relay",
            "blossom",
        ],
        "approve" => &["root", "purchase", "digest"],
        "invoke" => &["root", "purchase", "wait"],
        "show" | "cancel" | "recover" => &["root", "purchase"],
        _ => return Err("Unknown plugin purchase command.".into()),
    };
    if a.option_names().iter().any(|n| !allowed.contains(n)) {
        return Err("Unknown plugin purchase option.".into());
    }
    required(&a, "root")?;
    required(&a, "purchase")?;
    Ok(a)
}
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words.first().is_some_and(|s| s == "--help") {
        println!("{}", super::EXT_USAGE);
        return 0;
    }
    let a = match parse(words) {
        Ok(a) => a,
        Err(e) => return output.usage("plugin purchase", &e, super::EXT_USAGE),
    };
    match execute(&a) {
        Ok(view) => {
            let ok = !matches!(view.phase, Phase::Unknown | Phase::Failed);
            output.emit(
                &serde_json::to_value(view).expect("purchase view serializes"),
                |v| serde_json::to_string_pretty(v).unwrap_or_default(),
            );
            if ok { 0 } else { 1 }
        }
        Err(e) => output.fail("plugin purchase", &e),
    }
}
fn resident(home: &Path) -> Result<(RemoteWallet, Payer), String> {
    use std::os::unix::fs::PermissionsExt;
    let m = std::fs::symlink_metadata(home)
        .map_err(|_| "Approved resident wallet home is unavailable.")?;
    if !home.is_absolute() || !m.is_dir() || m.permissions().mode() & 0o077 != 0 {
        return Err("Wallet home must be an explicit existing private directory.".into());
    }
    let home = home
        .canonicalize()
        .map_err(|_| "Wallet home is unavailable.")?;
    let config: openagents_wallet::WalletConfig = serde_json::from_slice(&Store::private_input(
        &home.join(openagents_wallet::config::CONFIG_FILE),
        64 * 1024,
    )?)
    .map_err(|_| "Resident wallet configuration is unavailable.")?;
    let network = openagents_x402::network_id(config.network.as_str())
        .ok_or("Only admitted Bitcoin or testnet Lightning transport is supported.")?;
    let wallet = RemoteWallet::probe(&home)
        .ok_or("The approved resident wallet is unavailable; no replacement wallet was opened.")?;
    let node=wallet.bound_payment_identity().map_err(|_|"The resident cannot bind payment to the selected node; upgrade the admitted resident before purchase.")?;
    if node != wallet.node_id() {
        return Err(
            "The resident payer changed during connection; no payment was dispatched.".into(),
        );
    }
    let payer = Payer {
        home,
        node,
        network: network.into(),
    };
    Ok((wallet, payer))
}
fn source(root: &Path, offer: &Offer) -> pay_plugin::RegistrySource {
    pay_plugin::RegistrySource::new(
        offer.relay.clone(),
        offer.blossom.clone(),
        root.join("plugin-blobs"),
    )
}
fn packet(
    resolved: &Resolved,
    request: &str,
    files: &BTreeMap<String, String>,
) -> Result<Packet, String> {
    let p = &resolved.packet;
    let input = match &p.request_key {
        Some(k) => plugin::scope::with_request(&p.input, k, request)?,
        None => p.input.clone(),
    };
    let snapshot = p
        .snapshot(files)
        .map_err(|why| format!("Supplied files do not match the signed release: {why}."))?;
    Ok(Packet {
        snapshot: plugin::snapshot_digest(&snapshot, &BTreeMap::new()),
        module: plugin::digest(&p.wasm),
        input: plugin::digest(plugin::canonical(&input).as_bytes()),
        operation: p.operation.clone(),
        profile: match p.profile {
            plugin::Profile::Pure => "pure",
            plugin::Profile::SnapshotRead => "snapshot-read",
        }
        .into(),
        limits: json!({"fuel":p.limits.fuel,"memory_bytes":p.limits.memory_bytes,"output_bytes":p.limits.output_bytes,"read_bytes":p.limits.read_bytes,"module_bytes":p.limits.module_bytes}),
    })
}
struct Reply {
    status: u16,
    required: Option<String>,
    settlement: Option<String>,
    body: Vec<u8>,
}
trait Transport {
    fn send(&self, url: &str, body: &[u8], signature: Option<&str>) -> Result<Reply, String>;
    fn invoke(
        &self,
        url: &str,
        body: &[u8],
        signature: &str,
        authorization: Option<&str>,
    ) -> Result<Reply, String> {
        if authorization.is_some() {
            return Err(
                "Original private invocation authorization transport is unavailable.".into(),
            );
        }
        self.send(url, body, Some(signature))
    }
    fn recover(
        &self,
        _url: &str,
        _body: &[u8],
        _secret: &str,
        _payment_hash: &str,
    ) -> Result<Reply, String> {
        Err("The selected service has no admitted recovery transport.".into())
    }
}
struct Http(reqwest::blocking::Client);
impl Http {
    fn new() -> Result<Self, String> {
        Ok(Self(
            reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(60))
                .build()
                .map_err(|_| "Plugin transport is unavailable.")?,
        ))
    }
}
impl Transport for Http {
    fn send(&self, url: &str, body: &[u8], signature: Option<&str>) -> Result<Reply, String> {
        self.request(url, body, signature, None, None)
    }
    fn invoke(
        &self,
        url: &str,
        body: &[u8],
        signature: &str,
        authorization: Option<&str>,
    ) -> Result<Reply, String> {
        self.request(url, body, Some(signature), None, authorization)
    }
    fn recover(
        &self,
        url: &str,
        body: &[u8],
        secret: &str,
        payment_hash: &str,
    ) -> Result<Reply, String> {
        self.request(url, body, None, Some((secret, payment_hash)), None)
    }
}
impl Http {
    fn request(
        &self,
        url: &str,
        body: &[u8],
        signature: Option<&str>,
        recovery: Option<(&str, &str)>,
        authorization: Option<&str>,
    ) -> Result<Reply, String> {
        use std::io::Read;
        let mut request = self
            .0
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_vec());
        if let Some(s) = signature {
            request = request.header(PAYMENT_SIGNATURE, s);
        }
        if let Some((secret, hash)) = recovery {
            request = request
                .header(openagents_x402::outcome::AUTHORIZATION, secret)
                .header(openagents_x402::outcome::PAYMENT, hash);
        }
        if let Some(secret) = authorization {
            request = request.header(openagents_x402::outcome::AUTHORIZATION, secret);
        }
        let response = request
            .send()
            .map_err(|_| "Plugin service response is unavailable.")?;
        let status = response.status().as_u16();
        let header = |name| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let required = header(PAYMENT_REQUIRED);
        let settlement = header(PAYMENT_RESPONSE);
        let bound = if recovery.is_some() {
            256 * 1024
        } else {
            128 * 1024
        };
        let mut body = Vec::new();
        response
            .take(bound + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Plugin service body is unavailable.")?;
        if body.len() as u64 > bound {
            return Err("Plugin response exceeds the admitted bound.".into());
        }
        Ok(Reply {
            status,
            required,
            settlement,
            body,
        })
    }
}
fn preview(transport: &dyn Transport, url: &str) -> Result<Quote, String> {
    let reply = transport.send(url, b"{\"request\":\"\"}", None)?;
    let value: Value =
        serde_json::from_slice(&reply.body).map_err(|_| "Plugin quote preview is unavailable.")?;
    if reply.status != 409 || value["error"]["type"] != "quote_conflict" {
        return Err("Service does not offer the supported exact plugin quote contract.".into());
    }
    let quote: Quote =
        serde_json::from_value(value["quote"].clone()).map_err(|_| "Invalid plugin quote.")?;
    if value["quote_digest"] != execution::quote_digest(&quote) {
        return Err("Plugin quote digest changed.".into());
    }
    Ok(quote)
}
fn resolved(
    source: &dyn PluginSource,
    offer: &Offer,
    request: &str,
    files: &BTreeMap<String, String>,
) -> Result<Packet, String> {
    let id = offer
        .quote
        .plugin
        .as_deref()
        .ok_or("Missing plugin identity.")?;
    let r = source.resolve(id).map_err(|e| e.message)?;
    if Some(&r.release) != offer.quote.release.as_ref()
        || Some(&r.author) != offer.quote.author.as_ref()
        || Some(r.fee_msat) != offer.quote.fee_msat
    {
        return Err("Signed plugin release or author terms changed; review a new purchase.".into());
    }
    packet(&r, request, files)
}
/// `NAME=PATH` options: the snapshot entries the request supplies, read
/// from private files, each at most the 32 KiB a request text may be.
fn supplied_files(a: &Args) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    for option in a.options("file") {
        let (name, path) = option
            .split_once('=')
            .filter(|(name, path)| !name.is_empty() && !path.is_empty())
            .ok_or("Each --file is NAME=PATH.")?;
        let text = String::from_utf8(Store::private_input(Path::new(path), 32 * 1024)?)
            .map_err(|_| "Supplied files must be UTF-8 text.")?;
        if files.insert(name.to_owned(), text).is_some() {
            return Err("Each supplied file name appears once.".into());
        }
    }
    Ok(files)
}
fn execute(a: &Args) -> Result<View, String> {
    let root = Path::new(required(a, "root")?);
    let id = required(a, "purchase")?;
    let mut store = Store::open(root)?;
    let command = a.positional()[0].as_str();
    if command == "show" {
        return store.plugin_view(id);
    }
    if command == "cancel" {
        return store.cancel_plugin(id);
    }
    if command != "recover" {
        let current = crate::runtime().block_on(store.current_selection())?;
        if current.context.team_policy.is_some() {
            return Err("Paid plugin quote, approval, and dispatch are unavailable under this team policy; use original read-only recovery for existing payments.".into());
        }
    }
    let transport = Http::new()?;
    if command == "quote" {
        let current = crate::runtime().block_on(store.current_selection())?;
        let commercial = crate::runtime()
            .block_on(store.commercial_selection(receipts::purchase::CommercialProduct::Plugin))?;
        let plugin = required(a, "plugin")?;
        let request = String::from_utf8(Store::private_input(
            Path::new(required(a, "input")?),
            32 * 1024,
        )?)
        .map_err(|_| "Plugin notes must be UTF-8 text.")?;
        let files = supplied_files(a)?;
        let (_, payer) = resident(Path::new(required(a, "wallet-home")?))?;
        let max_msat = a.number::<u64>("max-msat", 0)?;
        let max_fee_msat = a.number::<u64>("max-fee-msat", 0)?;
        required(a, "max-msat")?;
        required(a, "max-fee-msat")?;
        let mut url = reqwest::Url::parse(&current.origin)
            .map_err(|_| "Invalid selected customer origin.")?;
        url.path_segments_mut()
            .map_err(|_| "Invalid selected origin.")?
            .clear()
            .extend(["v1", "plugins", plugin, "invoke"]);
        let quote = preview(&transport, url.as_str())?;
        let mut offer = Offer {
            url: url.to_string(),
            relay: crate::relay::relay_url(a.option("relay")),
            blossom: a.option("blossom").map(str::to_owned),
            quote,
            payment: wire::PaymentRequired {
                x402_version: 2,
                error: None,
                resource: wire::ResourceInfo {
                    url: url.to_string(),
                    description: None,
                    mime_type: None,
                    rest: Default::default(),
                },
                accepts: vec![],
                extensions: None,
            },
            packet: Packet {
                module: String::new(),
                input: String::new(),
                operation: String::new(),
                profile: String::new(),
                limits: Value::Null,
                snapshot: String::new(),
            },
            payer,
            max_msat,
            max_fee_msat,
            request_hash: String::new(),
            expires_at_ms: 0,
            recovery_authorization: None,
            commercial,
            shared: None,
        };
        use std::io::Read;
        let mut random = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|_| "Private purchase authorization is unavailable.")?;
        let secret = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        offer.recovery_authorization = Some(openagents_x402::outcome::commitment(&secret));
        if offer.quote.plugin.as_deref() != Some(plugin) {
            return Err("Preview identifies another plugin.".into());
        }
        offer.packet = resolved(&source(root, &offer), &offer, &request, &files)?;
        if offer.quote.price_msat > max_msat {
            return Err("Plugin total exceeds the approved maximum; no invoice was paid.".into());
        }
        let body = offer.body_with(&request, &files);
        offer.request_hash = binding_hash(
            &http_binding("POST", &offer.url, &body, &[])
                .map_err(|_| "Invalid exact plugin binding.")?,
        )
        .map_err(|_| "Invalid exact plugin request hash.")?;
        let reply = transport.send(&offer.url, &body, None)?;
        if reply.status != 402 {
            return Err(
                "Plugin terms changed or invoice issuance is unavailable; review a new purchase."
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&reply.body)
            .ok()
            .is_none_or(|v| v["recovery_contract"] != openagents_x402::outcome::SCHEMA)
        {
            return Err("The service does not admit the reviewed private outcome recovery contract; no payment was dispatched.".into());
        }
        offer.payment = wire::decode_payment_required(
            reply
                .required
                .as_deref()
                .ok_or("Missing exact plugin invoice.")?,
        )
        .map_err(|_| "Invalid x402 plugin challenge.")?;
        if offer.payment.accepts.len() != 1 {
            return Err("Choose exactly one supported Lightning invoice.".into());
        }
        let at = now();
        let invoice = validate_challenge(
            &offer.payment.accepts[0],
            &offer.request_hash,
            at / 1000,
            0,
            SupportedProfiles {
                http: true,
                mcp: false,
                native: false,
            },
        )
        .map_err(|_| "Plugin invoice does not bind the exact approved resource.")?;
        offer.expires_at_ms = at.saturating_add(receipts::purchase::MAX_QUOTE_MS).min(
            invoice
                .created_at()
                .saturating_add(invoice.expiry_seconds())
                .saturating_mul(1000),
        );
        if let Some(client) = shared_client(&offer.payer)? {
            let binding: pay_ledger::shared::Binding = serde_json::from_value(
                client
                    .call(pay_ledger::shared::Operation::Binding {})
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|_| "Invalid current shared plugin binding.")?;
            let intent = commercial_spend::wallet::plugin_intent(id, &offer, binding.clone())
                .map_err(|_| "Shared plugin conversion or original invoice is unqualified.")?;
            offer.shared = Some(receipts::shared_spend::Reference {
                intent: intent.id.clone(),
                digest: intent.digest(),
                mode: binding.mode(),
            });
        }
        return store.quote_plugin_supplied(id, offer, request, files, current, at, Some(secret));
    }
    let view = store.plugin_view(id)?;
    // Refuse a retry before opening even the payment transport.
    if command == "invoke" && view.phase != Phase::Approved {
        return Err("This purchase is unapproved or was already attempted. Retained uncertainty requires recovery, never repayment.".into());
    }
    let (wallet, payer) = resident(&view.offer.payer.home)?;
    if command == "recover" {
        if view.offer.commercial.is_some() {
            let reader = crate::runtime().block_on(store.plugin_native_reader(id))?;
            return recover_native(
                &mut store,
                id,
                &reader,
                &payer,
                &wallet,
                &transport,
                &crate::x402::open_ledger(),
                now(),
            );
        }
        let current = crate::runtime().block_on(store.current_selection())?;
        let commercial = crate::runtime()
            .block_on(store.commercial_selection(receipts::purchase::CommercialProduct::Plugin))?;
        return recover_reviewed(
            &mut store,
            id,
            &current,
            &payer,
            commercial.as_ref(),
            &wallet,
            &transport,
            &crate::x402::open_ledger(),
            now(),
        );
    }
    let supplied = store.plugin_request(id)?.to_owned();
    let files = store.plugin_files(id)?.clone();
    let source = source(root, &view.offer);
    let packet = resolved(&source, &view.offer, &supplied, &files)?;
    if packet != view.offer.packet || preview(&transport, &view.offer.url)? != view.offer.quote {
        return Err(
            "Plugin release, packet, or price changed; review and approve a new purchase.".into(),
        );
    }
    // Registry and quote reads can take time; recheck account rights after
    // those reads and before approving or starting the wallet dispatch.
    let current = crate::runtime().block_on(store.current_selection())?;
    let commercial = crate::runtime()
        .block_on(store.commercial_selection(receipts::purchase::CommercialProduct::Plugin))?;
    if current.context.team_policy.is_some() {
        return Err("Paid plugin dispatch is unavailable under the current team policy.".into());
    }
    if command == "approve" {
        return store.approve_plugin_reviewed(
            id,
            required(a, "digest")?,
            &current,
            &payer,
            commercial.as_ref(),
            now(),
        );
    }
    let policy = crate::x402::load_policy()?;
    let spent = crate::x402::open_ledger()
        .spent_since(openagents_x402::unix_now().saturating_sub(openagents_x402::policy::DAY_SECS))
        .map_err(|e| e.to_string())?;
    buy_reviewed(
        &mut store,
        id,
        &current,
        &payer,
        &packet,
        commercial.as_ref(),
        &wallet,
        &transport,
        policy.as_ref(),
        spent,
        &crate::x402::open_ledger(),
        a.number::<u64>("wait", 60)?.clamp(1, 300),
        now(),
    )
}
#[cfg(test)]
fn recover(
    store: &mut Store,
    id: &str,
    current: &Selection,
    payer: &Payer,
    wallet: &dyn LightningWallet,
    transport: &dyn Transport,
    ledger: &Ledger,
    at: u64,
) -> Result<View, String> {
    recover_reviewed(
        store, id, current, payer, None, wallet, transport, ledger, at,
    )
}
fn recover_reviewed(
    store: &mut Store,
    id: &str,
    current: &Selection,
    payer: &Payer,
    commercial: Option<&receipts::purchase::CommercialRef>,
    wallet: &dyn LightningWallet,
    transport: &dyn Transport,
    ledger: &Ledger,
    at: u64,
) -> Result<View, String> {
    let original = store.plugin_recovery_reviewed(id, current, payer, commercial)?;
    recover_original(store, id, payer, wallet, transport, ledger, at, original)
}
fn recover_native(
    store: &mut Store,
    id: &str,
    reader: &coder::customer::plugins::NativeReader,
    payer: &Payer,
    wallet: &dyn LightningWallet,
    transport: &dyn Transport,
    ledger: &Ledger,
    at: u64,
) -> Result<View, String> {
    let original = store.plugin_recovery_native(id, reader, payer)?;
    recover_original(store, id, payer, wallet, transport, ledger, at, original)
}
fn recover_original(
    store: &mut Store,
    id: &str,
    payer: &Payer,
    wallet: &dyn LightningWallet,
    transport: &dyn Transport,
    ledger: &Ledger,
    at: u64,
    (offer, body, secret): (Offer, Vec<u8>, String),
) -> Result<View, String> {
    if let Some(reference) = &offer.shared {
        let client = shared_client(&offer.payer)?
            .ok_or("The original shared custodian is required for recovery.")?;
        check_shared_reader(&client, reference)?;
        client
            .call(pay_ledger::shared::Operation::ReconcilePlugin {
                id: reference.intent.clone(),
            })
            .map_err(
                |_| "Original canonical payment reconciliation is unavailable; never repay.",
            )?;
    }
    use openagents_wallet::{PaymentDirection, PaymentStatus};
    let invoice =
        nostr::x402::decode_invoice(offer.invoice()).map_err(|_| "Retained invoice is invalid.")?;
    let hash = invoice
        .payment_hash()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let Some(record) = wallet
        .lookup_from_node(&payer.node, invoice.payment_hash())
        .map_err(
            |_| "Original resident payment lookup is unavailable; keep its unresolved liability.",
        )?
    else {
        return store.plugin_view(id);
    };
    if record.status != PaymentStatus::Succeeded
        || record.direction != PaymentDirection::Outbound
        || record.payment_hash != hash
        || record.amount_msat != Some(offer.quote.price_msat)
        || record.bolt11.as_ref().is_some_and(|s| s != offer.invoice())
        || record.updated_at < invoice.created_at()
        || record.updated_at > at / 1000 + nostr::x402::DEFAULT_CLOCK_SKEW
    {
        return Err("Original payment lookup is pending, failed, or mismatched; recovery cannot release liability or pay again.".into());
    }
    let Some(fee_msat) = record.fee_msat else {
        return store.plugin_view(id);
    };
    let Some(preimage) = record.preimage else {
        return store.plugin_view(id);
    };
    let charge = Charge {
        payment_hash: hash.clone(),
        amount_msat: offer.quote.price_msat,
        fee_msat,
    };
    store.plugin_recovered_charge(id, charge.clone(), &preimage)?;
    ledger
        .record_once(&Entry {
            paid_at: record.updated_at,
            binding: "http:1".into(),
            network: payer.network.clone(),
            provider: offer.payment.accepts[0].pay_to.clone(),
            capability: None,
            resource: offer.url.clone(),
            amount_msat: charge.amount_msat,
            fee_msat: charge.fee_msat,
            payment_hash: hash.clone(),
            phase: "paid".into(),
        })
        .map_err(
            |_| "Known payment could not be recorded in the buyer ledger; do not pay again.",
        )?;
    let reply = match transport.recover(&offer.url, &body, &secret, &hash) {
        Ok(r) => r,
        Err(_) => return store.plugin_view(id),
    };
    if reply.status != 200 {
        return store.plugin_view(id);
    }
    let evidence: openagents_x402::outcome::View = serde_json::from_slice(&reply.body)
        .map_err(|_| "Private recovery evidence is invalid; keep the original liability.")?;
    let view = store.plugin_recovered(id, evidence)?;
    let phase = match view.phase {
        Phase::Completed => "http_200",
        Phase::Failed => "delivery_failed",
        _ => "paid_delivery_unknown",
    };
    ledger.set_phase(&hash, phase).map_err(
        |_| "Known recovery evidence is retained but the buyer ledger phase needs reconciliation.",
    )?;
    Ok(view)
}
#[cfg(test)]
fn buy(
    store: &mut Store,
    id: &str,
    current: &Selection,
    payer: &Payer,
    packet: &Packet,
    wallet: &dyn LightningWallet,
    transport: &dyn Transport,
    policy: Option<&Policy>,
    spent: u64,
    ledger: &Ledger,
    wait: u64,
    at: u64,
) -> Result<View, String> {
    buy_reviewed(
        store, id, current, payer, packet, None, wallet, transport, policy, spent, ledger, wait, at,
    )
}
fn buy_reviewed(
    store: &mut Store,
    id: &str,
    current: &Selection,
    payer: &Payer,
    packet: &Packet,
    commercial: Option<&receipts::purchase::CommercialRef>,
    wallet: &dyn LightningWallet,
    transport: &dyn Transport,
    policy: Option<&Policy>,
    spent: u64,
    ledger: &Ledger,
    wait: u64,
    at: u64,
) -> Result<View, String> {
    let view = store.plugin_view(id)?;
    let offer = &view.offer;
    let limits = Policy::limits(
        policy,
        Flags {
            max_msat: Some(offer.max_msat),
            max_fee_msat: Some(offer.max_fee_msat),
        },
        Some(&offer.payment.accepts[0].pay_to),
        None,
    )
    .map_err(|e| e.to_string())?;
    Policy::admit(
        policy,
        limits,
        &offer.payment.accepts[0].pay_to,
        offer.quote.price_msat,
        spent,
    )
    .map_err(|e| e.to_string())?;
    if wallet.node_id() != payer.node {
        return Err("The approved payer node changed; no payment was dispatched.".into());
    }
    let authorization = store.plugin_invocation_authorization_reviewed(id, current, commercial)?;
    let shared = shared_client(payer)?;
    if let Some(reference) = &offer.shared {
        let client = shared
            .as_ref()
            .ok_or("Original shared custody is required; no raw payment fallback is allowed.")?;
        check_shared_client(client, reference)?;
        let binding: pay_ledger::shared::Binding = serde_json::from_value(
            client
                .call(pay_ledger::shared::Operation::Binding {})
                .map_err(|_| "Current shared spending authority is unavailable.")?,
        )
        .map_err(|_| "Invalid shared binding.")?;
        if binding.mode() != reference.mode {
            return Err(
                "Shared purchase mapping changed; review a new purchase before payment.".into(),
            );
        }
    } else if shared.is_some() {
        return Err("Shared custody requires an originally reviewed canonical intent; review a new purchase.".into());
    }
    let (offer, body) = store.begin_plugin_reviewed(id, current, payer, packet, commercial, at)?;
    let payment = if let (Some(client), Some(reference)) = (&shared, &offer.shared) {
        let binding: pay_ledger::shared::Binding = serde_json::from_value(
            client
                .call(pay_ledger::shared::Operation::Binding {})
                .map_err(|_| "Current shared spending authority is unavailable.")?,
        )
        .map_err(|_| "Invalid shared binding.")?;
        let intent = commercial_spend::wallet::plugin_intent(id, &offer, binding)
            .map_err(|_| "Original shared intent changed.")?;
        if intent.digest() != reference.digest || intent.id != reference.intent {
            return store.plugin_unknown(id);
        }
        client
            .call(pay_ledger::shared::Operation::DispatchPlugin {
                intent,
                wait_secs: wait,
            })
            .ok()
            .and_then(|v| serde_json::from_value::<pay_ledger::shared::Outcome>(v).ok())
            .and_then(|o| o.expense)
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or(openagents_wallet::WalletError::Node(
                "Original shared payment remains unresolved.".into(),
            ))
    } else {
        wallet.pay_from_node(
            &payer.node,
            offer.invoice(),
            offer.max_fee_msat,
            Duration::from_secs(wait),
        )
    };
    let proof = match payment {
        Ok(proof) => proof,
        Err(_) => return store.plugin_unknown(id),
    };
    if proof.bolt11 != offer.invoice()
        || store
            .plugin_paid(
                id,
                &proof.preimage,
                Charge {
                    payment_hash: proof.payment_hash.clone(),
                    amount_msat: proof.amount_msat,
                    fee_msat: proof.fee_msat,
                },
                at,
            )
            .is_err()
    {
        return store.plugin_unknown(id);
    }
    if ledger
        .record_once(&Entry {
            paid_at: at / 1000,
            binding: "http:1".into(),
            network: payer.network.clone(),
            provider: offer.payment.accepts[0].pay_to.clone(),
            capability: None,
            resource: offer.url.clone(),
            amount_msat: proof.amount_msat,
            fee_msat: proof.fee_msat,
            payment_hash: proof.payment_hash.clone(),
            phase: "paid".into(),
        })
        .is_err()
    {
        return store.plugin_unknown(id);
    }
    let mut payload = serde_json::Map::new();
    payload.insert("preimage".into(), json!(proof.preimage));
    let signature = wire::encode_header(&PaymentPayload {
        x402_version: 2,
        resource: Some(offer.payment.resource.clone()),
        accepted: offer.payment.accepts[0].clone(),
        payload,
        extensions: None,
    })
    .map_err(|e| e.to_string())?;
    let reply = match transport.invoke(&offer.url, &body, &signature, authorization.as_deref()) {
        Ok(reply) => reply,
        Err(_) => return store.plugin_unknown(id),
    };
    let settlement: Option<wire::SettlementResponse> = reply
        .settlement
        .as_deref()
        .and_then(|s| wire::decode_header(s).ok());
    let result = serde_json::from_slice(&reply.body).ok();
    if reply.status == 200
        && let (Some(s), Some(r)) = (settlement.clone(), result)
        && let Ok(view) = store.plugin_delivered(id, s, r)
    {
        if ledger.set_phase(&proof.payment_hash, "http_200").is_err() {
            return Err("Delivery completed but the buyer ledger phase needs reconciliation; do not pay again.".into());
        }
        return Ok(view);
    }
    if reply.status >= 400
        && let Some(s) = settlement
        && let Ok(view) = store.plugin_failed_delivery(id, s, reply.status)
    {
        let _ = ledger.set_phase(&proof.payment_hash, "delivery_failed");
        return Ok(view);
    }
    // A 402, execution failure, or lost acknowledgement never pays a replacement invoice.
    store.plugin_unknown(id)
}
fn shared_client(payer: &Payer) -> Result<Option<pay_ledger::shared::Client>, String> {
    let Some(mode) = openagents_wallet::custody::read(&payer.home)
        .map_err(|_| "Retained shared wallet custody is unavailable.")?
    else {
        return Ok(None);
    };
    let config: pay_ledger::shared::ClientConfig = serde_json::from_slice(&Store::private_input(
        &payer.home.join("shared-client.json"),
        16 * 1024,
    )?)
    .map_err(
        |_| "The protected shared client selection is required; raw wallet payment is disabled.",
    )?;
    if config.origin != mode.origin || config.socket != mode.controller || payer.node != mode.node {
        return Err("Shared client differs from the original wallet custodian.".into());
    }
    Ok(Some(pay_ledger::shared::Client { config }))
}
fn check_shared_client(
    client: &pay_ledger::shared::Client,
    reference: &receipts::shared_spend::Reference,
) -> Result<(), String> {
    if client.config.binding != reference.mode.binding
        || client.config.origin != reference.mode.origin
        || client.config.socket != reference.mode.socket
    {
        return Err("The original shared source and custodian are required.".into());
    }
    Ok(())
}
fn check_shared_reader(
    client: &pay_ledger::shared::Client,
    reference: &receipts::shared_spend::Reference,
) -> Result<(), String> {
    if client.config.origin != reference.mode.origin
        || client.config.socket != reference.mode.socket
    {
        return Err("The original shared custodian is required for recovery.".into());
    }
    let current: pay_ledger::shared::Binding = serde_json::from_value(
        client
            .call(pay_ledger::shared::Operation::Identity {})
            .map_err(|_| "Current native read authority is required for shared recovery.")?,
    )
    .map_err(|_| "Invalid native shared identity.")?;
    if !current.mode().same_native(&reference.mode) {
        return Err("The original native buyer source is required for recovery.".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "plugin_purchase/tests.rs"]
mod tests;
