//! `openagents x402`: sell one operation over HTTP or MCP for an exact
//! Lightning payment, or buy one. Both roles use the wallet under
//! `openagents wallet`.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nostr::x402::{SupportedProfiles, binding_hash, http_binding, mcp_binding, validate_challenge};
use openagents_wallet::ldk::LdkWallet;
use openagents_wallet::{LightningWallet, WalletConfig, WalletError, config};
use openagents_x402::facilitator::{HTTP_ONLY, MCP_ONLY};
use openagents_x402::mcp::{
    BOUND_METADATA, Gate, PAYMENT_RESPONSE_META, PaidTools, payment_required_from_result,
    with_payment,
};
use openagents_x402::server::{Executor, Receiver, Resource};
use openagents_x402::{
    FileReplayStore, PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentPayload,
    SettlementResponse, network_id, wire,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::mcp::{Server, Toll};
use crate::relay::{Client, relay_url, signer_for};
use crate::{Args, Output};

const USAGE: &str = "usage: openagents x402 COMMAND [OPTIONS]
  serve --url PUBLIC_URL --msat N [--listen HOST:PORT] [--timeout SECONDS]
        [--mime TYPE] [--seconds N] -- CMD [ARGS...]
                          Sell CMD at PUBLIC_URL for exactly N msat per call
                          (x402 exact/lnbtc, http:1). A request without
                          PAYMENT-SIGNATURE gets a 402 with an invoice bound to
                          the method, URL, and body; a paid request runs CMD
                          with the body on stdin and returns stdout. Each
                          invoice settles once; a replay is duplicate_settlement.
  fetch URL [--method M] [--body FILE|-] --max-msat N [--max-fee-msat F]
        [--wait SECONDS] [--cap PUBKEY:SLUG] [--relay URL] [--show-proof]
                          Buy one call: read the 402, check the invoice against
                          this request, refuse above --max-msat, pay from the
                          wallet, retry with the preimage, print the body. With
                          --cap, resolve that NIP-CAP head first and refuse a
                          challenge whose payTo or URL it does not advertise.
  mcp-serve --server URI --msat N [--tool GROUP]... [--timeout SECONDS]
                          Serve `openagents mcp serve` over stdio with a toll
                          (x402 exact/lnbtc, mcp:1): a tools/call without
                          _meta[\"x402/payment\"] gets an error result carrying
                          PaymentRequired with an invoice bound to URI, the
                          tool name, and its arguments; a paid call runs and
                          returns its result with _meta[\"x402/payment-response\"].
                          --tool narrows the served groups. URI is the name
                          the buyer must bind to; it is not connected to.
  call TOOL [--arg WORD]... --max-msat N [--max-fee-msat F] [--wait SECONDS]
        [--server URI] [--cap PUBKEY:SLUG] [--relay URL] [--show-proof]
        -- CMD [ARGS...]
                          Buy one tools/call: start CMD as a stdio MCP server,
                          call TOOL with {\"args\": [WORD...]}, check the
                          challenge's invoice against this call and URI, refuse
                          above --max-msat, pay from the wallet, retry with the
                          proof, print the result. With --cap, URI defaults to
                          the advertised endpoint and the payTo must be one it
                          advertises.
  native-serve --slug SLUG --msat N [--timeout SECONDS] [--seconds N]
        [--as PROFILE] [--relay URL] -- CMD [ARGS...]
                          Sell CMD over the relay (x402 exact/lnbtc,
                          nostr:openagents:1): every record is a private kind
                          3188 artifact sealed to the other party. A request
                          record gets a challenge with an invoice bound to the
                          buyer, this key, the purchase nonce, and the request
                          bytes; a valid claim settles once, then CMD runs with
                          the input on stdin and its stdout is sealed back with
                          a status chain (offered, claim_pending, admitted,
                          running, completed or failed).
  buy PROVIDER --slug SLUG [--input FILE|-] --max-msat N [--max-fee-msat F]
        [--wait SECONDS] [--as PROFILE] [--relay URL] [--show-proof]
                          Buy one run: resolve PROVIDER:SLUG on the relay, seal
                          the input and a request to PROVIDER, check the
                          challenge against them, refuse above --max-msat, pay
                          from the wallet, seal the claim, follow the status
                          chain, print the output. A run that does not end
                          within --wait leaves the purchase for `status`; it
                          is never paid again.
  status PROVIDER PURCHASE [--wait SECONDS] [--as PROFILE] [--relay URL]
                          Ask PROVIDER for the status chain of PURCHASE and
                          print the newest status and any output.
  advertise --slug SLUG --merchant ID [--url PUBLIC_URL]
        [--binding http:1|mcp:1|nostr:openagents:1] [--relays URL]...
        [--summary TEXT] [--dry-run] [--as PROFILE] [--relay URL]
                          Publish (or print) the kind 30180 adapter definition
                          that advertises a paid resource of this wallet
                          (NIP-CAP feature oa-x402-v1) over one binding:
                          http:1 (default) or mcp:1 at PUBLIC_URL (the MCP
                          server URI), or nostr:openagents:1 answered by this
                          key on --relays (default: --relay), with recovery
                          native-record-v1.
Replay records live in ~/.openagents/x402/replay and native purchases in
~/.openagents/x402/native. The preimage is printed only with --show-proof.
Add --json before `x402` for one JSON document.";

const SWITCHES: &[&str] = &["show-proof", "dry-run"];

/// The schema both x402 adapter operations declare: opaque bytes in and out.
const BYTES_SCHEMA: &str = r#"{"type":"string","contentEncoding":"binary"}"#;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("x402", "a command is required", USAGE);
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "serve" => serve(output, rest),
        "fetch" => fetch(output, rest),
        "mcp-serve" => mcp_serve(output, rest),
        "call" => call(output, rest),
        "native-serve" => crate::x402_native::serve(output, rest),
        "buy" => crate::x402_native::buy(output, rest),
        "status" => crate::x402_native::status(output, rest),
        "advertise" => advertise(output, rest),
        other => output.usage("x402", &format!("unknown command `{other}`"), USAGE),
    }
}

pub(crate) fn replay_dir() -> PathBuf {
    match std::env::var_os("OPENAGENTS_X402_HOME") {
        Some(home) => PathBuf::from(home),
        None => config::home()
            .parent()
            .map(|p| p.join("x402"))
            .unwrap_or_else(|| PathBuf::from("x402")),
    }
    .join("replay")
}

pub(crate) fn open_wallet() -> Result<(LdkWallet, WalletConfig), WalletError> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let (mnemonic, _) = config::load_or_create_seed(&home, false, String::new)?;
    Ok((
        LdkWallet::open(&home, &wallet_config, &mnemonic)?,
        wallet_config,
    ))
}

pub(crate) fn fail_wallet(output: &Output, error: WalletError) -> u8 {
    match error {
        WalletError::Invalid(message) => output.usage("x402", &message, USAGE),
        other => output.fail("x402", &other.to_string()),
    }
}

pub(crate) struct Node(pub(crate) Arc<LdkWallet>);

impl Receiver for Node {
    fn pay_to(&self) -> String {
        self.0.node_id()
    }
    fn invoice(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<String, String> {
        self.0
            .receive_exact(amount_msat, request_hash, expiry_secs)
            .map(|issued| issued.bolt11)
            .map_err(|error| error.to_string())
    }
}

struct Command {
    program: String,
    args: Vec<String>,
}

impl Executor for Command {
    fn execute(&self, body: &[u8]) -> Result<Vec<u8>, String> {
        use std::io::Write;
        use std::process::{Command as Process, Stdio};
        let mut child = Process::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("spawn {}: {error}", self.program))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(body);
        }
        let done = child
            .wait_with_output()
            .map_err(|error| error.to_string())?;
        if done.status.success() {
            Ok(done.stdout)
        } else {
            Err(format!("{} exited with {}", self.program, done.status))
        }
    }
}

