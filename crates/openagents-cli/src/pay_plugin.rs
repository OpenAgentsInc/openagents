//! `POST /v1/plugins/{id}/invoke` on the pay front (#10193): a paid call to
//! a published plugin.
//!
//! The route resolves `{id}` (`<publisher key>:<slug>`, or a slug one
//! publisher has published) to its newest signed release, pins that
//! release, and prices the call at the route's endpoint price plus the
//! release's `fee_msat`; the `402` names both parts. The settlement the
//! front writes before anything runs carries the plugin id, the release
//! id, the author (the release's signer), and the fee, so the ledger
//! splits the fee to the author (`pay_ledger::Split::Plugin`).
//!
//! The executor runs the pinned release's packet once through
//! `plugin::invoke_with_receipt`: no workspace, no network, no host
//! effects. A `pure` guest gets the packet only; a `snapshot-read` guest
//! (such as `explain-error`) runs with an empty snapshot, so it reads
//! nothing but the request it was sent. A plugin whose program is anything
//! but one module step, or that requires capabilities, is refused with
//! `plugin_not_invocable` before a price is quoted, so it can never be
//! sold.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder::package::Package;
use openagents_x402::front::{
    Call, Output as Served, Price, PricePart, Quote, RouteExecutor, Unpriced,
};
use serde_json::{Value, json};

use crate::plugin_registry::{self, Blobs, BlossomStore};

/// The role the ledger splits a plugin invocation by.
pub(crate) const ROLE: &str = "plugin_call";

/// How long a resolved listing is reused before the registry is asked
/// again, so a challenge and its paid retry see the same release.
const LISTING_TTL: Duration = Duration::from_secs(300);

/// One guest invocation's fixed parts: what the release pins.
pub(crate) struct Packet {
    pub wasm: Vec<u8>,
    pub profile: plugin::Profile,
    pub operation: String,
    /// The binding's fixed input; the request goes into `request_key`.
    pub input: Value,
    pub request_key: Option<String>,
}

/// A published plugin, resolved and pinned to one release.
pub(crate) struct Resolved {
    /// `<publisher>:<slug>`.
    pub id: String,
    pub release: String,
    /// The author party: the publisher key that signed the release.
    pub author: String,
    pub fee_msat: u64,
    pub packet: Packet,
}

/// Where a plugin id resolves: the registry, or a fake in tests.
pub(crate) trait PluginSource: Send + Sync {
    fn resolve(&self, id: &str) -> Result<Arc<Resolved>, Unpriced>;
}

fn refused(status: u16, kind: &str, message: impl Into<String>) -> Unpriced {
    Unpriced {
        status,
        kind: kind.into(),
        message: message.into(),
    }
}

fn not_invocable(message: impl Into<String>) -> Unpriced {
    refused(422, "plugin_not_invocable", message)
}

/// The packet of the plugin at `dir`: its program must be exactly one
/// `module` step that requires nothing, bound to inline guest bytes with
/// the `pure` or `snapshot-read` profile.
pub(crate) fn packet(dir: &Path) -> Result<Packet, Unpriced> {
    let package = Package::load(&dir.join("package.json")).map_err(not_invocable)?;
    let lock = Package::resolve(dir, &package)
        .map_err(|why| not_invocable(format!("the package doesn't resolve: {why}")))?;
    let pinned = lock
        .program
        .ok_or_else(|| not_invocable("the plugin has no program to invoke"))?;
    let path = dir.join(&pinned.found);
    let bytes = std::fs::read(&path).map_err(|e| not_invocable(format!("its program: {e}")))?;
    let program: Value =
        serde_json::from_slice(&bytes).map_err(|e| not_invocable(format!("its program: {e}")))?;
    let definition = &program["definition"];
    if definition["requires"]
        .as_array()
        .is_some_and(|required| !required.is_empty())
    {
        return Err(not_invocable(
            "the plugin requires capabilities; only pure guests are sold here",
        ));
    }
    let steps = definition["steps"].as_array().cloned().unwrap_or_default();
    let [step] = steps.as_slice() else {
        return Err(not_invocable(
            "the plugin's program is not one module step; only pure guests are sold here",
        ));
    };
    if step["kind"] != "module" {
        return Err(not_invocable(format!(
            "the plugin's step is a {} step, not a guest; only pure guests are sold here",
            step["kind"].as_str().unwrap_or("unknown")
        )));
    }
    let name = step["name"].as_str().unwrap_or_default();
    let module = &program["binding"]["steps"][name]["module"];
    let wasm = module["bytes_base64"]
        .as_str()
        .ok_or_else(|| not_invocable("the plugin's step names no guest bytes"))
        .and_then(|encoded| {
            plugin::decode_base64(encoded)
                .map_err(|_| not_invocable("the plugin's guest bytes are not base64"))
        })?;
    let profile = match module["profile"].as_str().unwrap_or("pure") {
        "pure" => plugin::Profile::Pure,
        "snapshot-read" => plugin::Profile::SnapshotRead,
        other => {
            return Err(not_invocable(format!(
                "the guest profile {other} is not sold here"
            )));
        }
    };
    let request_key = match &module["request"] {
        Value::Null => None,
        Value::String(key) if !key.is_empty() => Some(key.clone()),
        _ => return Err(not_invocable("the guest's request binding is malformed")),
    };
    Ok(Packet {
        wasm,
        profile,
        operation: module["operation"].as_str().unwrap_or("echo").to_owned(),
        input: module.get("input").cloned().unwrap_or(Value::Null),
        request_key,
    })
}

