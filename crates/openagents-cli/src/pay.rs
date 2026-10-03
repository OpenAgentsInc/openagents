//! `openagents pay serve`: the multi-route pay front. One process holds the
//! wallet (the `payTo` node), the replay store, and the settlement log, and
//! sells every route in a TOML file for an exact Lightning payment, in x402
//! v2 and the HTTP `Payment` scheme on the same invoice
//! (`openagents_x402::front`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use openagents_wallet::{LightningWallet, PaymentDirection, PaymentStatus};
use openagents_x402::front::{
    Call, Config, Front, NdjsonSettlements, Output as Served, Price, Route, RouteExecutor,
    SettlementSink,
};
use openagents_x402::server::Receiver;
use openagents_x402::{FileReplayStore, ReplayStore, network_id};
use serde::Deserialize;
use serde_json::json;

use crate::pay_hosted::{Hosted, HostedSpec};
use crate::pay_plugin::{Invoke, LedgerSink, PluginSource, ROLE, RegistrySource};
use crate::x402::{Node, fail_wallet, open_wallet, replay_dir, toll_floor, x402_home};
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents pay COMMAND [OPTIONS]
  serve --routes FILE [--listen HOST:PORT] [--seconds N]
                          Sell every route in FILE (TOML) from this
                          computer's wallet: one listener, one replay store,
                          one settlement log. A request without a proof gets
                          a 402 carrying one invoice two ways: x402 v2
                          (PAYMENT-REQUIRED) and the HTTP Payment scheme
                          (WWW-Authenticate: Payment, lightning/charge, the
                          body's digest bound). A valid proof, by either
                          scheme, is consumed once across every route, the
                          settlement is appended to the log, and only then
                          does the route run: a command (body on stdin), a
                          plugin's workflow (body as the request), an
                          upstream HTTP service (body forwarded), or a
                          published plugin named by the path. If the log
                          cannot be written the call gets a 503, nothing runs,
                          and the same proof stays good for a retry.
  payouts --ledger FILE [--relay URL] [--interval SECS] [--spark-home DIR] [--once]
                          Pay the ledger's accrued shares out from this
                          computer's wallet, every SECS (default 60): per
                          payee, once its owed amount reaches 100 sats (Spark
                          address) or 1,000 sats (Lightning address), or once
                          its oldest share is a day old. The destination is
                          the signed release's payout, then the payee's NIP-A3
                          Spark address, then its profile's lud16, read from
                          the relay; a payee with none stays owed. Lightning
                          addresses are paid by LNURL-pay from the wallet
                          within a 1% fee cap; Spark addresses from the payout
                          Spark wallet in DIR (default
                          /var/lib/openagents-pay/spark), topped up from the
                          wallet. Each payout's payment hash or transfer id is
                          written before it is sent; after a restart an
                          interrupted payout is settled only by looking it up,
                          never sent again. A failed payout returns its
                          shares and the payee backs off (5 min, doubling, at
                          most 6 h). OPENAGENTS_PAY_LEDGER and
                          OPENAGENTS_PAY_SPARK_HOME stand in for the flags.
  payout-list --ledger FILE [--open]
                          List payouts: state, rail, amounts, and the wallet
                          reference; --open lists planned, sending, and
                          unknown ones only.
  reconcile --ledger FILE [--spark-home DIR] [--report-dir DIR] [--resolve]
                          Check the ledger against the receiver wallet (through
                          the running node's control.sock) and the payout
                          Spark wallet: every Lightning settlement is a
                          succeeded inbound payment of its amount, every sent
                          payout a succeeded outbound record, no outbound
                          payment is unexplained, no payout stays unknown,
                          and holdings cover what the ledger owes. Prints the
                          report (state ok, drift, or unknown); --report-dir
                          writes latest.json/.txt and daily/DATE.json/.txt
                          there; --resolve settles an unknown payout whose
                          wallet record proves it sent or failed. Nothing is
                          ever sent. A drift is also an error line on stderr.
  payout-spark-init [--spark-home DIR]
                          Make a fresh seed for the payout Spark wallet in DIR
                          and print its Spark address (never the seed).
The route file:
  public_url = \"https://api.openagents.com\"   # routes bind this + path
  listen = \"127.0.0.1:8402\"                    # optional
  realm = \"api.openagents.com\"                 # optional; default the host
  expiry_secs = 300                            # optional
  challenge_key_file = \"/path/key\"             # optional
  settlements = \"/path/settlements.ndjson\"     # optional
  ledger = \"/path/ledger.sqlite\"               # optional: crates/pay-ledger
                                               # instead of the NDJSON log
  plugin_cache = \"/path/plugins\"               # optional
  [[route]]
  id = \"messages\"            path = \"/v1/messages\"     method = \"POST\"
  price_sats = 21            # or price_msat
  role = \"endpoint\"          resource = \"api:messages\" # optional
  plugin = \"explain-error\"   # optional, the plugin a plugin_call sells
  description = \"...\"        mime = \"application/json\" # optional
  command = [\"prog\", \"arg\"]  # or plugin_dir = \"DIR\" [workspace = \"DIR\"]
                             # or upstream = \"http://host/path/{id}\"
                             # or registry = \"wss://relay\" [blossom = URL]
A registry route (path with {id}, such as /v1/plugins/{id}/invoke) sells
the published plugin {id} names: its newest signed release, priced at the
route's price plus the release's fee_msat (the 402 names both parts), run
once as a sandboxed guest with the body as the request, the fee recorded
as the author's share.
A [hosted] section sells author-hosted resources at GET and POST
/x/{resource} (#10194): an author registers an upstream with
`openagents x402 publish` (POST /v1/resources, signed by their key); the
front sells calls with its own 402 and invoice, records the owner's share
(the rule's [hosted_resource] split), then forwards the paid request to the
upstream with an OpenAgents-Paid header signed by the pay host key
(published at GET /v1/paid-key). The author runs no wallet.
  [hosted]
  registry = \"/path/hosted.ndjson\"     # optional
  key_file = \"/path/paid-header.key\"   # optional; made on first use
  schemes = [\"https\"]                  # optional; https and http only
  timeout_secs = 30                      # optional
  max_request_bytes = 1048576            # optional
  max_response_bytes = 4194304           # optional
An upstream that resolves to a loopback, private, link-local, or other
non-public address is refused at registration and at every call; it is
called at the checked address with no redirects and no proxy.
A path segment {name} matches any one segment and reaches the command as
OPENAGENTS_PAY_PARAM_NAME and an upstream URL as {name}. Defaults: the
replay store ~/.openagents/x402/replay, the challenge key
~/.openagents/x402/payment-challenge.key (made on first use), the
settlements ~/.openagents/x402/settlements.ndjson. Every process that
settles for this wallet must share all three.";

#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("serve", Effect::LongRunning, "wallet"),
    Declared::screen("payouts", Effect::Spends, "wallet"),
    Declared::screen("payout-list", Effect::ReadOnly, "wallet"),
    Declared::screen("reconcile", Effect::LocalWrite, "wallet"),
    Declared::screen("payout-spark-init", Effect::Secret, "wallet"),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("pay", "a command is required", USAGE);
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "serve" => serve(output, rest),
        "payouts" => crate::pay_payout::payouts(output, rest, USAGE),
        "payout-list" => crate::pay_payout::list(output, rest, USAGE),
        "reconcile" => crate::pay_reconcile::reconcile(output, rest, USAGE),
        "payout-spark-init" => crate::pay_payout::spark_init(output, rest, USAGE),
        other => output.usage("pay", &format!("unknown command `{other}`"), USAGE),
    }
}

/// The route file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RouteFile {
    pub public_url: String,
    pub listen: Option<String>,
    pub realm: Option<String>,
    pub expiry_secs: Option<u32>,
    pub challenge_key_file: Option<PathBuf>,
    pub settlements: Option<PathBuf>,
    pub ledger: Option<PathBuf>,
    pub plugin_cache: Option<PathBuf>,
    pub hosted: Option<HostedSpec>,
    #[serde(rename = "route", default)]
    pub routes: Vec<RouteSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RouteSpec {
    pub id: String,
    pub path: String,
    #[serde(default = "post")]
    pub method: String,
    pub price_sats: Option<u64>,
    pub price_msat: Option<u64>,
    #[serde(default = "endpoint")]
    pub role: String,
    pub resource: Option<String>,
    pub plugin: Option<String>,
    pub description: Option<String>,
    pub mime: Option<String>,
    pub command: Option<Vec<String>>,
    pub plugin_dir: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
    pub upstream: Option<String>,
    pub registry: Option<String>,
    pub blossom: Option<String>,
}

fn post() -> String {
    "POST".into()
}

fn endpoint() -> String {
    "endpoint".into()
}

impl RouteFile {
    pub(crate) fn parse(text: &str, base: &Path) -> Result<Self, String> {
        let mut file: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        // Relative paths in the file are relative to the file.
        let anchor = |path: &mut PathBuf| {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        };
        if let Some(path) = &mut file.challenge_key_file {
            anchor(path);
        }
        if let Some(path) = &mut file.settlements {
            anchor(path);
        }
        if let Some(path) = &mut file.ledger {
            anchor(path);
        }
        if let Some(path) = &mut file.plugin_cache {
            anchor(path);
        }
        if let Some(hosted) = &mut file.hosted {
            hosted.anchor(base);
        }
        for route in &mut file.routes {
            if let Some(path) = &mut route.plugin_dir {
                anchor(path);
            }
            if let Some(path) = &mut route.workspace {
                anchor(path);
            }
        }
        Ok(file)
    }

    pub(crate) fn realm(&self) -> String {
        self.realm.clone().unwrap_or_else(|| {
            let rest = self
                .public_url
                .split_once("://")
                .map_or(self.public_url.as_str(), |(_, rest)| rest);
            rest.split(['/', '?', '#'])
                .next()
                .unwrap_or_default()
                .to_string()
        })
    }
}

impl RouteSpec {
    pub(crate) fn price_msat(&self) -> Result<u64, String> {
        match (self.price_sats, self.price_msat) {
            (Some(sats), None) => sats
                .checked_mul(1000)
                .ok_or_else(|| format!("route {}: price_sats is too large", self.id)),
            (None, Some(msat)) => Ok(msat),
            _ => Err(format!(
                "route {}: give exactly one of price_sats and price_msat",
                self.id
            )),
        }
    }

    fn executor(&self) -> Result<Arc<dyn RouteExecutor>, String> {
        if self.registry.is_some() {
            return Err(format!(
                "route {}: give exactly one of command, plugin_dir, upstream, and registry",
                self.id
            ));
        }
        match (&self.command, &self.plugin_dir, &self.upstream) {
            (Some(argv), None, None) => {
                let Some((program, args)) = argv.split_first() else {
                    return Err(format!("route {}: command is empty", self.id));
                };
                Ok(Arc::new(CommandExec {
                    program: program.clone(),
                    args: args.to_vec(),
                }))
            }
            (None, Some(dir), None) => Ok(Arc::new(PluginExec {
                dir: dir.clone(),
                workspace: self.workspace.clone().unwrap_or_else(|| PathBuf::from(".")),
            })),
            (None, None, Some(url)) => {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(format!(
                        "route {}: upstream must be an http(s) URL",
                        self.id
                    ));
                }
                Ok(Arc::new(UpstreamExec {
                    url: url.clone(),
                    client: reqwest::blocking::Client::builder()
                        .timeout(Duration::from_secs(120))
                        .build()
                        .map_err(|e| e.to_string())?,
                }))
            }
            _ => Err(format!(
                "route {}: give exactly one of command, plugin_dir, upstream, and registry",
                self.id
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn route(&self) -> Result<Route, String> {
        self.route_with(|_| Arc::new(crate::pay_plugin::NoPlugins))
    }

    /// The route, with `source` resolving a registry route's plugins.
    pub(crate) fn route_with(
        &self,
        source: impl FnOnce(&str) -> Arc<dyn PluginSource>,
    ) -> Result<Route, String> {
        if self.workspace.is_some() && self.plugin_dir.is_none() {
            return Err(format!("route {}: workspace needs plugin_dir", self.id));
        }
        if self.blossom.is_some() && self.registry.is_none() {
            return Err(format!("route {}: blossom needs registry", self.id));
        }
        let (price, executor, role): (Price, Arc<dyn RouteExecutor>, String) = match &self.registry
        {
            Some(relay) => {
                if self.command.is_some() || self.plugin_dir.is_some() || self.upstream.is_some() {
                    return Err(format!(
                        "route {}: give exactly one of command, plugin_dir, upstream, and registry",
                        self.id
                    ));
                }
                if !self.path.split('/').any(|segment| segment == "{id}") {
                    return Err(format!(
                        "route {}: a registry route's path names the plugin with {{id}}",
                        self.id
                    ));
                }
                let invoke = Invoke::new(self.price_msat()?, source(relay));
                (invoke.price(), invoke, ROLE.to_owned())
            }
            None => (
                Price::Fixed(self.price_msat()?),
                self.executor()?,
                self.role.clone(),
            ),
        };
        Ok(Route {
            id: self.id.clone(),
            method: self.method.clone(),
            path: self.path.clone(),
            price,
            executor,
            role,
            resource: self
                .resource
                .clone()
                .unwrap_or_else(|| format!("route:{}", self.id)),
            plugin: self.plugin.clone(),
            description: self
                .description
                .clone()
                .unwrap_or_else(|| format!("{} over openagents pay", self.id)),
            model_cost_only: false,
            mime_type: self
                .mime
                .clone()
                .unwrap_or_else(|| "application/octet-stream".into()),
        })
    }
}

impl RouteFile {
    /// Where a registry route keeps the releases it fetched.
    pub(crate) fn plugin_cache(&self) -> PathBuf {
        self.plugin_cache
            .clone()
            .unwrap_or_else(|| x402_home().join("plugins"))
    }

    /// Every route, with registry routes resolving on the relay they name.
    pub(crate) fn routes(&self) -> Result<Vec<Route>, String> {
        let cache = self.plugin_cache();
        self.routes
            .iter()
            .map(|spec| {
                spec.route_with(|relay| {
                    Arc::new(RegistrySource::new(
                        relay.to_owned(),
                        spec.blossom.clone(),
                        cache.clone(),
                    ))
                })
            })
            .collect()
    }
}

/// The front a route file describes, on `receiver`, `store`, and `sink`,
/// with `hosted`'s routes after the file's.
pub(crate) fn front<S: ReplayStore>(
    file: &RouteFile,
    network: &'static str,
    challenge_key: Vec<u8>,
    receiver: Arc<dyn Receiver>,
    store: S,
    sink: Arc<dyn SettlementSink>,
    hosted: Option<&Arc<Hosted>>,
) -> Result<Front<S>, String> {
    let mut routes = file.routes()?;
    if let Some(hosted) = hosted {
        routes.extend(hosted.routes());
    }
    Front::new(
        Config {
            base_url: file.public_url.clone(),
            network,
            realm: file.realm(),
            challenge_key,
            timeout_secs: file.expiry_secs.unwrap_or(300),
        },
        receiver,
        openagents_x402::Facilitator::new(store, nostr::x402::DEFAULT_CLOCK_SKEW),
        sink,
        routes,
    )
}

/// Read the challenge key at `path`, or make 32 random bytes there (0600).
pub(crate) fn challenge_key(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    match std::fs::read(path) {
        Ok(key) if key.len() >= 32 => return Ok(key),
        Ok(_) => {
            return Err(format!(
                "{}: the key is shorter than 32 bytes",
                path.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("{}: {error}", path.display())),
    }
    let mut key = vec![0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut key))
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(&key).and_then(|()| file.sync_all())
        });
    match written {
        Ok(()) => Ok(key),
        // Another process made it first; use theirs.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

struct CommandExec {
    program: String,
    args: Vec<String>,
}

impl RouteExecutor for CommandExec {
    fn execute(&self, call: &Call<'_>) -> Result<Served, String> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .env("OPENAGENTS_PAY_ROUTE", call.route)
            .env(
                "OPENAGENTS_PAY_PAYMENT_HASH",
                call.payment_hash.unwrap_or_default(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (name, value) in call.params {
            command.env(
                format!("OPENAGENTS_PAY_PARAM_{}", name.to_ascii_uppercase()),
                value,
            );
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("spawn {}: {error}", self.program))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&call.request.body);
        }
        let done = child.wait_with_output().map_err(|e| e.to_string())?;
        if done.status.success() {
            Ok(Served {
                body: done.stdout,
                content_type: None,
            })
        } else {
            Err(format!("{} exited with {}", self.program, done.status))
        }
    }
}

/// A plugin's workflow, run once with the body as the request, as
/// `openagents plugin run` runs it.
struct PluginExec {
    dir: PathBuf,
    workspace: PathBuf,
}

impl RouteExecutor for PluginExec {
    fn execute(&self, call: &Call<'_>) -> Result<Served, String> {
        let request = std::str::from_utf8(&call.request.body)
            .map_err(|_| "the request body is not UTF-8".to_string())?;
        let ran = crate::ext_run::execute(&self.dir, &self.workspace, request)?;
        if ran["finished"].as_bool() != Some(true) {
            return Err(format!(
                "the plugin did not finish: {}",
                ran["reply"].as_str().unwrap_or_default()
            ));
        }
        Ok(Served {
            body: ran.to_string().into_bytes(),
            content_type: Some("application/json".into()),
        })
    }
}

struct UpstreamExec {
    url: String,
    client: reqwest::blocking::Client,
}

impl UpstreamExec {
    fn url_for(&self, call: &Call<'_>) -> String {
        let mut url = self.url.clone();
        for (name, value) in call.params {
            url = url.replace(&format!("{{{name}}}"), value);
        }
        if let Some((_, query)) = call.request.target.split_once('?') {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(query);
        }
        url
    }
}

impl RouteExecutor for UpstreamExec {
    fn execute(&self, call: &Call<'_>) -> Result<Served, String> {
        let method = reqwest::Method::from_bytes(call.request.method.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut request = self
            .client
            .request(method, self.url_for(call))
            .header("openagents-pay-route", call.route)
            .header(
                "openagents-pay-payment-hash",
                call.payment_hash.unwrap_or_default(),
            )
            .body(call.request.body.clone());
        for name in ["content-type", "accept"] {
            if let Some(value) = call.request.header(name) {
                request = request.header(name, value);
            }
        }
        let response = request.send().map_err(|e| e.to_string())?;
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = response.bytes().map_err(|e| e.to_string())?.to_vec();
        if !status.is_success() {
            return Err(format!("upstream answered {status}"));
        }
        Ok(Served { body, content_type })
    }
}

impl Node {
    /// The inbound amount the wallet recorded for `payment_hash`, waiting
    /// briefly for the claim to be recorded after the preimage was shown.
    fn received(&self, payment_hash: [u8; 32]) -> Result<Option<u64>, String> {
        for attempt in 0..10 {
            let record = self.0.lookup(payment_hash).map_err(|e| e.to_string())?;
            if let Some(record) = record
                && record.direction == PaymentDirection::Inbound
                && record.status == PaymentStatus::Succeeded
            {
                return Ok(record.amount_msat);
            }
            if attempt < 9 {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        Ok(None)
    }
}

/// The wallet as the front's receiver, with `received_msat` from `lookup`.
struct Wallet(Node);

impl Receiver for Wallet {
    fn pay_to(&self) -> String {
        self.0.pay_to()
    }
    fn invoice(&self, amount: u64, request_hash: [u8; 32], expiry: u32) -> Result<String, String> {
        self.0.invoice(amount, request_hash, expiry)
    }
    fn received_msat(&self, payment_hash: [u8; 32]) -> Result<Option<u64>, String> {
        self.0.received(payment_hash)
    }
}

fn serve(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("pay", &message, USAGE),
    };
    let Some(routes_path) = args.option("routes").map(PathBuf::from) else {
        return output.usage("pay", "serve needs --routes FILE", USAGE);
    };
    let seconds: u64 = match args.number("seconds", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage("pay", &message, USAGE),
    };
    let text = match std::fs::read_to_string(&routes_path) {
        Ok(text) => text,
        Err(error) => return output.fail("pay", &format!("{}: {error}", routes_path.display())),
    };
    let base = routes_path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let file = match RouteFile::parse(&text, &base) {
        Ok(file) => file,
        Err(message) => {
            return output.fail("pay", &format!("{}: {message}", routes_path.display()));
        }
    };
    let listen = args
        .option("listen")
        .map(str::to_string)
        .or_else(|| file.listen.clone())
        .unwrap_or_else(|| "127.0.0.1:8402".into());
    let key_path = file
        .challenge_key_file
        .clone()
        .unwrap_or_else(|| x402_home().join("payment-challenge.key"));
    let key = match challenge_key(&key_path) {
        Ok(key) => key,
        Err(message) => return output.fail("pay", &message),
    };
    let settlements_path = file
        .settlements
        .clone()
        .unwrap_or_else(|| x402_home().join("settlements.ndjson"));
    let ledger = match &file.ledger {
        Some(path) => match LedgerSink::open(path) {
            Ok(sink) => Some(Arc::new(sink)),
            Err(message) => return output.fail("pay", &message),
        },
        None => None,
    };
    let sink: Arc<dyn SettlementSink> = match &ledger {
        Some(ledger) => ledger.clone(),
        None => match NdjsonSettlements::open(&settlements_path) {
            Ok(sink) => Arc::new(sink),
            Err(message) => return output.fail("pay", &message),
        },
    };
    if let Err(message) = file.routes() {
        return output.fail("pay", &message);
    }
    let hosted = match &file.hosted {
        Some(spec) => match Hosted::open(spec, &file.public_url, ledger.clone()) {
            Ok(hosted) => Some(hosted),
            Err(message) => return output.fail("pay", &message),
        },
        None => None,
    };

    let (wallet, wallet_config) = match open_wallet() {
        Ok(opened) => opened,
        Err(error) => return fail_wallet(output, error),
    };
    let stop_wallet = |message: &str| {
        let _ = wallet.stop();
        output.fail("pay", message)
    };
    let Some(network) = network_id(wallet_config.network.as_str()) else {
        return stop_wallet(&format!(
            "x402 exact/lnbtc has no network for {}; init the wallet on bitcoin or testnet",
            wallet_config.network.as_str()
        ));
    };
    for spec in &file.routes {
        if let Err(message) = spec
            .price_msat()
            .and_then(|msat| toll_floor(&wallet_config, msat))
        {
            return stop_wallet(&format!("route {}: {message}", spec.id));
        }
    }
    let store = match FileReplayStore::open(&replay_dir()) {
        Ok(store) => store,
        Err(error) => return stop_wallet(&error.to_string()),
    };
    let listener = match std::net::TcpListener::bind(&listen) {
        Ok(listener) => listener,
        Err(error) => return stop_wallet(&format!("listen on {listen}: {error}")),
    };
    let wallet = Arc::new(wallet);
    let front = match front(
        &file,
        network,
        key,
        Arc::new(Wallet(Node(wallet.clone()))),
        store,
        sink,
        hosted.as_ref(),
    ) {
        Ok(front) => Arc::new(front),
        Err(message) => {
            let _ = wallet.stop();
            return output.fail("pay", &message);
        }
    };
    let routes: Vec<_> = front
        .routes()
        .iter()
        .map(|route| {
            let price = match route.price {
                Price::Fixed(msat) => msat,
                Price::Of(_) | Price::Quote(_) => 0,
            };
            json!({"id": route.id, "method": route.method, "path": route.path,
                   "price_msat": price, "role": route.role, "resource": route.resource})
        })
        .collect();
    output.line(
        &json!({
            "event": "serving",
            "listen": listener.local_addr().map(|a| a.to_string()).unwrap_or_default(),
            "public_url": file.public_url,
            "pay_to": wallet.node_id(),
            "network": network,
            "routes": routes,
            "replay_dir": replay_dir().display().to_string(),
            "settlements": file.ledger.as_ref().unwrap_or(&settlements_path).display().to_string(),
            "paid_key": hosted.as_ref().map(|h| h.host_pubkey().to_owned()),
        }),
        |v| {
            let mut text = format!(
                "serving {} routes at {} for {} (payTo {})",
                v["routes"].as_array().map_or(0, Vec::len),
                v["listen"].as_str().unwrap_or(""),
                v["public_url"].as_str().unwrap_or(""),
                v["pay_to"].as_str().unwrap_or("")
            );
            for route in v["routes"].as_array().into_iter().flatten() {
                text.push_str(&format!(
                    "\n  {} {} {} msat ({})",
                    route["method"].as_str().unwrap_or(""),
                    route["path"].as_str().unwrap_or(""),
                    route["price_msat"],
                    route["id"].as_str().unwrap_or("")
                ));
            }
            text
        },
    );

    let stop = Arc::new(AtomicBool::new(false));
    if seconds > 0 {
        let stop = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds));
            stop.store(true, Ordering::Relaxed);
        });
    }
    let log_output = *output;
    let log = move |event: &serde_json::Value| {
        log_output.line(event, |v| {
            format!(
                "{} {} -> {} {}{}",
                v["method"].as_str().unwrap_or(""),
                v["target"].as_str().unwrap_or(""),
                v["status"],
                v["outcome"].as_str().unwrap_or(""),
                v["error_reason"]
                    .as_str()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            )
        });
    };
    let served = serve_front(listener, front, hosted, stop, log);
    let stopped = wallet.stop();
    if let Err(error) = served {
        return output.fail("pay", &error.to_string());
    }
    match stopped {
        Ok(()) => 0,
        Err(error) => output.fail("pay", &error.to_string()),
    }
}

/// Serve `front`, with `hosted`'s registration and key endpoints before
/// its routes, until `stop` is set.
pub(crate) fn serve_front<S: ReplayStore + Send + Sync + 'static>(
    listener: std::net::TcpListener,
    front: Arc<Front<S>>,
    hosted: Option<Arc<Hosted>>,
    stop: Arc<AtomicBool>,
    log: impl Fn(&serde_json::Value) + Send + Sync + 'static,
) -> std::io::Result<()> {
    openagents_x402::server::serve_with(listener, stop, move |request| {
        let now = openagents_x402::unix_now();
        if let Some(hosted) = &hosted
            && let Some((response, outcome)) =
                hosted.answer(request, &front.pay_to(), front.config().network, now)
        {
            log(&json!({"method": request.method, "target": request.target,
                        "status": response.status, "outcome": outcome}));
            return response;
        }
        let (response, event) = front.handle(request, now);
        log(&serde_json::to_value(&event).unwrap_or_default());
        response
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_x402::server::Request;

    const FILE: &str = r#"
public_url = "https://api.example.com/"
settlements = "s.ndjson"

[[route]]
id = "messages"
path = "/v1/messages"
price_sats = 21
role = "endpoint"
resource = "api:messages"
mime = "text/plain"
command = ["sh", "-c", "printf '%s|%s|' \"$OPENAGENTS_PAY_ROUTE\" \"$OPENAGENTS_PAY_PARAM_ID\"; cat"]

[[route]]
id = "invoke"
method = "POST"
path = "/v1/plugins/{id}/invoke"
price_msat = 5000
role = "plugin_call"
plugin = "explain-error"
plugin_dir = "plugins/explain-error"

[[route]]
id = "weather"
method = "GET"
path = "/x/{city}"
price_sats = 3
role = "hosted_resource"
upstream = "http://127.0.0.1:9/weather/{city}"
"#;

    #[test]
    fn the_route_file_parses_into_routes() {
        let file = RouteFile::parse(FILE, Path::new("/etc/pay")).unwrap();
        assert_eq!(file.realm(), "api.example.com");
        assert_eq!(file.settlements, Some(PathBuf::from("/etc/pay/s.ndjson")));
        let routes: Vec<Route> = file.routes.iter().map(|r| r.route().unwrap()).collect();
        assert_eq!(routes.len(), 3);
        assert!(matches!(routes[0].price, Price::Fixed(21_000)));
        assert_eq!(routes[0].method, "POST");
        assert!(matches!(routes[1].price, Price::Fixed(5_000)));
        assert_eq!(routes[1].plugin.as_deref(), Some("explain-error"));
        assert_eq!(routes[1].resource, "route:invoke");
        assert_eq!(
            file.routes[1].plugin_dir,
            Some(PathBuf::from("/etc/pay/plugins/explain-error"))
        );
        assert_eq!(routes[2].role, "hosted_resource");
    }

    #[test]
    fn a_route_names_one_price_and_one_executor() {
        let bad = |route: &str| {
            let text = format!("public_url = \"https://a.example\"\n[[route]]\n{route}");
            RouteFile::parse(&text, Path::new("."))
                .and_then(|file| file.routes[0].route().map(|_| ()))
                .unwrap_err()
        };
        assert!(bad("id = \"a\"\npath = \"/a\"\ncommand = [\"x\"]").contains("price"));
        assert!(
            bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1\nprice_msat = 1000\ncommand = [\"x\"]")
                .contains("price")
        );
        assert!(bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1").contains("exactly one"));
        assert!(
            bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1\ncommand = [\"x\"]\nupstream = \"http://u\"")
                .contains("exactly one")
        );
        assert!(bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1\ncommand = []").contains("empty"));
        assert!(
            bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1\nupstream = \"ftp://u\"")
                .contains("http")
        );
        assert!(
            bad("id = \"a\"\npath = \"/a\"\nprice_sats = 1\ncommand = [\"x\"]\nmystery = 1")
                .contains("mystery")
        );
    }

    #[test]
    fn the_command_executor_gets_the_body_route_and_params() {
        let file = RouteFile::parse(FILE, Path::new(".")).unwrap();
        let route = file.routes[0].route().unwrap();
        let request = Request {
            method: "POST".into(),
            target: "/v1/messages".into(),
            headers: vec![],
            body: b"hello".to_vec(),
        };
        let params = vec![("id".to_string(), "p1".to_string())];
        let out = route
            .executor
            .execute(&Call {
                route: "messages",
                params: &params,
                request: &request,
                payment_hash: Some("ab"),
                provider_keys: None,
                quote: None,
            })
            .unwrap();
        assert_eq!(out.body, b"messages|p1|hello");
    }

    #[test]
    fn the_upstream_url_takes_params_and_the_query() {
        let exec = UpstreamExec {
            url: "http://127.0.0.1:9/weather/{city}".into(),
            client: reqwest::blocking::Client::new(),
        };
        let request = Request {
            method: "GET".into(),
            target: "/x/oslo?units=c".into(),
            headers: vec![],
            body: vec![],
        };
        let params = vec![("city".to_string(), "oslo".to_string())];
        let call = Call {
            route: "weather",
            params: &params,
            request: &request,
            payment_hash: None,
            provider_keys: None,
            quote: None,
        };
        assert_eq!(
            exec.url_for(&call),
            "http://127.0.0.1:9/weather/oslo?units=c"
        );
    }

    #[test]
    fn the_challenge_key_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k/key");
        let key = challenge_key(&path).unwrap();
        assert_eq!(key.len(), 32);
        assert_eq!(challenge_key(&path).unwrap(), key);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