fn serve(output: &Output, words: &[String]) -> u8 {
    let (options, command) = match words.iter().position(|w| w == "--") {
        Some(index) => (&words[..index], &words[index + 1..]),
        None => return output.usage("x402", "serve needs `-- CMD [ARGS...]`", USAGE),
    };
    let Some((program, program_args)) = command.split_first() else {
        return output.usage("x402", "serve needs a command after `--`", USAGE);
    };
    let args = match Args::parse(options, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let Some(url) = args.option("url") else {
        return output.usage("x402", "serve needs --url PUBLIC_URL", USAGE);
    };
    let msat: u64 = match args.number("msat", 0) {
        Ok(0) => return output.usage("x402", "serve needs --msat N (positive)", USAGE),
        Ok(msat) => msat,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let timeout: u32 = match args.number("timeout", 300) {
        Ok(0) => return output.usage("x402", "--timeout must be positive", USAGE),
        Ok(timeout) => timeout,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let seconds: u64 = match args.number("seconds", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let listen = args.option("listen").unwrap_or("127.0.0.1:8402");
    let mime = args.option("mime").unwrap_or("application/octet-stream");
    if http_binding("GET", url, b"", &[]).is_err() {
        return output.usage(
            "x402",
            "--url must be an absolute http(s) URL without a fragment",
            USAGE,
        );
    }

    let (wallet, wallet_config) = match open_wallet() {
        Ok(opened) => opened,
        Err(error) => return fail_wallet(output, error),
    };
    let Some(network) = network_id(wallet_config.network.as_str()) else {
        let _ = wallet.stop();
        return output.fail(
            "x402",
            &format!(
                "x402 exact/lnbtc has no network for {}; init the wallet on bitcoin or testnet",
                wallet_config.network.as_str()
            ),
        );
    };
    let store = match FileReplayStore::open(&replay_dir()) {
        Ok(store) => store,
        Err(error) => {
            let _ = wallet.stop();
            return output.fail("x402", &error.to_string());
        }
    };
    let listener = match std::net::TcpListener::bind(listen) {
        Ok(listener) => listener,
        Err(error) => {
            let _ = wallet.stop();
            return output.fail("x402", &format!("listen on {listen}: {error}"));
        }
    };
    let wallet = Arc::new(wallet);
    let resource = Arc::new(Resource {
        url: url.to_string(),
        network,
        amount_msat: msat,
        timeout_secs: timeout,
        description: format!("{program} over openagents x402"),
        mime_type: mime.to_string(),
        receiver: Arc::new(Node(wallet.clone())),
        executor: Arc::new(Command {
            program: program.clone(),
            args: program_args.to_vec(),
        }),
        facilitator: openagents_x402::Facilitator::new(store, nostr::x402::DEFAULT_CLOCK_SKEW),
    });
    output.line(
        &json!({
            "event": "serving",
            "listen": listener.local_addr().map(|a| a.to_string()).unwrap_or_default(),
            "url": url,
            "pay_to": wallet.node_id(),
            "network": network,
            "amount_msat": msat,
            "timeout_secs": timeout,
            "replay_dir": replay_dir().display().to_string(),
        }),
        |v| {
            format!(
                "serving {} at {} for {} msat (payTo {})",
                v["url"].as_str().unwrap_or(""),
                v["listen"].as_str().unwrap_or(""),
                v["amount_msat"],
                v["pay_to"].as_str().unwrap_or("")
            )
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
    let served = openagents_x402::server::serve(listener, resource, stop, move |event| {
        log_output.line(&serde_json::to_value(event).unwrap_or_default(), |v| {
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
    });
    let stopped = wallet.stop();
    if let Err(error) = served {
        return output.fail("x402", &error.to_string());
    }
    match stopped {
        Ok(()) => 0,
        Err(error) => output.fail("x402", &error.to_string()),
    }
}

fn fetch(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let Some(url) = args.positional().first() else {
        return output.usage("x402", "fetch needs a URL", USAGE);
    };
    let method = args.option("method").unwrap_or("GET").to_ascii_uppercase();
    let body = match args.option("body") {
        None => Vec::new(),
        Some("-") => {
            let mut bytes = Vec::new();
            if let Err(error) = std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes) {
                return output.fail("x402", &format!("read stdin: {error}"));
            }
            bytes
        }
        Some(path) => match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => return output.fail("x402", &format!("read {path}: {error}")),
        },
    };
    let max_msat: u64 = match args.number("max-msat", 0) {
        Ok(0) => return output.usage("x402", "fetch needs --max-msat N (positive)", USAGE),
        Ok(max) => max,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let max_fee: u64 = match args.number("max-fee-msat", 0) {
        Ok(fee) => fee,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let wait: u64 = match args.number("wait", 60) {
        Ok(wait) => wait,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let show_proof = args.switch("show-proof");
    let descriptor = match args.option("cap") {
        None => None,
        Some(head) => match resolve_descriptor(head, args.option("relay"), args.option("as")) {
            Ok(descriptor) => Some(descriptor),
            Err(Refusal::Usage(message)) => return output.usage("x402", &message, USAGE),
            Err(Refusal::Failure(message)) => return output.fail("x402", &message),
        },
    };
    if let Some((definition, _)) = &descriptor
        && definition.endpoint != *url
    {
        return output.fail(
            "x402",
            &format!(
                "{url} is not the advertised endpoint {} of the named capability",
                definition.endpoint
            ),
        );
    }

    // Bind the request we are about to send, independently of the server.
    let request_hash = match http_binding(&method, url, &body, &[]).and_then(|b| binding_hash(&b)) {
        Ok(hash) => hash,
        Err(_) => {
            return output.usage(
                "x402",
                "URL must be absolute http(s) without a fragment and METHOD a token",
                USAGE,
            );
        }
    };

    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    let runtime = crate::runtime();
    let send = |signature: Option<String>| {
        let request = client
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).expect("method is a token"),
                url,
            )
            .body(body.clone());
        let request = match signature {
            Some(signature) => request.header(PAYMENT_SIGNATURE, signature),
            None => request,
        };
        runtime.block_on(async {
            let response = request.send().await?;
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let bytes = response.bytes().await?.to_vec();
            Ok::<_, reqwest::Error>((status, headers, bytes))
        })
    };

    let (status, headers, first_body) = match send(None) {
        Ok(reply) => reply,
        Err(error) => return output.fail("x402", &format!("{method} {url}: {error}")),
    };
    if status != 402 {
        return finish(
            output,
            status,
            &headers,
            &first_body,
            None,
            None,
            show_proof,
        );
    }
    let Some(required) = headers
        .get(PAYMENT_REQUIRED)
        .and_then(|v| v.to_str().ok())
        .map(wire::decode_payment_required)
    else {
        return output.fail("x402", "402 without a PAYMENT-REQUIRED header");
    };
    let required = match required {
        Ok(required) => required,
        Err(error) => return output.fail("x402", &format!("PAYMENT-REQUIRED: {error}")),
    };
    if required.resource.url != *url {
        return output.fail(
            "x402",
            "the challenge names a different resource URL than the one requested",
        );
    }
    let (payload, proof, amount) = match buy(
        &required,
        &request_hash,
        HTTP_ONLY,
        "http:1",
        descriptor.as_ref(),
        Budget {
            max_msat,
            max_fee,
            wait,
        },
    ) {
        Ok(bought) => bought,
        Err(message) => return output.fail("x402", &message),
    };
    let signature = match wire::encode_header(&payload) {
        Ok(signature) => signature,
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    let (status, headers, paid_body) = match send(Some(signature)) {
        Ok(reply) => reply,
        Err(error) => {
            return output.fail(
                "x402",
                &format!(
                    "paid retry failed: {error}; proof for payment {} is in `openagents wallet lookup`",
                    proof.payment_hash
                ),
            );
        }
    };
    finish(
        output,
        status,
        &headers,
        &paid_body,
        Some(&proof),
        Some(amount),
        show_proof,
    )
}

/// What the buyer will spend on one call.
pub(crate) struct Budget {
    pub(crate) max_msat: u64,
    pub(crate) max_fee: u64,
    pub(crate) wait: u64,
}

/// Pick the one requirement of `required` that is a valid exact/lnbtc
/// invoice for `request_hash` under `profiles`, pin it to the capability if
/// one was named, and pay it from the wallet. Returns the payload to retry
/// with, the proof, and the amount paid.
fn buy(
    required: &openagents_x402::PaymentRequired,
    request_hash: &str,
    profiles: SupportedProfiles,
    binding: &str,
    descriptor: Option<&(PaidCapability, String)>,
    budget: Budget,
) -> Result<(PaymentPayload, openagents_wallet::Proof, u64), String> {
    let now = openagents_x402::unix_now();
    let Some((terms, invoice)) = required.accepts.iter().find_map(|terms| {
        validate_challenge(
            terms,
            request_hash,
            now,
            nostr::x402::DEFAULT_CLOCK_SKEW,
            profiles,
        )
        .ok()
        .map(|invoice| (terms, invoice))
    }) else {
        return Err(
            "no offered payment requirement is a valid exact/lnbtc invoice for this request".into(),
        );
    };
    if let Some((definition, event_id)) = descriptor {
        let admitted =
            definition.x402.receivers.iter().any(|receiver| {
                receiver.network == terms.network && receiver.pay_to == terms.pay_to
            });
        if !admitted {
            return Err(format!(
                "the challenge's payTo is not a receiver advertised by capability {event_id}"
            ));
        }
        if !definition.x402.bindings.iter().any(|b| b == binding) {
            return Err(format!("the named capability does not advertise {binding}"));
        }
    }
    if invoice.amount_msat() > budget.max_msat {
        return Err(format!(
            "the resource costs {} msat, above --max-msat {}",
            invoice.amount_msat(),
            budget.max_msat
        ));
    }

    let bolt11 = terms
        .extra
        .get("invoice")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let proof = pay_invoice(&bolt11, &terms.network, &budget)?;

    let mut payload = Map::new();
    payload.insert("preimage".into(), Value::String(proof.preimage.clone()));
    Ok((
        PaymentPayload {
            x402_version: 2,
            resource: Some(required.resource.clone()),
            accepted: terms.clone(),
            payload,
            extensions: None,
        },
        proof,
        invoice.amount_msat(),
    ))
}

/// Pay `bolt11` from the wallet within `budget`. A payment still pending
/// after the wait is reported with its hash so the proof can be reused,
/// never paid twice.
pub(crate) fn pay_invoice(
    bolt11: &str,
    network: &str,
    budget: &Budget,
) -> Result<openagents_wallet::Proof, String> {
    let (wallet, wallet_config) = open_wallet().map_err(|error| error.to_string())?;
    if network_id(wallet_config.network.as_str()) != Some(network) {
        let _ = wallet.stop();
        return Err(format!(
            "the invoice is on {network} but this wallet is on {}",
            wallet_config.network.as_str()
        ));
    }
    let proof = wallet.pay(bolt11, budget.max_fee, Duration::from_secs(budget.wait));
    let stopped = wallet.stop();
    let proof = match proof {
        Ok(proof) => proof,
        Err(WalletError::Pending {
            payment_hash,
            waited_secs,
        }) => {
            return Err(format!(
                "payment {payment_hash} is still pending after {waited_secs}s; run `openagents wallet lookup {payment_hash}`, then retry to reuse the proof"
            ));
        }
        Err(error) => return Err(error.to_string()),
    };
    stopped.map_err(|error| error.to_string())?;
    Ok(proof)
}

/// The toll `openagents x402 mcp-serve` puts on every tool call.
struct McpToll(PaidTools<FileReplayStore>);

impl Toll for McpToll {
    fn gate(&self, params: &Value) -> Result<Gate, &'static str> {
        self.0.gate(params, openagents_x402::unix_now())
    }
}

fn mcp_serve(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let Some(server) = args.option("server") else {
        return output.usage("x402", "mcp-serve needs --server URI", USAGE);
    };
    if mcp_binding(server, &json!({"name": "probe"}), &BOUND_METADATA).is_err() {
        return output.usage("x402", "--server must be an absolute URI", USAGE);
    }
    let msat: u64 = match args.number("msat", 0) {
        Ok(0) => return output.usage("x402", "mcp-serve needs --msat N (positive)", USAGE),
        Ok(msat) => msat,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let timeout: u32 = match args.number("timeout", 300) {
        Ok(0) => return output.usage("x402", "--timeout must be positive", USAGE),
        Ok(timeout) => timeout,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let tools: Vec<String> = args
        .options("tool")
        .iter()
        .map(|t| (*t).to_owned())
        .collect();
    let known: Vec<String> = crate::mcp::groups(crate::USAGE)
        .into_iter()
        .map(|g| g.name)
        .collect();
    if let Some(unknown) = tools.iter().find(|t| !known.contains(t)) {
        return output.usage("x402", &format!("--tool {unknown} is not a group"), USAGE);
    }

    let (wallet, wallet_config) = match open_wallet() {
        Ok(opened) => opened,
        Err(error) => return fail_wallet(output, error),
    };
    let Some(network) = network_id(wallet_config.network.as_str()) else {
        let _ = wallet.stop();
        return output.fail(
            "x402",
            &format!(
                "x402 exact/lnbtc has no network for {}; init the wallet on bitcoin or testnet",
                wallet_config.network.as_str()
            ),
        );
    };
    let store = match FileReplayStore::open(&replay_dir()) {
        Ok(store) => store,
        Err(error) => {
            let _ = wallet.stop();
            return output.fail("x402", &error.to_string());
        }
    };
    let wallet = Arc::new(wallet);
    let toll = McpToll(PaidTools {
        server: server.to_owned(),
        network,
        amount_msat: msat,
        timeout_secs: timeout,
        description: "openagents over MCP".into(),
        receiver: Arc::new(Node(wallet.clone())),
        facilitator: openagents_x402::Facilitator::with_profiles(
            store,
            nostr::x402::DEFAULT_CLOCK_SKEW,
            MCP_ONLY,
        ),
    });
    // stdout is the MCP wire, so the banner goes to stderr in both modes.
    let banner = json!({
        "event": "serving",
        "server": server,
        "pay_to": wallet.node_id(),
        "network": network,
        "amount_msat": msat,
        "timeout_secs": timeout,
        "tools": tools,
        "replay_dir": replay_dir().display().to_string(),
    });
    eprintln!("{banner}");
    let mcp = Server {
        usage: crate::USAGE.to_owned(),
        timeout: Duration::from_secs(u64::from(timeout)),
        tools,
        toll: Some(Arc::new(toll)),
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let served = mcp.serve(stdin.lock(), stdout.lock(), std::io::stderr());
    match wallet.stop() {
        Ok(()) => served,
        Err(error) => output.fail("x402", &error.to_string()),
    }
}

/// One stdio MCP server started for a single call.
struct StdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl StdioClient {
    fn start(program: &str, args: &[String]) -> Result<Self, String> {
        let mut child = std::process::Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("spawn {program}: {error}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        let mut client = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let init = client.request(
            "initialize",
            json!({
                "protocolVersion": crate::mcp::PROTOCOL_VERSIONS[0],
                "capabilities": {},
                "clientInfo": { "name": "openagents x402 call", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        if init.get("protocolVersion").is_none() {
            return Err(format!(
                "initialize did not return a protocol version: {init}"
            ));
        }
        client.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
        Ok(client)
    }

    fn send(&mut self, message: &Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(message).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        self.stdin
            .write_all(&bytes)
            .and_then(|()| self.stdin.flush())
            .map_err(|error| format!("write to the MCP server: {error}"))
    }

    /// Send one request and return its `result`, or the server's error text.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .map_err(|error| format!("read from the MCP server: {error}"))?;
            if read == 0 {
                return Err(format!("the MCP server closed before answering {method}"));
            }
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(format!(
                    "{method}: {} (code {})",
                    error["message"].as_str().unwrap_or("error"),
                    error["code"]
                ));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}

impl Drop for StdioClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn call(output: &Output, words: &[String]) -> u8 {
    let (options, command) = match words.iter().position(|w| w == "--") {
        Some(index) => (&words[..index], &words[index + 1..]),
        None => return output.usage("x402", "call needs `-- CMD [ARGS...]`", USAGE),
    };
    let Some((program, program_args)) = command.split_first() else {
        return output.usage("x402", "call needs a command after `--`", USAGE);
    };
    let args = match Args::parse(options, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let Some(tool) = args.positional().first() else {
        return output.usage("x402", "call needs a TOOL", USAGE);
    };
    let tool_args: Vec<&str> = args.options("arg");
    let max_msat: u64 = match args.number("max-msat", 0) {
        Ok(0) => return output.usage("x402", "call needs --max-msat N (positive)", USAGE),
        Ok(max) => max,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let max_fee: u64 = match args.number("max-fee-msat", 0) {
        Ok(fee) => fee,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let wait: u64 = match args.number("wait", 60) {
        Ok(wait) => wait,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let show_proof = args.switch("show-proof");
    let descriptor = match args.option("cap") {
        None => None,
        Some(head) => match resolve_descriptor(head, args.option("relay"), args.option("as")) {
            Ok(descriptor) => Some(descriptor),
            Err(Refusal::Usage(message)) => return output.usage("x402", &message, USAGE),
            Err(Refusal::Failure(message)) => return output.fail("x402", &message),
        },
    };
    let server = match (args.option("server"), &descriptor) {
        (Some(server), Some((definition, _))) if definition.endpoint != server => {
            return output.fail(
                "x402",
                &format!(
                    "{server} is not the advertised endpoint {} of the named capability",
                    definition.endpoint
                ),
            );
        }
        (Some(server), _) => server.to_owned(),
        (None, Some((definition, _))) => definition.endpoint.clone(),
        (None, None) => return output.usage("x402", "call needs --server URI or --cap", USAGE),
    };
    let params = json!({ "name": tool, "arguments": { "args": tool_args } });
    let request_hash =
        match mcp_binding(&server, &params, &BOUND_METADATA).and_then(|b| binding_hash(&b)) {
            Ok(hash) => hash,
            Err(_) => return output.usage("x402", "--server must be an absolute URI", USAGE),
        };

    let mut client = match StdioClient::start(program, program_args) {
        Ok(client) => client,
        Err(message) => return output.fail("x402", &message),
    };
    let first = match client.request("tools/call", params.clone()) {
        Ok(result) => result,
        Err(message) => return output.fail("x402", &message),
    };
    let Some(required) = payment_required_from_result(&first) else {
        return finish_call(output, &first, None, None, show_proof);
    };
    if required.resource.url != server {
        return output.fail(
            "x402",
            "the challenge names a different server URI than the one bound",
        );
    }
    let (payload, proof, amount) = match buy(
        &required,
        &request_hash,
        MCP_ONLY,
        "mcp:1",
        descriptor.as_ref(),
        Budget {
            max_msat,
            max_fee,
            wait,
        },
    ) {
        Ok(bought) => bought,
        Err(message) => return output.fail("x402", &message),
    };
    let paid = match with_payment(params, &payload) {
        Ok(paid) => paid,
        Err(message) => return output.fail("x402", message),
    };
    match client.request("tools/call", paid) {
        Ok(result) => finish_call(output, &result, Some(&proof), Some(amount), show_proof),
        Err(message) => output.fail(
            "x402",
            &format!(
                "paid retry failed: {message}; proof for payment {} is in `openagents wallet lookup`",
                proof.payment_hash
            ),
        ),
    }
}

fn finish_call(
    output: &Output,
    result: &Value,
    proof: Option<&openagents_wallet::Proof>,
    amount_msat: Option<u64>,
    show_proof: bool,
) -> u8 {
    let settlement = result
        .get("_meta")
        .and_then(|meta| meta.get(PAYMENT_RESPONSE_META))
        .cloned();
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let mut value = json!({
        "paid": proof.is_some(),
        "amount_msat": amount_msat,
        "fee_msat": proof.map(|p| p.fee_msat),
        "payment_hash": proof.map(|p| p.payment_hash.clone()),
        "settlement": settlement,
        "is_error": is_error,
        "result": result,
    });
    if show_proof && let Some(proof) = proof {
        value["preimage"] = Value::String(proof.preimage.clone());
        value["bolt11"] = Value::String(proof.bolt11.clone());
    }
    output.emit(&value, |value| {
        let text = value["result"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        match proof {
            Some(proof) => format!(
                "{text}\nopenagents x402: paid {} msat (+{} fee), payment {}",
                amount_msat.unwrap_or_default(),
                proof.fee_msat,
                proof.payment_hash
            ),
            None => text,
        }
    });
    if is_error { crate::EXIT_FAILURE } else { 0 }
}

fn finish(
    output: &Output,
    status: u16,
    headers: &reqwest::header::HeaderMap,
    body: &[u8],
    proof: Option<&openagents_wallet::Proof>,
    amount_msat: Option<u64>,
    show_proof: bool,
) -> u8 {
    let settlement: Option<SettlementResponse> = headers
        .get(PAYMENT_RESPONSE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| wire::decode_header(v).ok());
    let text = String::from_utf8_lossy(body).to_string();
    let mut value = json!({
        "status": status,
        "paid": proof.is_some(),
        "amount_msat": amount_msat,
        "fee_msat": proof.map(|p| p.fee_msat),
        "payment_hash": proof.map(|p| p.payment_hash.clone()),
        "settlement": settlement,
        "body": text,
    });
    if show_proof && let Some(proof) = proof {
        value["preimage"] = Value::String(proof.preimage.clone());
        value["bolt11"] = Value::String(proof.bolt11.clone());
    }
    let ok = (200..300).contains(&status);
    if output.json() {
        println!("{value}");
    } else {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
        if let Some(proof) = proof {
            eprintln!(
                "openagents x402: paid {} msat (+{} fee), payment {}, status {status}",
                amount_msat.unwrap_or_default(),
                proof.fee_msat,
                proof.payment_hash
            );
        }
    }
    if ok { 0 } else { crate::EXIT_FAILURE }
}

/// A refusal on the way to a paid capability: the caller's words, or the
/// relay and record.
enum Refusal {
    Usage(String),
    Failure(String),
}

/// A resolved `oa-x402-v1` adapter: the advertised endpoint and descriptor.
struct PaidCapability {
    endpoint: String,
    x402: nostr::cap::X402Descriptor,
}

/// What one `oa-x402-v1` advertisement says.
struct Advertisement<'a> {
    publisher: &'a str,
    slug: &'a str,
    /// The public endpoint for `http:1` and `mcp:1`; ignored for the
    /// native binding, which is reached through `relays`.
    url: &'a str,
    /// `http:1`, `mcp:1`, or `nostr:openagents:1`.
    binding: &'a str,
    /// The relays that carry native records; empty for HTTP and MCP.
    relays: &'a [String],
    network: &'a str,
    pay_to: &'a str,
    merchant: &'a str,
    summary: &'a str,
}

/// The x402 adapter definition for one paid resource.
fn paid_definition(ad: &Advertisement<'_>) -> Value {
    let Advertisement {
        publisher,
        slug,
        url,
        binding,
        relays,
        network,
        pay_to,
        merchant,
        summary,
    } = *ad;
    let native = binding == openagents_x402::native::PROFILE;
    let (interface, transport) = if native {
        ("openagents.x402.native.v1", "nostr-cj")
    } else if binding == "mcp:1" {
        ("openagents.x402.mcp.v1", "mcp")
    } else {
        ("openagents.x402.http.v1", "http")
    };
    let remote = if native {
        json!({"worker": publisher, "relays": relays})
    } else {
        json!({"endpoint": url})
    };
    let recovery = if native {
        openagents_x402::native::RECOVERY
    } else {
        "none"
    };
    let schema = json!({
        "digest": format!(
            "sha256:{}",
            Sha256::digest(BYTES_SCHEMA.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
        "size": BYTES_SCHEMA.len(),
        "media_type": "application/schema+json",
    });
    json!({
        "v": 1,
        "requires": [nostr::cap::X402_FEATURE],
        "id": format!("{publisher}:x402/{slug}"),
        "profile": "adapter",
        "summary": summary,
        "input": schema,
        "output": schema,
        "effects": {
            "reads": [],
            "writes": [],
            "network": ["remote"],
            "process": false,
            "delegates": false,
            "spend": false
        },
        "minimum": {},
        "support": {
            "bounds": {},
            "cancellation": "unsupported",
            "idempotency": "none",
            "evidence": []
        },
        "binding_contract": {
            "interface": interface,
            "transport": transport,
            "operations": [slug],
            "remote": remote,
            "x402": {
                "v": "openagents.x402-discovery.v1",
                "protocol": "x402-v2",
                "scheme": "exact",
                "asset": "BTC",
                "method": "bolt11",
                "flow": "upfront",
                "bindings": [binding],
                "receivers": [{"network": network, "pay_to": pay_to}],
                "merchant": merchant,
                "recovery": recovery,
                "recovery_contract": null
            }
        }
    })
}

fn advertise(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let Some(slug) = args.option("slug") else {
        return output.usage("x402", "advertise needs --slug SLUG", USAGE);
    };
    let Some(merchant) = args.option("merchant") else {
        return output.usage("x402", "advertise needs --merchant ID", USAGE);
    };
    let binding = args.option("binding").unwrap_or("http:1");
    let native = binding == openagents_x402::native::PROFILE;
    let relay = relay_url(args.option("relay"));
    let mut relays: Vec<String> = args
        .options("relays")
        .into_iter()
        .map(str::to_owned)
        .collect();
    if relays.is_empty() {
        relays.push(relay.clone());
    }
    if native
        && (relays.len() > 8
            || relays
                .iter()
                .any(|r| !(r.starts_with("ws://") || r.starts_with("wss://"))))
    {
        return output.usage("x402", "--relays takes one to eight ws(s):// URLs", USAGE);
    }
    let url = match (args.option("url"), native) {
        (_, true) => "",
        (Some(url), false) => url,
        (None, false) => {
            return output.usage("x402", "advertise needs --url PUBLIC_URL", USAGE);
        }
    };
    let bound = match binding {
        "http:1" => http_binding("POST", url, &[], &[]).is_ok(),
        "mcp:1" => mcp_binding(url, &json!({"name": "probe"}), &BOUND_METADATA).is_ok(),
        openagents_x402::native::PROFILE => true,
        other => {
            return output.usage(
                "x402",
                &format!("--binding {other} is not http:1, mcp:1, or nostr:openagents:1"),
                USAGE,
            );
        }
    };
    if !bound {
        return output.usage(
            "x402",
            "--url must be an absolute URI (http(s) without a fragment for http:1)",
            USAGE,
        );
    }
    let default_summary = format!("a paid {binding} resource (x402 exact/lnbtc)");
    let summary = args.option("summary").unwrap_or(&default_summary);
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail("x402", &message),
    };
    let (wallet, wallet_config) = match open_wallet() {
        Ok(opened) => opened,
        Err(error) => return fail_wallet(output, error),
    };
    let pay_to = wallet.node_id();
    let _ = wallet.stop();
    let Some(network) = network_id(wallet_config.network.as_str()) else {
        return output.fail(
            "x402",
            &format!(
                "wallet network `{}` has no x402 network id; use bitcoin or testnet",
                wallet_config.network.as_str()
            ),
        );
    };
    let body = paid_definition(&Advertisement {
        publisher: signer.pubkey(),
        slug,
        url,
        binding,
        relays: &relays,
        network,
        pay_to: &pay_to,
        merchant,
        summary,
    });
    let definition = match nostr::cap::parse_definition(&body) {
        Ok(definition) => definition,
        Err(error) => return output.usage("x402", &format!("definition: {error}"), USAGE),
    };
    let tags = vec![
        nostr::domain::Tag::new(vec!["d".to_owned(), slug.to_owned()]),
        nostr::domain::Tag::new(vec!["t".to_owned(), nostr::cap::CAP_MARKER.to_owned()]),
        nostr::domain::Tag::new(vec!["t".to_owned(), definition.profile.tag().to_owned()]),
        nostr::domain::Tag::new(vec![
            "t".to_owned(),
            format!("oa:transport:{}", definition.transport),
        ]),
    ];
    let event = signer.sign(
        crate::relay::unix_now(),
        nostr::cap::DISCOVERY_KIND,
        tags,
        body.to_string(),
    );
    if let Err(error) = nostr::cap::check_discovery_tags(&event.tags, &definition) {
        return output.fail("x402", &format!("discovery tags: {error}"));
    }
    let endpoint = if native {
        json!({"worker": event.pubkey, "relays": relays})
    } else {
        Value::String(url.to_owned())
    };
    let mut doc = json!({
        "relay": relay,
        "event_id": event.id,
        "publisher": event.pubkey,
        "slug": slug,
        "endpoint": endpoint,
        "binding": binding,
        "network": network,
        "pay_to": pay_to,
        "merchant": merchant,
        "definition": body,
        "published": false,
    });
    if args.switch("dry-run") {
        output.emit(&doc, |value| {
            format!(
                "dry run: {}:{} advertises {} for payTo {} on {} (not published)",
                value["publisher"],
                value["slug"],
                value["endpoint"],
                value["pay_to"],
                value["network"]
            )
        });
        return 0;
    }
    let mut client = Client::connect(&relay, signer);
    let published = client.publish(event, Duration::from_secs(20));
    client.close();
    match published {
        Ok(ack) if ack.accepted => {
            doc["published"] = Value::Bool(true);
            output.emit(&doc, |value| {
                format!(
                    "published {} as {}:{} on {}\n  endpoint {}\n  payTo {} ({})",
                    value["event_id"],
                    value["publisher"],
                    value["slug"],
                    value["relay"],
                    value["endpoint"],
                    value["pay_to"],
                    value["network"]
                )
            });
            0
        }
        Ok(ack) => output.fail(
            "x402",
            &format!("{relay} refused the head: {}", ack.message),
        ),
        Err(message) => output.fail("x402", &message),
    }
}

/// Resolve `PUBKEY:SLUG` to the newest valid kind 30180 head and its
/// x402 descriptor. A head without `oa-x402-v1`, or with a refusal, is not
/// a paid capability.
fn resolve_descriptor(
    head: &str,
    relay: Option<&str>,
    profile: Option<&str>,
) -> Result<(PaidCapability, String), Refusal> {
    let Some((author, slug)) = head.split_once(':') else {
        return Err(Refusal::Usage("--cap takes PUBKEY:SLUG".into()));
    };
    let author = author.trim().to_ascii_lowercase();
    if author.len() != 64 || !author.chars().all(|c| c.is_ascii_hexdigit()) || slug.is_empty() {
        return Err(Refusal::Usage("--cap takes PUBKEY:SLUG".into()));
    }
    let signer = signer_for(profile).map_err(Refusal::Failure)?;
    let url = relay_url(relay);
    let mut client = Client::connect(&url, signer);
    let mut newest: Option<nostr::domain::Event> = None;
    let filter = json!({
        "kinds": [nostr::cap::DISCOVERY_KIND],
        "authors": [author],
        "#d": [slug],
        "#t": [nostr::cap::CAP_MARKER],
        "limit": 8,
    });
    let outcome = client.subscribe(vec![filter], false, Duration::from_secs(15), |event| {
        if newest
            .as_ref()
            .is_none_or(|current| event.created_at > current.created_at)
        {
            newest = Some(event.clone());
        }
    });
    client.close();
    outcome.map_err(Refusal::Failure)?;
    let Some(event) = newest else {
        return Err(Refusal::Failure(format!(
            "{url} has no kind {} head {author}:{slug}",
            nostr::cap::DISCOVERY_KIND
        )));
    };
    event.validate_crypto().map_err(|error| {
        Refusal::Failure(format!("capability {}: signature: {error}", event.id))
    })?;
    if event.is_expired(openagents_x402::unix_now()) {
        return Err(Refusal::Failure(format!(
            "capability {} has expired",
            event.id
        )));
    }
    let body: Value = serde_json::from_str(&event.content)
        .map_err(|error| Refusal::Failure(format!("capability {}: {error}", event.id)))?;
    let definition = nostr::cap::parse_definition(&body)
        .and_then(|definition| {
            nostr::cap::check_discovery_tags(&event.tags, &definition).map(|()| definition)
        })
        .map_err(|error| Refusal::Failure(format!("capability {}: {error}", event.id)))?;
    let Some(x402) = definition.x402 else {
        return Err(Refusal::Failure(format!(
            "capability {} does not require {}",
            event.id,
            nostr::cap::X402_FEATURE
        )));
    };
    let endpoint = body["binding_contract"]["remote"]["endpoint"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if endpoint.is_empty() {
        return Err(Refusal::Failure(format!(
            "capability {} has no remote endpoint",
            event.id
        )));
    }
    Ok((PaidCapability { endpoint, x402 }, event.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_advertised_definition_passes_the_cap_contract() {
        let body = paid_definition(&Advertisement {
            publisher: &"a".repeat(64),
            slug: "echo",
            url: "https://example.com/echo",
            binding: "http:1",
            relays: &[],
            network: nostr::x402::TESTNET,
            pay_to: &format!("02{}", "b".repeat(64)),
            merchant: "demo",
            summary: "echo bytes",
        });
        let definition = nostr::cap::parse_definition(&body).unwrap();
        let x402 = definition.x402.unwrap();
        assert_eq!(x402.bindings, vec!["http:1".to_owned()]);
        assert_eq!(x402.receivers[0].network, nostr::x402::TESTNET);
        assert_eq!(definition.transport, "http");
    }

    #[test]
    fn the_mcp_definition_names_the_server_over_mcp() {
        let body = paid_definition(&Advertisement {
            publisher: &"a".repeat(64),
            slug: "tools",
            url: "mcp://tools.example.com/openagents",
            binding: "mcp:1",
            relays: &[],
            network: nostr::x402::TESTNET,
            pay_to: &format!("02{}", "b".repeat(64)),
            merchant: "demo",
            summary: "openagents over MCP",
        });
        let definition = nostr::cap::parse_definition(&body).unwrap();
        let x402 = definition.x402.unwrap();
        assert_eq!(x402.bindings, vec!["mcp:1".to_owned()]);
        assert_eq!(definition.transport, "mcp");
        assert_eq!(
            body["binding_contract"]["remote"]["endpoint"],
            "mcp://tools.example.com/openagents"
        );
    }

    #[test]
    fn the_native_definition_names_the_worker_and_relays_over_nostr_cj() {
        let publisher = "a".repeat(64);
        let relays = vec!["wss://relay.openagents.com/".to_owned()];
        let body = paid_definition(&Advertisement {
            publisher: &publisher,
            slug: "echo",
            url: "",
            binding: openagents_x402::native::PROFILE,
            relays: &relays,
            network: nostr::x402::TESTNET,
            pay_to: &format!("02{}", "b".repeat(64)),
            merchant: "demo",
            summary: "echo bytes over Nostr",
        });
        let definition = nostr::cap::parse_definition(&body).unwrap();
        let x402 = definition.x402.unwrap();
        assert_eq!(
            x402.bindings,
            vec![openagents_x402::native::PROFILE.to_owned()]
        );
        assert_eq!(x402.recovery, openagents_x402::native::RECOVERY);
        assert_eq!(definition.transport, "nostr-cj");
        assert_eq!(body["binding_contract"]["remote"]["worker"], publisher);
        assert_eq!(body["binding_contract"]["remote"]["relays"][0], relays[0]);
        assert!(body["binding_contract"]["remote"].get("endpoint").is_none());
    }
}