/// The route's price and executor: the endpoint price plus the pinned
/// release's fee, and the run of that release.
pub(crate) struct Invoke {
    endpoint_msat: u64,
    source: Arc<dyn PluginSource>,
    /// Releases quoted so far, by release id, so the executor runs exactly
    /// the release the payment bought.
    pinned: Mutex<HashMap<String, Arc<Resolved>>>,
}

impl Invoke {
    pub(crate) fn new(endpoint_msat: u64, source: Arc<dyn PluginSource>) -> Arc<Self> {
        Arc::new(Self {
            endpoint_msat,
            source,
            pinned: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn price(self: &Arc<Self>) -> Price {
        let this = Arc::clone(self);
        Price::Quote(Arc::new(move |call: &Call<'_>| this.quote(call)))
    }

    fn quote(&self, call: &Call<'_>) -> Result<Quote, Unpriced> {
        let id = call
            .param("id")
            .ok_or_else(|| refused(404, "plugin_not_found", "the path names no plugin"))?;
        let resolved = self.source.resolve(id)?;
        let price = self
            .endpoint_msat
            .checked_add(resolved.fee_msat)
            .ok_or_else(|| refused(422, "plugin_not_invocable", "the fee is too large"))?;
        let quote = Quote {
            price_msat: price,
            parts: vec![
                PricePart {
                    name: "endpoint".into(),
                    msat: self.endpoint_msat,
                },
                PricePart {
                    name: "author_fee".into(),
                    msat: resolved.fee_msat,
                },
            ],
            plugin: Some(resolved.id.clone()),
            release: Some(resolved.release.clone()),
            author: Some(resolved.author.clone()),
            fee_msat: Some(resolved.fee_msat),
            resource: None,
        };
        self.pinned
            .lock()
            .map_err(|_| refused(503, "plugin_unavailable", "the plugin cache is poisoned"))?
            .insert(resolved.release.clone(), resolved);
        Ok(quote)
    }
}

impl RouteExecutor for Invoke {
    fn execute(&self, call: &Call<'_>) -> Result<Served, String> {
        let release = call
            .quote
            .and_then(|quote| quote.release.as_deref())
            .ok_or("the call was not quoted for a release")?;
        let resolved = self
            .pinned
            .lock()
            .map_err(|_| "the plugin cache is poisoned")?
            .get(release)
            .cloned()
            .ok_or_else(|| format!("the release {release} is not pinned here"))?;
        let request = std::str::from_utf8(&call.request.body)
            .map_err(|_| "the request body is not UTF-8".to_string())?;
        let ran = run(&resolved, request, call.payment_hash.unwrap_or("unpaid"))?;
        Ok(Served {
            body: ran.to_string().into_bytes(),
            content_type: Some("application/json".into()),
        })
    }
}

/// Run `resolved`'s guest once on `request`, with no grant beyond the
/// packet, and return its value and receipt.
pub(crate) fn run(resolved: &Resolved, request: &str, invocation: &str) -> Result<Value, String> {
    let packet = &resolved.packet;
    let input = match &packet.request_key {
        Some(key) => plugin::scope::with_request(&packet.input, key, request)?,
        None => packet.input.clone(),
    };
    let (result, receipt) = plugin::invoke_with_receipt(plugin::Call {
        wasm: &packet.wasm,
        profile: packet.profile,
        invocation,
        operation: &packet.operation,
        input: &input,
        snapshot: &plugin::Snapshot::default(),
        handles: &BTreeMap::new(),
        limits: plugin::Limits::default(),
        cancelled: Arc::new(AtomicBool::new(false)),
        required: true,
    });
    let value = result.map_err(|error| format!("the plugin did not return a value: {error}"))?;
    Ok(json!({
        "plugin": resolved.id,
        "release": resolved.release,
        "status": value.status,
        "value": value.value,
        "verification": value.verification,
        "receipt": receipt.to_json(),
    }))
}

/// The registry as a [`PluginSource`]: the newest listing, its signed
/// release checked and fetched into `cache/<release>` once.
pub(crate) struct RegistrySource {
    relay: String,
    blossom: Option<String>,
    cache: PathBuf,
    listings: Mutex<HashMap<String, (Instant, Arc<Resolved>)>>,
}

impl RegistrySource {
    pub(crate) fn new(relay: String, blossom: Option<String>, cache: PathBuf) -> Self {
        Self {
            relay,
            blossom,
            cache,
            listings: Mutex::new(HashMap::new()),
        }
    }

    fn fetch(&self, id: &str) -> Result<Resolved, Unpriced> {
        let unavailable = |why: String| refused(503, "plugin_unavailable", why);
        let signer = crate::relay::signer_for(None).map_err(unavailable)?;
        let mut client = crate::relay::Client::connect(&self.relay, signer);
        let fetched = fetch_into(
            &mut client,
            id,
            self.blossom.as_deref(),
            &self.relay,
            &self.cache,
        );
        client.close();
        fetched
    }
}

/// Resolve `id` on `registry` and fetch its release into `cache/<release>`
/// unless a checked copy is already there.
pub(crate) fn fetch_into(
    registry: &mut dyn plugin_registry::Registry,
    id: &str,
    blossom: Option<&str>,
    relay: &str,
    cache: &Path,
) -> Result<Resolved, Unpriced> {
    let unavailable = |why: String| refused(503, "plugin_unavailable", why);
    let listing =
        plugin_registry::find(registry, id).map_err(|why| refused(404, "plugin_not_found", why))?;
    let release = listing.release["id"]
        .as_str()
        .ok_or_else(|| not_invocable("the listing names no release"))?
        .to_owned();
    if release.is_empty() || !release.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(not_invocable("the listing's release id is malformed"));
    }
    let dir = cache.join(&release);
    let pin_path = dir.join(".openagents-pin.json");
    let pin: Value = match std::fs::read(&pin_path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| unavailable(e.to_string()))?,
        Err(_) => {
            let mut owned: Vec<Box<dyn Blobs>> = Vec::new();
            for base in blossom
                .into_iter()
                .map(str::to_owned)
                .chain(listing.blobs.clone())
            {
                let store = ext_eval::blob::Blossom::new(&base).map_err(unavailable)?;
                owned.push(Box::new(BlossomStore::new(store, None)));
            }
            if let Ok(store) = ext_eval::blob::Blossom::for_relay(relay) {
                owned.push(Box::new(BlossomStore::new(store, None)));
            }
            let stores: Vec<&dyn Blobs> = owned.iter().map(AsRef::as_ref).collect();
            let staging = cache.join(format!(".fetching-{release}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&staging);
            std::fs::create_dir_all(&staging).map_err(|e| unavailable(e.to_string()))?;
            let fetched = plugin_registry::download(registry, &listing, &stores, &staging)
                .map_err(not_invocable)?;
            let record = Package::load(&staging.join("package.json")).map_err(not_invocable)?;
            if record.publisher != listing.publisher {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(not_invocable(
                    "its package.json names another publisher than the key that signed it",
                ));
            }
            let pin = json!({
                "id": listing.package,
                "release": release,
                "author": listing.publisher,
                "fee_msat": fetched.fee.as_ref().map_or(0, |fee| fee.msat),
            });
            std::fs::write(staging.join(".openagents-pin.json"), pin.to_string())
                .map_err(|e| unavailable(e.to_string()))?;
            if std::fs::rename(&staging, &dir).is_err() {
                // Another call fetched it first; use theirs.
                let _ = std::fs::remove_dir_all(&staging);
            }
            pin
        }
    };
    Ok(Resolved {
        id: pin["id"].as_str().unwrap_or(&listing.package).to_owned(),
        release,
        author: pin["author"]
            .as_str()
            .unwrap_or(&listing.publisher)
            .to_owned(),
        fee_msat: pin["fee_msat"].as_u64().unwrap_or(0),
        packet: packet(&dir)?,
    })
}

impl PluginSource for RegistrySource {
    fn resolve(&self, id: &str) -> Result<Arc<Resolved>, Unpriced> {
        if let Ok(listings) = self.listings.lock()
            && let Some((at, resolved)) = listings.get(id)
            && at.elapsed() < LISTING_TTL
        {
            return Ok(Arc::clone(resolved));
        }
        let resolved = Arc::new(self.fetch(id)?);
        if let Ok(mut listings) = self.listings.lock() {
            listings.insert(id.to_owned(), (Instant::now(), Arc::clone(&resolved)));
        }
        Ok(resolved)
    }
}

/// The pay-ledger as the front's settlement sink: a plugin call splits its
/// fee to the release's author, every other role goes to OpenAgents, and
/// every call that reaches a route is a `call` record.
pub(crate) struct LedgerSink(Mutex<pay_ledger::Ledger>);

impl LedgerSink {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        pay_ledger::Ledger::open(path)
            .map(|ledger| Self(Mutex::new(ledger)))
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> Self {
        Self(Mutex::new(pay_ledger::Ledger::in_memory().unwrap()))
    }

    /// Record where `party`'s shares are paid, from a signed hosted
    /// resource registration (`source` = `registration`).
    pub(crate) fn register_payout(&self, party: &str, payout: &str, at: u64) -> Result<(), String> {
        let (kind, value) = pay_ledger::payee::classify(payout).ok_or_else(|| {
            format!("payout {payout} is not a Spark address, Lightning address, or node key")
        })?;
        self.0
            .lock()
            .map_err(|_| "the ledger lock is poisoned".to_string())?
            .register_payee(pay_ledger::Payee {
                party: party.to_owned(),
                destination_kind: kind.as_str().into(),
                destination_value: value,
                source: pay_ledger::payee::Source::Registration.as_str().into(),
                verified_at: msat(at)?,
            })
            .map_err(|e| e.to_string())
    }

    #[cfg(test)]
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut pay_ledger::Ledger) -> T) -> T {
        f(&mut self.0.lock().unwrap())
    }
}

fn msat(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| "an amount is too large for the ledger".to_string())
}

impl openagents_x402::front::SettlementSink for LedgerSink {
    fn on_settled(&self, settlement: &openagents_x402::front::Settlement) -> Result<(), String> {
        let split = match (&settlement.author, settlement.fee_msat) {
            (Some(author), Some(fee)) if settlement.role == ROLE => pay_ledger::Split::Plugin {
                author: author.clone(),
                fee_msat: msat(fee)?,
            },
            (Some(owner), _) if settlement.role == openagents_x402::hosted::ROLE => {
                pay_ledger::Split::HostedResource {
                    owner: owner.clone(),
                }
            }
            _ => pay_ledger::Split::OpenAgents,
        };
        let input = pay_ledger::SettlementInput {
            key: settlement.payment_hash.clone(),
            resource: settlement.resource.clone(),
            plugin_id: settlement.plugin.clone(),
            release_id: settlement.release.clone(),
            price_msat: msat(settlement.price_msat)?,
            received_msat: msat(settlement.received_msat.min(settlement.price_msat))?,
            rail: pay_ledger::Rail::Lightning,
            payer_alias: None,
            settled_at: msat(settlement.settled_at)?,
            split,
        };
        self.0
            .lock()
            .map_err(|_| "the ledger lock is poisoned".to_string())?
            .record_settlement(input)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn on_call(&self, usage: &openagents_x402::front::Usage) {
        let record = pay_ledger::CallRecord {
            at: i64::try_from(usage.at).unwrap_or(i64::MAX),
            route: usage.route.clone(),
            resource: usage.resource.clone(),
            plugin_id: usage.plugin.clone(),
            release_id: usage.release.clone(),
            outcome: usage.outcome.clone(),
            paid: usage.paid,
            price_msat: usage.price_msat.and_then(|p| i64::try_from(p).ok()),
        };
        if let Ok(mut ledger) = self.0.lock() {
            let _ = ledger.record_call(&record);
        }
    }
}

/// A source with no plugins, for routes built without a registry.
#[cfg(test)]
pub(crate) struct NoPlugins;

#[cfg(test)]
impl PluginSource for NoPlugins {
    fn resolve(&self, id: &str) -> Result<Arc<Resolved>, Unpriced> {
        Err(refused(404, "plugin_not_found", format!("no plugin {id}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::x402::decode_invoice;
    use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
    use openagents_x402::front::{Config, Front, Route};
    use openagents_x402::server::{Receiver, Request, Response};
    use openagents_x402::wire::decode_payment_required;
    use openagents_x402::{
        Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_SIGNATURE, PaymentPayload,
    };
    use serde_json::Map;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NODE: [u8; 32] = [9; 32];

    fn to_hex(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
    const AUTHOR: &str = "a7cff3ee1ff0209f971b9f24673db310ab858899c9d9a99b640e6cb29b1753f0";
    const ID: &str =
        "a7cff3ee1ff0209f971b9f24673db310ab858899c9d9a99b640e6cb29b1753f0:explain-error";
    const RELEASE: &str = "5e1ea5e5";
    /// After the v1 split rule takes effect.
    const NOW: u64 = 1_792_022_400 + 60;
    const ERROR: &str = "error[E0425]: cannot find value `totl` in this scope\n --> src/main.rs:3:13\n  |\n3 |     println!(\"{}\", totl);\n  |                    ^^^^ help: a local variable with a similar name exists: `total`\n";

    /// A receiver that signs real invoices with a test key and keeps each
    /// preimage, so the test buyer "pays" by reading it back.
    struct FakeReceiver {
        counter: AtomicU64,
        preimages: Mutex<HashMap<String, [u8; 32]>>,
    }

    impl FakeReceiver {
        fn pay(&self, invoice: &str) -> String {
            let hash = to_hex(decode_invoice(invoice).unwrap().payment_hash());
            to_hex(self.preimages.lock().unwrap()[&hash])
        }
    }

    impl Receiver for FakeReceiver {
        fn pay_to(&self) -> String {
            to_hex(payee_of(NODE))
        }
        fn invoice(
            &self,
            amount: u64,
            request_hash: [u8; 32],
            expiry: u32,
        ) -> Result<String, String> {
            let n = self.counter.fetch_add(1, Ordering::SeqCst);
            let mut seed = request_hash.to_vec();
            seed.extend(n.to_be_bytes());
            let preimage: [u8; 32] = Sha256::digest(&seed).into();
            let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
            let mut fields = tag(1, &words(&payment_hash));
            fields.extend(tag(16, &words(&[2; 32])));
            fields.extend(tag(23, &words(&request_hash)));
            fields.extend(tag(6, &number(u64::from(expiry))));
            let hrp = format!("lnbc{}n", amount / 100);
            let invoice = signed_by(NODE, &hrp, fields, false, false, NOW);
            self.preimages
                .lock()
                .unwrap()
                .insert(to_hex(payment_hash), preimage);
            Ok(invoice)
        }
        fn received_msat(&self, _: [u8; 32]) -> Result<Option<u64>, String> {
            Ok(None)
        }
    }

    /// The registry, faked: one published plugin, `explain-error`, with a
    /// 1-sat author fee.
    struct OnePlugin(Arc<Resolved>);

    impl PluginSource for OnePlugin {
        fn resolve(&self, id: &str) -> Result<Arc<Resolved>, Unpriced> {
            if id == self.0.id || id == "explain-error" {
                Ok(Arc::clone(&self.0))
            } else {
                Err(refused(404, "plugin_not_found", format!("no plugin {id}")))
            }
        }
    }

    fn explain_error_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-explain-error")
    }

    fn explain_error(fee_msat: u64) -> Arc<Resolved> {
        Arc::new(Resolved {
            id: ID.into(),
            release: RELEASE.into(),
            author: AUTHOR.into(),
            fee_msat,
            packet: packet(&explain_error_dir()).unwrap(),
        })
    }

    fn front(
        dir: &Path,
        receiver: Arc<FakeReceiver>,
        sink: Arc<LedgerSink>,
    ) -> Front<FileReplayStore> {
        let spec: crate::pay::RouteFile = crate::pay::RouteFile::parse(
            "public_url = \"https://api.example.com\"\n[[route]]\nid = \"invoke\"\npath = \"/v1/plugins/{id}/invoke\"\nprice_sats = 5\nregistry = \"wss://relay.example\"\n",
            dir,
        )
        .unwrap();
        let route: Route = spec.routes[0]
            .route_with(|_| Arc::new(OnePlugin(explain_error(1_000))))
            .unwrap();
        assert_eq!(route.role, ROLE);
        Front::new(
            Config {
                base_url: "https://api.example.com".into(),
                network: nostr::x402::MAINNET,
                realm: "api.example.com".into(),
                challenge_key: vec![7; 32],
                timeout_secs: 300,
            },
            receiver,
            Facilitator::new(FileReplayStore::open(dir).unwrap(), 60),
            sink,
            vec![route],
        )
        .unwrap()
    }

    fn post(target: &str, body: &str, headers: Vec<(String, String)>) -> Request {
        Request {
            method: "POST".into(),
            target: target.into(),
            headers,
            body: body.as_bytes().to_vec(),
        }
    }

    fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
        response
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn json_body(response: &Response) -> Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    #[test]
    fn a_paid_invocation_runs_the_pinned_release_and_the_author_gets_the_fee() {
        let dir = tempfile::tempdir().unwrap();
        let receiver = Arc::new(FakeReceiver {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
        });
        let sink = Arc::new(LedgerSink::in_memory());
        let front = front(dir.path(), receiver.clone(), sink.clone());
        let target = format!("/v1/plugins/{ID}/invoke");

        // No proof: a 402 for the endpoint price plus the author's fee.
        let (challenge, event) = front.handle(&post(&target, ERROR, vec![]), NOW);
        assert_eq!(
            challenge.status,
            402,
            "{}",
            String::from_utf8_lossy(&challenge.body)
        );
        assert_eq!(event.outcome, "challenged");
        let body = json_body(&challenge);
        assert_eq!(body["price_msat"], 6_000);
        assert_eq!(
            body["price_parts"][0],
            json!({"name": "endpoint", "msat": 5_000})
        );
        assert_eq!(
            body["price_parts"][1],
            json!({"name": "author_fee", "msat": 1_000})
        );
        assert_eq!(body["release"], RELEASE);
        let detail = body["detail"].as_str().unwrap();
        assert!(
            detail.contains("6 sats (endpoint 5 sats + author fee 1 sat)"),
            "{detail}"
        );

        // Pay the x402 terms and retry.
        let required =
            decode_payment_required(header(&challenge, PAYMENT_REQUIRED).unwrap()).unwrap();
        let accepted = required.accepts[0].clone();
        assert_eq!(accepted.amount, "6000");
        let invoice = accepted.extra["invoice"].as_str().unwrap().to_string();
        let mut proof = Map::new();
        proof.insert("preimage".into(), json!(receiver.pay(&invoice)));
        let signature = openagents_x402::wire::encode_header(&PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted,
            payload: proof,
            extensions: None,
        })
        .unwrap();
        let (paid, event) = front.handle(
            &post(&target, ERROR, vec![(PAYMENT_SIGNATURE.into(), signature)]),
            NOW,
        );
        assert_eq!(paid.status, 200, "{}", String::from_utf8_lossy(&paid.body));
        assert_eq!(event.outcome, "executed");
        let ran = json_body(&paid);
        assert_eq!(ran["plugin"], ID);
        assert_eq!(ran["release"], RELEASE);
        assert_eq!(ran["status"], "ok");
        assert_eq!(ran["receipt"]["profile"], "snapshot-read");
        assert_eq!(ran["receipt"]["outcome"]["type"], "value");
        assert!(
            ran["value"].to_string().contains("totl"),
            "{}",
            ran["value"]
        );

        // The ledger: one settlement for this release, the fee to the author,
        // and both calls (the challenge and the paid one) as call records.
        sink.with(|ledger| {
            let settled = ledger.since(0).unwrap();
            assert_eq!(settled.len(), 1);
            let author_share: Vec<_> = settled[0]
                .shares
                .iter()
                .filter(|share| share.role == "author")
                .collect();
            assert_eq!(author_share.len(), 1);
            assert_eq!(author_share[0].party, AUTHOR);
            assert_eq!(author_share[0].amount_msat, 1_000);
            assert!(ledger.accrued(AUTHOR).unwrap() >= 1_000);
            assert_eq!(settled[0].plugin_id.as_deref(), Some(ID));
            assert_eq!(settled[0].release_id.as_deref(), Some(RELEASE));
            assert_eq!(settled[0].price_msat, 6_000);
            let calls = ledger.calls_since(0).unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].1.outcome, "challenged");
            assert!(!calls[0].1.paid);
            assert_eq!(calls[1].1.outcome, "executed");
            assert!(calls[1].1.paid);
            assert_eq!(calls[1].1.release_id.as_deref(), Some(RELEASE));
        });
    }

    #[test]
    fn an_unknown_plugin_is_a_404_and_never_a_402() {
        let dir = tempfile::tempdir().unwrap();
        let receiver = Arc::new(FakeReceiver {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
        });
        let sink = Arc::new(LedgerSink::in_memory());
        let front = front(dir.path(), receiver, sink.clone());
        let (response, event) = front.handle(&post("/v1/plugins/nobody/invoke", "x", vec![]), NOW);
        assert_eq!(response.status, 404);
        assert_eq!(json_body(&response)["error"]["type"], "plugin_not_found");
        assert_eq!(event.outcome, "unpriced");
        sink.with(|ledger| {
            assert!(ledger.since(0).unwrap().is_empty());
            let calls = ledger.calls_since(0).unwrap();
            assert_eq!(calls.len(), 1);
            assert!(!calls[0].1.paid);
        });
    }

    fn package_with(program: &Value) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let text = serde_json::to_string_pretty(program).unwrap();
        std::fs::create_dir_all(dir.path().join("programs")).unwrap();
        std::fs::write(dir.path().join("programs/demo.json"), &text).unwrap();
        let package = json!({
            "v": 1, "slug": "demo", "name": "Demo", "summary": "A demo.",
            "version": "0.1.0", "publisher": AUTHOR, "provenance": "test",
            "program": {"name": "demo", "digest": coder::package::digest(&text)},
        });
        std::fs::write(dir.path().join("package.json"), package.to_string()).unwrap();
        dir
    }

    #[test]
    fn only_a_single_guest_step_is_invocable() {
        let explain: Value = serde_json::from_slice(
            &std::fs::read(explain_error_dir().join("programs/explain-error.json")).unwrap(),
        )
        .unwrap();
        // The real explain-error packet loads, as a snapshot-read guest.
        let loaded = packet(&explain_error_dir()).unwrap();
        assert_eq!(loaded.profile, plugin::Profile::SnapshotRead);
        assert_eq!(loaded.request_key.as_deref(), Some("text"));

        let mut effectful = explain.clone();
        effectful["definition"]["steps"][0]["kind"] = json!("tool");
        let refusal = packet(package_with(&effectful).path()).err().unwrap();
        assert_eq!(refusal.kind, "plugin_not_invocable");
        assert_eq!(refusal.status, 422);

        let mut requiring = explain.clone();
        requiring["definition"]["requires"] = json!(["write"]);
        let refusal = packet(package_with(&requiring).path()).err().unwrap();
        assert_eq!(refusal.kind, "plugin_not_invocable");

        let missing = tempfile::tempdir().unwrap();
        assert_eq!(
            packet(missing.path()).err().unwrap().kind,
            "plugin_not_invocable"
        );
    }

    #[test]
    fn a_registry_route_needs_an_id_segment_and_no_other_executor() {
        let bad = |route: &str| {
            let text = format!("public_url = \"https://a.example\"\n[[route]]\n{route}");
            crate::pay::RouteFile::parse(&text, Path::new("."))
                .and_then(|file| {
                    file.routes[0]
                        .route_with(|_| Arc::new(NoPlugins))
                        .map(|_| ())
                })
                .unwrap_err()
        };
        assert!(
            bad("id = \"a\"\npath = \"/v1/plugins/x/invoke\"\nprice_sats = 1\nregistry = \"wss://r\"")
                .contains("{id}")
        );
        assert!(
            bad("id = \"a\"\npath = \"/v1/plugins/{id}/invoke\"\nprice_sats = 1\nregistry = \"wss://r\"\ncommand = [\"x\"]")
                .contains("exactly one")
        );
        assert!(
            bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1\ncommand = [\"x\"]\nblossom = \"https://b\"")
                .contains("blossom")
        );
    }
}
