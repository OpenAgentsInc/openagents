//! `openagents x402`: sell one operation over HTTP or MCP for an exact
//! Lightning payment, or buy one. Buying pays from the person's Spark
//! wallet (`openagents wallet`, `x402_spark`) unless `--pay-with` says this
//! computer's Lightning node or the phone; selling from this computer
//! (`serve`, `mcp-serve`, `native-serve`) needs that node, under
//! `openagents x402 node` (`x402_node`), and `publish` sells through
//! OpenAgents with no node at all.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nostr::x402::{SupportedProfiles, binding_hash, http_binding, mcp_binding, validate_challenge};
use openagents_wallet::open::Opened;
use openagents_wallet::{LightningWallet, WalletConfig, WalletError, config};
use openagents_x402::facilitator::{HTTP_ONLY, MCP_ONLY};
use openagents_x402::mcp::{
    BOUND_METADATA, Gate, PAYMENT_RESPONSE_META, PaidTools, payment_required_from_result,
    with_payment,
};
use openagents_x402::policy::{
    Ceiling, DAY_SECS, Entry, Flags, LEDGER_FILE, Ledger, Limits, POLICY_FILE, Policy,
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
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents x402 COMMAND [OPTIONS]
  serve --url PUBLIC_URL --msat N [--listen HOST:PORT] [--expiry SECONDS]
        [--mime TYPE] [--seconds N] -- CMD [ARGS...]
                          Self-hosting: sell CMD from this computer's own
                          wallet at PUBLIC_URL for exactly N msat per call
                          (x402 exact/lnbtc, http:1); `publish` sells through
                          OpenAgents' receiver instead. A request without
                          PAYMENT-SIGNATURE gets a 402 with an invoice bound to
                          the method, URL, and body; a paid request runs CMD
                          with the body on stdin and returns stdout. Each
                          invoice settles once; a replay is duplicate_settlement.
  publish --upstream URL --price-sats N --payout ADDRESS [--resource NAME]
        [--method GET|POST] [--summary TEXT] [--front URL] [--dry-run]
        [--as PROFILE]
                          Sell an HTTP service you run through OpenAgents'
                          receiver, with no wallet of your own: sign a
                          registration with this key and post it to the pay
                          front (--front, default $OPENAGENTS_PAY_FRONT or
                          https://api.openagents.com). The front sells
                          FRONT/x/NAME with its own 402 and invoice, forwards
                          each paid call to URL (https, a public address)
                          with an OpenAgents-Paid header to check against the
                          key at FRONT/v1/paid-key, and pays your share to
                          ADDRESS (a Spark address, Lightning address, or node
                          key). NAME defaults to URL's last path segment;
                          METHOD to POST. Publishing again with the same key
                          updates it.
  fetch URL [--method M] [--body FILE|-] [--max-msat N] [--max-fee-msat F]
        [--wait SECONDS] [--cap PUBKEY:SLUG] [--relay URL] [--show-proof]
        [--pay-with wallet|node|phone]
                          Buy one call: read the 402, check the invoice against
                          this request, refuse above the ceiling, pay from your
                          wallet, retry with the preimage, print the body. With
                          --cap, resolve that NIP-CAP head first and refuse a
                          challenge whose payTo or URL it does not advertise.
                          --pay-with node pays from this computer's Lightning
                          node instead; --pay-with phone asks the owner's phone
                          to pay through this computer's Coder host (mainnet
                          only; the owner approves each payment there, and the
                          wait is at least 300 seconds).
  mcp-serve --server URI --msat N --tool GROUP [--tool GROUP]... [--expiry SECONDS]
                          Serve `openagents mcp serve` over stdio with a toll
                          (x402 exact/lnbtc, mcp:1): a tools/call without
                          _meta[\"x402/payment\"] gets an error result carrying
                          PaymentRequired with an invoice bound to URI, the
                          tool name, and its arguments; a paid call runs and
                          returns its result with _meta[\"x402/payment-response\"].
                          --tool names each group sold (at least one); only
                          their read-only commands run, and wallet, pay, x402,
                          key, ssh, service, and host are never sold. URI is
                          the name the buyer must bind to; it is not
                          connected to.
  call TOOL [--arg WORD]... [--max-msat N] [--max-fee-msat F] [--wait SECONDS]
        [--server URI] [--cap PUBKEY:SLUG] [--relay URL] [--show-proof]
        [--pay-with wallet|node|phone]
        -- CMD [ARGS...]
                          Buy one tools/call: start CMD as a stdio MCP server,
                          call TOOL with {\"args\": [WORD...]}, check the
                          challenge's invoice against this call and URI, refuse
                          above the ceiling, pay from your wallet, retry with the
                          proof, print the result. With --cap, URI defaults to
                          the advertised endpoint and the payTo must be one it
                          advertises.
  native-serve --slug SLUG --msat N [--expiry SECONDS] [--per-buyer N]
        [--rerun-safe] [--seconds N] [--as PROFILE] [--relay URL]
        (-- CMD [ARGS...] | --cj WORKER [--cj-relay URL])
                          Sell CMD over the relay (x402 exact/lnbtc,
                          nostr:openagents:1): every record is a private kind
                          3188 artifact sealed to the other party. A request
                          record gets a challenge with an invoice bound to the
                          buyer, this key, the purchase nonce, and the request
                          bytes; a valid claim settles once, then CMD runs with
                          the input on stdin and its stdout is sealed back with
                          a status chain (offered, claim_pending, admitted,
                          running, completed or failed). --per-buyer refuses a
                          buyer's Nth+1 request in a rolling hour (rate_limited).
                          On start, purchases a previous process left admitted
                          or running are finished as failed
                          (provider_restarted); with --rerun-safe, one whose
                          execution window is still open is run again instead
                          once its input is read back from the relay. With
                          --cj, the input is sent as one NIP-CJ job to WORKER
                          (npub or hex) over --cj-relay (default CODER_RELAY,
                          then --relay) and its result is the output; the job
                          ID is kept in the purchase store, so a restart follows
                          the job instead of failing the purchase. Worker
                          outcomes become causes no_worker, worker_silent,
                          worker_refused, or worker_failed.
  buy PROVIDER --slug SLUG [--input FILE|-] [--max-msat N] [--max-fee-msat F]
        [--wait SECONDS] [--as PROFILE] [--relay URL] [--show-proof]
        [--pay-with wallet|node]
                          Buy one run: resolve PROVIDER:SLUG on the relay, seal
                          the input and a request to PROVIDER, check the
                          challenge against them, refuse above the ceiling, pay
                          from your wallet, seal the claim, follow the status
                          chain, print the output. A run that does not end
                          within --wait leaves the purchase for `status`; it
                          is never paid again.
  status PROVIDER PURCHASE [--wait SECONDS] [--as PROFILE] [--relay URL]
                          Ask PROVIDER for the status chain of PURCHASE and
                          print the newest status and any output. Without
                          --wait, a running purchase is followed to the end of
                          its execution window. `unknown` means the provider
                          has not answered; nothing is paid twice in any case.
  status --list [--as PROFILE]
  status --finish BUYER:PURCHASE --cause CAUSE [--as PROFILE] [--relay URL]
                          Provider side: list this key's open purchases, or
                          finish one by hand as failed with a recorded cause
                          (provider_restarted, operator_cancelled,
                          execute_until_passed) and publish the status.
  advertise --slug SLUG --merchant ID [--url PUBLIC_URL] [--front URL]
        [--local] [--binding http:1|mcp:1|nostr:openagents:1] [--relays URL]...
        [--summary TEXT] [--test] [--dev] [--dry-run] [--as PROFILE] [--relay URL]
                          Publish (or print) the kind 30180 adapter definition
                          that advertises a paid resource (NIP-CAP feature
                          oa-x402-v1) over one binding. http:1 (default)
                          advertises the resource SLUG this key published
                          (`publish`) at the pay front's URL with the front's
                          payTo; --local advertises PUBLIC_URL paid to this
                          computer's wallet instead (self-hosting). mcp:1
                          advertises the MCP server URI PUBLIC_URL, and
                          nostr:openagents:1 this key on --relays (default:
                          --relay) with recovery native-record-v1, both paid
                          to this wallet. --test and --dev mark demo listings,
                          hidden from `cap list` unless --all.
  policy [show]           Print the buyer policy and where it lives.
  policy set [--max-msat N|-] [--max-fee-msat F|-] [--daily-cap-msat N|-]
        [--provider NODE_ID | --cap PUBKEY:SLUG]
                          Set the default ceiling, or the ceiling for one
                          provider node (payTo) or one capability; `-` clears
                          a field. --daily-cap-msat applies to the whole wallet.
  policy allow NODE_ID... / policy deny NODE_ID...
                          Add to or remove from the provider allowlist. An
                          empty list admits every provider.
  ledger [--since SECONDS] [--binding B] [--provider NODE_ID]
                          List what this buyer paid across http:1, mcp:1, and
                          nostr:openagents:1: when, provider, resource, amount,
                          fee, payment hash, and how the call ended, plus the
                          total; --since limits to the last N seconds.
The buyer ceiling for a call is --max-msat if given, else the policy's
capability, provider, or default ceiling; with neither, the call is refused
before anything is paid. The allowlist and the daily cap (amounts plus fees
over the last 24 hours, from the ledger) hold whatever the flags say. Without
--max-fee-msat the fee cap is the policy's, else max(10 sats, 2% of the
amount) for the wallet; the Lightning node and phone keep 1% + 1000 msat.
Replay records live in ~/.openagents/x402/replay, native purchases in
~/.openagents/x402/native, the policy in ~/.openagents/x402/policy.json, and
the ledger in ~/.openagents/x402/ledger.ndjson. The preimage is printed only
with --show-proof. Add --json before `x402` for one JSON document.
Buying (fetch, call, buy) pays from your wallet, `openagents wallet`: the
same balance as the OpenAgents app on your phone. Selling from this computer
(serve, mcp-serve, native-serve) needs its own Lightning node; set it up and
run it with `openagents x402 node` (see `openagents x402 node --help`), or sell
through OpenAgents with no node: `openagents x402 publish`. With no wallet on
this computer and a Lightning node set up, buying pays from the node.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("serve", Effect::LongRunning, "wallet"),
    Declared::screen("publish", Effect::Publishes, "wallet"),
    Declared::screen("fetch", Effect::Spends, "wallet"),
    Declared::screen("mcp-serve", Effect::LongRunning, "wallet"),
    Declared::screen("call", Effect::Spends, "wallet"),
    Declared::screen("native-serve", Effect::LongRunning, "wallet"),
    Declared::screen("buy", Effect::Spends, "wallet"),
    Declared::screen("status", Effect::Publishes, "wallet"),
    Declared::screen("advertise", Effect::Publishes, "wallet"),
    Declared::screen("policy show", Effect::ReadOnly, "wallet"),
    Declared::screen("policy set", Effect::Spends, "wallet"),
    Declared::screen("policy allow", Effect::Spends, "wallet"),
    Declared::screen("policy deny", Effect::Spends, "wallet"),
    Declared::screen("ledger", Effect::ReadOnly, "wallet"),
];

const SWITCHES: &[&str] = &["show-proof", "dry-run", "local", "test", "dev"];

/// The pay front `publish` and `advertise` talk to by default.
pub(crate) fn pay_front(flag: Option<&str>) -> String {
    flag.map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_PAY_FRONT").ok())
        .unwrap_or_else(|| "https://api.openagents.com".into())
        .trim_end_matches('/')
        .to_owned()
}

/// The schema both x402 adapter operations declare: opaque bytes in and out.
const BYTES_SCHEMA: &str = r#"{"type":"string","contentEncoding":"binary"}"#;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("x402", "a command is required", USAGE);
    };
    if rest.first().is_some_and(|word| word == "--help") {
        if command == "node" {
            return crate::x402_node::run(output, rest);
        }
        if let Some(usage) = crate::argv::command_usage("x402", command, USAGE) {
            println!("{usage}");
            return 0;
        }
    }
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "serve" => serve(output, rest),
        "publish" => publish(output, rest),
        "fetch" => fetch(output, rest),
        "mcp-serve" => mcp_serve(output, rest),
        "call" => call(output, rest),
        "native-serve" => crate::x402_native::serve(output, rest),
        "buy" => crate::x402_native::buy(output, rest),
        "status" => crate::x402_native::status(output, rest),
        "advertise" => advertise(output, rest),
        "policy" => policy(output, rest),
        "ledger" => ledger(output, rest),
        "node" => crate::x402_node::run(output, rest),
        other => output.usage("x402", &format!("unknown command `{other}`"), USAGE),
    }
}

pub(crate) fn x402_home() -> PathBuf {
    match std::env::var_os("OPENAGENTS_X402_HOME") {
        Some(home) => PathBuf::from(home),
        None => config::home()
            .parent()
            .map(|p| p.join("x402"))
            .unwrap_or_else(|| PathBuf::from("x402")),
    }
}

pub(crate) fn replay_dir() -> PathBuf {
    x402_home().join("replay")
}

/// The resident node when `x402 node serve` answers, else a node opened here.
pub(crate) fn open_wallet() -> Result<(Opened, WalletConfig), WalletError> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let (mnemonic, _) = config::load_or_create_seed(&home, false, String::new)?;
    Ok((
        Opened::open(&home, &wallet_config, &mnemonic)?,
        wallet_config,
    ))
}

pub(crate) fn fail_wallet(output: &Output, error: WalletError) -> u8 {
    match error {
        WalletError::Invalid(message) => output.usage("x402", &message, USAGE),
        other => output.fail("x402", &other.to_string()),
    }
}

pub(crate) struct Node(pub(crate) Arc<Opened>);

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
            .receive_exact_from_node(&self.0.node_id(), amount_msat, request_hash, expiry_secs)
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
    let timeout = match expiry(&args) {
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
    if let Err(message) = toll_floor(&wallet_config, msat) {
        let _ = wallet.stop();
        return output.fail("x402", &message);
    }
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
    let flags = match flags(&args).and_then(|flags| ceiling_present(flags).map(|()| flags)) {
        Ok(flags) => flags,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let wait: u64 = match args.number("wait", 60) {
        Ok(wait) => wait,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let payer = match payer(&args, true) {
        Ok(payer) => payer,
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

    let spend = Spend {
        flags,
        capability: args.option("cap").map(str::to_owned),
        wait,
        binding: "http:1",
        resource: url.clone(),
        payer,
    };
    let fetched = fetch_paid(&method, url, &body, |required| {
        buy(
            required,
            &request_hash,
            HTTP_ONLY,
            "http:1",
            descriptor.as_ref(),
            spend,
        )
    });
    match fetched {
        Ok(Fetched { reply, paid: None }) => finish(
            output,
            reply.status,
            &reply.headers,
            &reply.body,
            None,
            None,
            show_proof,
        ),
        Ok(Fetched {
            reply,
            paid: Some((proof, amount)),
        }) => finish(
            output,
            reply.status,
            &reply.headers,
            &reply.body,
            Some(&proof),
            Some(amount),
            show_proof,
        ),
        Err(message) => output.fail("x402", &message),
    }
}

/// One HTTP answer.
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: reqwest::header::HeaderMap,
    pub(crate) body: Vec<u8>,
}

/// What `fetch` got: the final answer, and the proof and amount when it
/// paid for it.
pub(crate) struct Fetched {
    pub(crate) reply: Reply,
    pub(crate) paid: Option<(openagents_wallet::Proof, u64)>,
}

/// One `fetch`: send the request; on a `402`, check the challenge names
/// this URL, have `buy` pay it (it checks the invoice against the request
/// and the policy), and retry once with the proof.
pub(crate) fn fetch_paid(
    method: &str,
    url: &str,
    body: &[u8],
    buy: impl FnOnce(
        &openagents_x402::PaymentRequired,
    ) -> Result<(PaymentPayload, openagents_wallet::Proof, u64), String>,
) -> Result<Fetched, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| error.to_string())?;
    let runtime = crate::runtime();
    let send = |signature: Option<String>| {
        let request = client
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).expect("method is a token"),
                url,
            )
            .body(body.to_vec());
        let request = match signature {
            Some(signature) => request.header(PAYMENT_SIGNATURE, signature),
            None => request,
        };
        runtime.block_on(async {
            let response = request.send().await?;
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let body = response.bytes().await?.to_vec();
            Ok::<_, reqwest::Error>(Reply {
                status,
                headers,
                body,
            })
        })
    };

    let first = send(None).map_err(|error| format!("{method} {url}: {error}"))?;
    if first.status != 402 {
        return Ok(Fetched {
            reply: first,
            paid: None,
        });
    }
    let required = first
        .headers
        .get(PAYMENT_REQUIRED)
        .and_then(|v| v.to_str().ok())
        .ok_or("402 without a PAYMENT-REQUIRED header")?;
    let required = wire::decode_payment_required(required)
        .map_err(|error| format!("PAYMENT-REQUIRED: {error}"))?;
    if required.resource.url != *url {
        return Err("the challenge names a different resource URL than the one requested".into());
    }
    let (payload, proof, amount) = buy(&required)?;
    let signature = wire::encode_header(&payload).map_err(|error| error.to_string())?;
    match send(Some(signature)) {
        Ok(reply) => Ok(Fetched {
            reply,
            paid: Some((proof, amount)),
        }),
        Err(error) => {
            set_phase(&proof.payment_hash, "retry_failed");
            Err(format!(
                "paid retry failed: {error}; proof for payment {} is in `openagents x402 node lookup`",
                proof.payment_hash
            ))
        }
    }
}

/// What the buyer is willing to spend on one call, before the policy is
/// applied: the flags, the capability the call is for, the wait, and how
/// the ledger names it.
pub(crate) struct Spend {
    pub(crate) flags: Flags,
    pub(crate) capability: Option<String>,
    pub(crate) wait: u64,
    pub(crate) binding: &'static str,
    pub(crate) resource: String,
    /// Who pays: the person's wallet, this computer's Lightning node, or
    /// the owner's phone (`--pay-with`).
    pub(crate) payer: Payer,
}

/// Who pays for a call (`--pay-with`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Payer {
    /// The person's Spark wallet, `openagents wallet` (`x402_spark`).
    Wallet,
    /// This computer's Lightning node, `openagents x402 node`.
    Node,
    /// The owner's phone, through this computer's Coder host (`x402_phone`).
    Phone,
}

/// `--pay-with wallet|node|phone` (`phone` only where `phone` is true);
/// without it, the wallet, unless this computer has no wallet and has a
/// Lightning node set up.
pub(crate) fn payer(args: &Args, phone: bool) -> Result<Payer, String> {
    match args.option("pay-with") {
        Some("wallet") => Ok(Payer::Wallet),
        Some("node") => Ok(Payer::Node),
        Some("phone") if phone => Ok(Payer::Phone),
        Some(_) if phone => Err("--pay-with takes wallet, node, or phone".into()),
        Some(_) => Err("--pay-with takes wallet or node".into()),
        None => Ok(computer_payer()),
    }
}

/// The payer this computer uses when nothing names one: its wallet, unless
/// it has none and has a Lightning node set up.
pub(crate) fn computer_payer() -> Payer {
    default_payer(
        openagents_spark::computer::has_seed(&openagents_spark::computer::home()),
        config::home().join(config::CONFIG_FILE).is_file(),
    )
}

/// The payer when `--pay-with` is not given.
fn default_payer(has_wallet: bool, has_node: bool) -> Payer {
    if !has_wallet && has_node {
        Payer::Node
    } else {
        Payer::Wallet
    }
}

/// Pay `bolt11` the way `payer` says, within the fee cap and the wait.
pub(crate) fn pay_by(
    payer: Payer,
    bolt11: &str,
    network: &str,
    max_fee_msat: u64,
    wait: u64,
    resource: &str,
) -> Result<openagents_wallet::Proof, String> {
    match payer {
        Payer::Wallet => crate::x402_spark::pay(bolt11, network, max_fee_msat),
        Payer::Node => pay_invoice(bolt11, network, max_fee_msat, wait),
        Payer::Phone => crate::x402_phone::pay(bolt11, network, max_fee_msat, wait, resource),
    }
}

/// Read `--max-msat` and `--max-fee-msat`, both optional.
pub(crate) fn flags(args: &Args) -> Result<Flags, String> {
    let max_msat = match args.number::<u64>("max-msat", 0)? {
        0 if args.option("max-msat").is_some() => {
            return Err("--max-msat must be positive".into());
        }
        0 => None,
        max => Some(max),
    };
    let max_fee_msat = args
        .option("max-fee-msat")
        .map(|_| args.number::<u64>("max-fee-msat", 0))
        .transpose()?;
    Ok(Flags {
        max_msat,
        max_fee_msat,
    })
}

pub(crate) fn load_policy() -> Result<Option<Policy>, String> {
    Policy::load(&x402_home().join(POLICY_FILE)).map_err(|error| error.to_string())
}

pub(crate) fn open_ledger() -> Ledger {
    Ledger::open(&x402_home().join(LEDGER_FILE))
}

/// The ceilings for a call to `provider` (the invoice's `payTo`, when known)
/// and `capability`, from the flags and the policy.
pub(crate) fn limits(
    policy: Option<&Policy>,
    flags: Flags,
    provider: Option<&str>,
    capability: Option<&str>,
) -> Result<Limits, String> {
    Policy::limits(policy, flags, provider, capability).map_err(|error| error.to_string())
}

/// Apply the wallet default only when neither flags nor policy set a fee cap.
/// Before a native challenge arrives, use the amount ceiling for its request.
pub(crate) fn payer_limits(
    policy: Option<&Policy>,
    flags: Flags,
    provider: Option<&str>,
    capability: Option<&str>,
    payer: Payer,
    amount_msat: Option<u64>,
) -> Result<Limits, String> {
    let mut resolved = limits(policy, flags, provider, capability)?;
    let policy_fee = policy.and_then(|p| p.ceiling(provider, capability).0.max_fee_msat);
    if payer == Payer::Wallet && flags.max_fee_msat.or(policy_fee).is_none() {
        resolved.max_fee_msat = (amount_msat.unwrap_or(resolved.max_msat) / 50).max(10_000);
    }
    Ok(resolved)
}

/// Refuse `amount_msat` to `provider` if the ceiling, the allowlist, or the
/// daily cap says so. Nothing is paid on an error.
pub(crate) fn admit(
    policy: Option<&Policy>,
    limits: Limits,
    provider: &str,
    amount_msat: u64,
) -> Result<(), String> {
    let spent = open_ledger()
        .spent_since(openagents_x402::unix_now().saturating_sub(DAY_SECS))
        .map_err(|error| error.to_string())?;
    Policy::admit(policy, limits, provider, amount_msat, spent)
        .map_err(|error| format!("{error}; nothing was paid"))
}

/// Write one payment to the ledger. A ledger error never undoes a payment,
/// so it is reported on stderr and the call continues.
pub(crate) fn record_payment(
    spend: &Spend,
    network: &str,
    provider: &str,
    proof: &openagents_wallet::Proof,
    phase: &str,
) {
    let entry = Entry {
        paid_at: openagents_x402::unix_now(),
        binding: spend.binding.to_owned(),
        network: network.to_owned(),
        provider: provider.to_owned(),
        capability: spend.capability.clone(),
        resource: spend.resource.clone(),
        amount_msat: proof.amount_msat,
        fee_msat: proof.fee_msat,
        payment_hash: proof.payment_hash.clone(),
        phase: phase.to_owned(),
    };
    if let Err(error) = open_ledger().append(&entry) {
        eprintln!("x402: ledger: {error}");
    }
}

/// Refuse before any request goes out when neither a flag nor a policy
/// file can bound the call.
pub(crate) fn ceiling_present(flags: Flags) -> Result<(), String> {
    if flags.max_msat.is_some() || x402_home().join(POLICY_FILE).is_file() {
        return Ok(());
    }
    Err("no --max-msat and no policy; give --max-msat N or run `openagents x402 policy set --max-msat N`".into())
}

/// Refuse a toll the configured LSP would never forward.
pub(crate) fn toll_floor(wallet_config: &WalletConfig, msat: u64) -> Result<(), String> {
    match wallet_config
        .lsp
        .as_ref()
        .and_then(|lsp| lsp.min_payment_msat)
    {
        Some(min) if msat < min => Err(format!(
            "--msat {msat} is below the LSP's smallest forwarded payment, {min} msat; buyers could never settle it"
        )),
        _ => Ok(()),
    }
}

/// Note how the call paid for by `payment_hash` ended.
pub(crate) fn set_phase(payment_hash: &str, phase: &str) {
    if let Err(error) = open_ledger().set_phase(payment_hash, phase) {
        eprintln!("x402: ledger: {error}");
    }
}

/// `--expiry SECONDS`, or its earlier name `--timeout`; default 300.
pub(crate) fn expiry(args: &Args) -> Result<u32, String> {
    let name = if args.option("expiry").is_some() {
        "expiry"
    } else {
        "timeout"
    };
    match args.number::<u32>(name, 300)? {
        0 => Err(format!("--{name} must be positive")),
        seconds => Ok(seconds),
    }
}

/// Pick the one requirement of `required` that is a valid exact/lnbtc
/// invoice for `request_hash` under `profiles`, pin it to the capability if
/// one was named, and pay it from the wallet. Returns the payload to retry
/// with, the proof, and the amount paid.
/// The first offered requirement whose invoice is valid for this request.
pub(crate) fn offered<'a>(
    required: &'a openagents_x402::PaymentRequired,
    request_hash: &str,
    profiles: SupportedProfiles,
) -> Result<(&'a nostr::x402::PaymentRequirements, nostr::x402::Invoice), String> {
    let now = openagents_x402::unix_now();
    required
        .accepts
        .iter()
        .find_map(|terms| {
            validate_challenge(
                terms,
                request_hash,
                now,
                nostr::x402::DEFAULT_CLOCK_SKEW,
                profiles,
            )
            .ok()
            .map(|invoice| (terms, invoice))
        })
        .ok_or_else(|| {
            "no offered payment requirement is a valid exact/lnbtc invoice for this request".into()
        })
}

pub(crate) fn buy(
    required: &openagents_x402::PaymentRequired,
    request_hash: &str,
    profiles: SupportedProfiles,
    binding: &str,
    descriptor: Option<&(PaidCapability, String)>,
    spend: Spend,
) -> Result<(PaymentPayload, openagents_wallet::Proof, u64), String> {
    let (terms, invoice) = offered(required, request_hash, profiles)?;
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
    let policy = load_policy()?;
    let limits = payer_limits(
        policy.as_ref(),
        spend.flags,
        Some(&terms.pay_to),
        spend.capability.as_deref(),
        spend.payer,
        Some(invoice.amount_msat()),
    )?;
    admit(
        policy.as_ref(),
        limits,
        &terms.pay_to,
        invoice.amount_msat(),
    )?;

    let bolt11 = terms
        .extra
        .get("invoice")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let proof = pay_by(
        spend.payer,
        &bolt11,
        &terms.network,
        limits.max_fee_msat,
        spend.wait,
        &spend.resource,
    )?;
    record_payment(&spend, &terms.network, &terms.pay_to, &proof, "paid");

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
    max_fee_msat: u64,
    wait: u64,
) -> Result<openagents_wallet::Proof, String> {
    let (wallet, wallet_config) = open_wallet().map_err(|error| error.to_string())?;
    if network_id(wallet_config.network.as_str()) != Some(network) {
        let _ = wallet.stop();
        return Err(format!(
            "the invoice is on {network} but this wallet is on {}",
            wallet_config.network.as_str()
        ));
    }
    let proof = wallet.pay(bolt11, max_fee_msat, Duration::from_secs(wait));
    let stopped = wallet.stop();
    let proof = match proof {
        Ok(proof) => proof,
        Err(WalletError::Pending {
            payment_hash,
            waited_secs,
        }) => {
            return Err(format!(
                "payment {payment_hash} is still pending after {waited_secs}s; run `openagents x402 node lookup {payment_hash}`, then retry to reuse the proof"
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
    let timeout = match expiry(&args) {
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
    // A paid server sells only the groups its operator names, and never
    // one that moves money, holds keys, or opens shells (audit CLI-01).
    if tools.is_empty() {
        return output.usage(
            "x402",
            "mcp-serve needs at least one --tool GROUP to sell",
            USAGE,
        );
    }
    if let Some(refused) = tools.iter().find(|t| !crate::mcp::served(t)) {
        let why = if crate::mcp::NEVER_SERVED.contains(&refused.as_str()) {
            "is never served over MCP"
        } else {
            "has no read-only command to serve"
        };
        return output.usage("x402", &format!("--tool {refused} {why}"), USAGE);
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
    if let Err(message) = toll_floor(&wallet_config, msat) {
        let _ = wallet.stop();
        return output.fail("x402", &message);
    }
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
    let flags = match flags(&args).and_then(|flags| ceiling_present(flags).map(|()| flags)) {
        Ok(flags) => flags,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let wait: u64 = match args.number("wait", 60) {
        Ok(wait) => wait,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let payer = match payer(&args, true) {
        Ok(payer) => payer,
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
        Spend {
            flags,
            capability: args.option("cap").map(str::to_owned),
            wait,
            binding: "mcp:1",
            resource: format!("{server} {tool}"),
            payer,
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
                "paid retry failed: {message}; proof for payment {} is in `openagents x402 node lookup`",
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
    if let Some(proof) = proof {
        set_phase(
            &proof.payment_hash,
            if is_error { "tool_error" } else { "completed" },
        );
    }
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
    if let Some(proof) = proof {
        set_phase(&proof.payment_hash, &format!("http_{status}"));
    }
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

fn policy(output: &Output, words: &[String]) -> u8 {
    let path = x402_home().join(POLICY_FILE);
    let (verb, rest) = match words.first().map(String::as_str) {
        None | Some("show") => ("show", &words[words.len().min(1)..]),
        Some(verb) => (verb, &words[1..]),
    };
    let mut current = match Policy::load(&path) {
        Ok(policy) => policy,
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    let show = |output: &Output, policy: Option<&Policy>, path: &std::path::Path| -> u8 {
        let value = json!({
            "path": path,
            "present": policy.is_some(),
            "policy": policy,
        });
        output.emit(&value, |value| match policy {
            None => format!("no policy at {} (flags decide every call)", path.display()),
            Some(_) => format!(
                "{}\n{}",
                path.display(),
                serde_json::to_string_pretty(&value["policy"]).unwrap_or_default()
            ),
        });
        0
    };
    match verb {
        "show" => show(output, current.as_ref(), &path),
        "set" => {
            let args = match Args::parse(rest, &[]) {
                Ok(args) => args,
                Err(message) => return output.usage("x402", &message, USAGE),
            };
            let field = |name: &str| -> Result<Option<Option<u64>>, String> {
                match args.option(name) {
                    None => Ok(None),
                    Some("-") => Ok(Some(None)),
                    Some(text) => text
                        .parse::<u64>()
                        .map(|n| Some(Some(n)))
                        .map_err(|_| format!("--{name} takes a number or `-`, not `{text}`")),
                }
            };
            let (max_msat, max_fee_msat, daily) = match (
                field("max-msat"),
                field("max-fee-msat"),
                field("daily-cap-msat"),
            ) {
                (Ok(a), Ok(b), Ok(c)) => (a, b, c),
                (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => {
                    return output.usage("x402", &m, USAGE);
                }
            };
            if max_msat.is_none() && max_fee_msat.is_none() && daily.is_none() {
                return output.usage("x402", "policy set needs a field to set", USAGE);
            }
            let mut policy = current.take().unwrap_or_default();
            if let Some(daily) = daily {
                if args.option("provider").is_some() || args.option("cap").is_some() {
                    return output.usage(
                        "x402",
                        "--daily-cap-msat applies to the whole wallet, not one provider or capability",
                        USAGE,
                    );
                }
                policy.daily_cap_msat = daily;
            }
            let apply = |ceiling: &mut Ceiling| {
                if let Some(max) = max_msat {
                    ceiling.max_msat = max;
                }
                if let Some(fee) = max_fee_msat {
                    ceiling.max_fee_msat = fee;
                }
            };
            match (args.option("provider"), args.option("cap")) {
                (Some(_), Some(_)) => {
                    return output.usage("x402", "name --provider or --cap, not both", USAGE);
                }
                (Some(provider), None) => {
                    let entry = policy.providers.entry(provider.to_owned()).or_default();
                    apply(entry);
                    if *entry == Ceiling::default() {
                        policy.providers.remove(provider);
                    }
                }
                (None, Some(cap)) => {
                    let entry = policy.capabilities.entry(cap.to_owned()).or_default();
                    apply(entry);
                    if *entry == Ceiling::default() {
                        policy.capabilities.remove(cap);
                    }
                }
                (None, None) => apply(&mut policy.default),
            }
            if let Err(error) = policy.save(&path) {
                return output.fail("x402", &error.to_string());
            }
            show(output, Some(&policy), &path)
        }
        "allow" | "deny" => {
            if rest.is_empty() {
                return output.usage("x402", &format!("policy {verb} needs NODE_ID..."), USAGE);
            }
            let mut policy = current.take().unwrap_or_default();
            for node in rest {
                if verb == "allow" {
                    if !policy.allow.contains(node) {
                        policy.allow.push(node.clone());
                    }
                } else {
                    policy.allow.retain(|n| n != node);
                }
            }
            if let Err(error) = policy.save(&path) {
                return output.fail("x402", &error.to_string());
            }
            show(output, Some(&policy), &path)
        }
        other => output.usage("x402", &format!("unknown policy command `{other}`"), USAGE),
    }
}

fn ledger(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let since: u64 = match args.number("since", 0) {
        Ok(0) => 0,
        Ok(seconds) => openagents_x402::unix_now().saturating_sub(seconds),
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let ledger = open_ledger();
    let entries: Vec<Entry> = match ledger.entries() {
        Ok(entries) => entries
            .into_iter()
            .filter(|e| e.paid_at >= since)
            .filter(|e| args.option("binding").is_none_or(|b| b == e.binding))
            .filter(|e| args.option("provider").is_none_or(|p| p == e.provider))
            .collect(),
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    let amount: u64 = entries.iter().map(|e| e.amount_msat).sum();
    let fees: u64 = entries.iter().map(|e| e.fee_msat).sum();
    let value = json!({
        "path": ledger.path(),
        "count": entries.len(),
        "amount_msat": amount,
        "fee_msat": fees,
        "entries": entries,
    });
    output.emit(&value, |_| {
        let mut lines: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "{}  {:<19} {}  {:>10} msat +{:<6} {}  {}  {}",
                    e.paid_at,
                    e.binding,
                    &e.provider[..e.provider.len().min(12)],
                    e.amount_msat,
                    e.fee_msat,
                    &e.payment_hash[..e.payment_hash.len().min(12)],
                    e.phase,
                    e.resource
                )
            })
            .collect();
        lines.push(format!(
            "{} payments, {amount} msat + {fees} msat fees ({})",
            entries.len(),
            ledger.path().display()
        ));
        lines.join("\n")
    });
    0
}

/// A refusal on the way to a paid capability: the caller's words, or the
/// relay and record.
enum Refusal {
    Usage(String),
    Failure(String),
}

/// A resolved `oa-x402-v1` adapter: the advertised endpoint and descriptor.
pub(crate) struct PaidCapability {
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

fn listing_tags(args: &Args) -> Vec<nostr::domain::Tag> {
    ["test", "dev"]
        .into_iter()
        .filter(|name| args.switch(name))
        .map(|name| nostr::domain::Tag::new(vec!["t".into(), format!("oa:{name}")]))
        .collect()
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
    let central = binding == "http:1" && !args.switch("local");
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail("x402", &message),
    };
    // http:1 by default: the resource this key published, at the front's
    // URL, paid to the front's receiver.
    let registered = if central {
        match registered(&pay_front(args.option("front")), slug, signer.pubkey()) {
            Ok(registered) => Some(registered),
            Err(message) => return output.fail("x402", &message),
        }
    } else {
        None
    };
    let url = match (&registered, args.option("url"), native) {
        (Some(found), Some(url), _) if url != found.url => {
            return output.usage(
                "x402",
                &format!(
                    "{slug} is sold at {}, not {url}; drop --url or pass --local",
                    found.url
                ),
                USAGE,
            );
        }
        (Some(found), _, _) => found.url.as_str(),
        (None, _, true) => "",
        (None, Some(url), false) => url,
        (None, None, false) => {
            return output.usage("x402", "advertise --local needs --url PUBLIC_URL", USAGE);
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
    let (pay_to, network) = match &registered {
        Some(found) => (found.pay_to.clone(), found.network.as_str()),
        None => {
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
            (pay_to, network)
        }
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
    let mut tags = vec![
        nostr::domain::Tag::new(vec!["d".to_owned(), slug.to_owned()]),
        nostr::domain::Tag::new(vec!["t".to_owned(), nostr::cap::CAP_MARKER.to_owned()]),
        nostr::domain::Tag::new(vec!["t".to_owned(), definition.profile.tag().to_owned()]),
        nostr::domain::Tag::new(vec![
            "t".to_owned(),
            format!("oa:transport:{}", definition.transport),
        ]),
    ];
    tags.extend(listing_tags(&args));
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

/// A resource registered on the pay front, as `GET /v1/resources/{name}`
/// describes it.
pub(crate) struct Registered {
    pub(crate) url: String,
    pub(crate) pay_to: String,
    pub(crate) network: String,
}

/// Read `name`'s registration from `front` and hold it to `owner`.
fn registered(front: &str, name: &str, owner: &str) -> Result<Registered, String> {
    let url = format!("{front}{}/{name}", openagents_x402::hosted::REGISTER_PATH);
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .and_then(|client| client.get(&url).send())
        .map_err(|e| format!("{url}: {e}"))?;
    let status = response.status();
    let value: Value = response.json().map_err(|e| format!("{url}: {e}"))?;
    if status.as_u16() == 404 {
        return Err(format!(
            "{name} is not published on {front}; run `openagents x402 publish` first, or pass --local to advertise a self-hosted URL"
        ));
    }
    if !status.is_success() {
        return Err(format!("{url} answered {status}: {value}"));
    }
    if value["owner"].as_str() != Some(owner) {
        return Err(format!("{name} on {front} is published by another key"));
    }
    let field = |name: &str| {
        value[name]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{url} gave no {name}"))
    };
    Ok(Registered {
        url: field("url")?,
        pay_to: field("pay_to")?,
        network: field("network")?,
    })
}

/// The registration `publish` signs, from its flags.
pub(crate) fn registration(args: &Args) -> Result<openagents_x402::hosted::Registration, String> {
    let upstream = args
        .option("upstream")
        .ok_or("publish needs --upstream URL")?
        .to_owned();
    let sats: u64 = args.number("price-sats", 0)?;
    if sats == 0 {
        return Err("publish needs --price-sats N (at least 1)".into());
    }
    let payout = args
        .option("payout")
        .ok_or("publish needs --payout ADDRESS")?
        .to_owned();
    if pay_ledger::payee::classify(&payout).is_none() {
        return Err(
            "--payout must be a mainnet Spark address, Lightning address, or node key".into(),
        );
    }
    let resource = match args.option("resource") {
        Some(name) => name.to_owned(),
        None => upstream
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .split_once("://")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split('/').skip(1).filter(|s| !s.is_empty()).last())
            .map(str::to_ascii_lowercase)
            .ok_or("publish needs --resource NAME (the URL has no path to name it by)")?,
    };
    let registration = openagents_x402::hosted::Registration {
        v: 1,
        resource,
        upstream,
        method: args.option("method").unwrap_or("POST").to_ascii_uppercase(),
        price_msat: sats.checked_mul(1000).ok_or("--price-sats is too large")?,
        payout,
        summary: args.option("summary").map(str::to_owned),
    };
    registration.check()?;
    Ok(registration)
}

fn publish(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let registration = match registration(&args) {
        Ok(registration) => registration,
        Err(message) => return output.usage("x402", &message, USAGE),
    };
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail("x402", &message),
    };
    let front = pay_front(args.option("front"));
    let body = match serde_json::to_string(&registration) {
        Ok(body) => body,
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    if args.switch("dry-run") {
        let doc = json!({
            "front": front,
            "owner": signer.pubkey(),
            "url": format!("{front}/x/{}", registration.resource),
            "registration": registration,
            "published": false,
        });
        output.emit(&doc, |v| {
            format!(
                "dry run: {} would sell {} at {} for {} msat (not published)",
                v["owner"].as_str().unwrap_or(""),
                v["registration"]["upstream"].as_str().unwrap_or(""),
                v["url"].as_str().unwrap_or(""),
                v["registration"]["price_msat"]
            )
        });
        return 0;
    }
    match post_registration(&front, &signer, &body) {
        Ok(value) => {
            output.emit(&value, |v| {
                format!(
                    "published {} at {}\n  {} msat per {} call, paid to {} ({}); your share goes to {}\n  check the OpenAgents-Paid header against {front}{}",
                    v["resource"].as_str().unwrap_or(""),
                    v["url"].as_str().unwrap_or(""),
                    v["price_msat"],
                    v["method"].as_str().unwrap_or(""),
                    v["pay_to"].as_str().unwrap_or(""),
                    v["network"].as_str().unwrap_or(""),
                    registration.payout,
                    openagents_x402::hosted::KEY_PATH,
                )
            });
            0
        }
        Err(message) => output.fail("x402", &message),
    }
}

/// Post one signed registration to `front`; the front's description of
/// the registered resource on success.
pub(crate) fn post_registration(
    front: &str,
    signer: &nostr::domain::RelaySigner,
    body: &str,
) -> Result<Value, String> {
    let url = format!("{front}{}", openagents_x402::hosted::REGISTER_PATH);
    let authorization = openagents_x402::hosted::http_auth(
        signer,
        "POST",
        &url,
        body.as_bytes(),
        openagents_x402::unix_now(),
        vec![],
    );
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .and_then(|client| {
            client
                .post(&url)
                .header("authorization", authorization)
                .header("content-type", "application/json")
                .body(body.to_owned())
                .send()
        })
        .map_err(|e| format!("{url}: {e}"))?;
    let status = response.status();
    let value: Value = response.json().map_err(|e| format!("{url}: {e}"))?;
    if status.is_success() {
        Ok(value)
    } else {
        Err(format!(
            "{front} refused the registration ({status}): {}",
            value["error"]["message"]
                .as_str()
                .or(value["error"]["type"].as_str())
                .unwrap_or_default()
        ))
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
    fn advertisement_marks_only_explicit_test_and_dev_flags() {
        for (words, expected) in [
            (vec![], vec![]),
            (vec!["--test"], vec!["oa:test"]),
            (vec!["--dev"], vec!["oa:dev"]),
            (vec!["--test", "--dev"], vec!["oa:test", "oa:dev"]),
        ] {
            let words: Vec<String> = words.into_iter().map(str::to_owned).collect();
            let args = Args::parse(&words, SWITCHES).unwrap();
            let tags = listing_tags(&args);
            let values: Vec<&str> = tags.iter().map(|t| t.as_slice()[1].as_str()).collect();
            assert_eq!(values, expected);
        }
    }

    #[test]
    fn wallet_fee_defaults_use_price_not_spending_ceiling() {
        for (sats, fee_sats) in [(5, 10), (1_000, 20), (100_000, 2_000)] {
            let flags = Flags {
                max_msat: Some(1_000_000_000),
                max_fee_msat: None,
            };
            let resolved =
                payer_limits(None, flags, None, None, Payer::Wallet, Some(sats * 1_000)).unwrap();
            assert_eq!(resolved.max_fee_msat, fee_sats * 1_000);
            assert!(resolved.max_fee_msat >= 3_000);
            for payer in [Payer::Node, Payer::Phone] {
                assert_eq!(
                    payer_limits(None, flags, None, None, payer, Some(sats * 1_000)).unwrap(),
                    limits(None, flags, None, None).unwrap()
                );
            }
        }
        let flags = Flags {
            max_msat: Some(u64::MAX),
            max_fee_msat: None,
        };
        assert_eq!(
            payer_limits(None, flags, None, None, Payer::Wallet, Some(u64::MAX))
                .unwrap()
                .max_fee_msat,
            u64::MAX / 50
        );
    }

    #[test]
    fn wallet_fee_overrides_keep_policy_precedence_and_zero() {
        let mut policy = Policy {
            default: Ceiling {
                max_msat: Some(100_000),
                max_fee_msat: Some(1_000),
            },
            ..Policy::default()
        };
        policy.providers.insert(
            "node".into(),
            Ceiling {
                max_msat: None,
                max_fee_msat: Some(2_000),
            },
        );
        policy.capabilities.insert(
            "cap".into(),
            Ceiling {
                max_msat: None,
                max_fee_msat: Some(0),
            },
        );
        for (provider, capability, expected) in [
            (None, None, 1_000),
            (Some("node"), None, 2_000),
            (Some("node"), Some("cap"), 0),
        ] {
            assert_eq!(
                payer_limits(
                    Some(&policy),
                    Flags::default(),
                    provider,
                    capability,
                    Payer::Wallet,
                    Some(5_000)
                )
                .unwrap()
                .max_fee_msat,
                expected
            );
            let flags = Flags {
                max_msat: None,
                max_fee_msat: Some(17),
            };
            assert_eq!(
                payer_limits(
                    Some(&policy),
                    flags,
                    provider,
                    capability,
                    Payer::Wallet,
                    Some(5_000)
                )
                .unwrap()
                .max_fee_msat,
                17
            );
        }
        assert_eq!(
            payer_limits(
                None,
                Flags {
                    max_msat: Some(5_000),
                    max_fee_msat: None
                },
                None,
                None,
                Payer::Wallet,
                None
            )
            .unwrap()
            .max_fee_msat,
            10_000
        );
    }

    fn wallet_config(min_payment_msat: Option<u64>) -> WalletConfig {
        let mut lsp = config::Lsp::parse(
            &format!("{}@lsp.example:9735", "ab".repeat(33)),
            None,
            config::LspProtocol::Lsps1,
        )
        .unwrap();
        lsp.min_payment_msat = min_payment_msat;
        WalletConfig {
            network: config::Network::Testnet,
            esplora_url: String::new(),
            listen: None,
            lsp: Some(lsp),
            trusted_peers: Vec::new(),
        }
    }

    #[test]
    fn buying_pays_from_the_wallet_unless_only_a_node_is_set_up() {
        assert_eq!(default_payer(true, true), Payer::Wallet);
        assert_eq!(default_payer(true, false), Payer::Wallet);
        assert_eq!(default_payer(false, false), Payer::Wallet);
        assert_eq!(default_payer(false, true), Payer::Node);
        let args = |words: &[&str]| {
            Args::parse(
                &words.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>(),
                SWITCHES,
            )
            .expect("args")
        };
        assert_eq!(payer(&args(&["--pay-with", "node"]), true), Ok(Payer::Node));
        assert_eq!(
            payer(&args(&["--pay-with", "wallet"]), false),
            Ok(Payer::Wallet)
        );
        assert_eq!(
            payer(&args(&["--pay-with", "phone"]), true),
            Ok(Payer::Phone)
        );
        assert!(payer(&args(&["--pay-with", "phone"]), false).is_err());
    }

    #[test]
    fn a_toll_below_the_lsp_minimum_is_refused() {
        assert!(toll_floor(&wallet_config(Some(1_000)), 999).is_err());
        assert_eq!(toll_floor(&wallet_config(Some(1_000)), 1_000), Ok(()));
        assert_eq!(toll_floor(&wallet_config(None), 1), Ok(()));
    }

    #[test]
    fn expiry_takes_the_new_flag_or_the_old_name_and_refuses_zero() {
        let parse = |words: &[&str]| {
            Args::parse(
                &words.iter().map(|w| w.to_string()).collect::<Vec<_>>(),
                &[],
            )
            .unwrap()
        };
        assert_eq!(expiry(&parse(&[])), Ok(300));
        assert_eq!(expiry(&parse(&["--expiry", "60"])), Ok(60));
        assert_eq!(expiry(&parse(&["--timeout", "45"])), Ok(45));
        assert!(expiry(&parse(&["--expiry", "0"])).is_err());
    }

    #[test]
    fn the_flags_refuse_a_zero_ceiling_and_pass_none_through() {
        let parse = |words: &[&str]| {
            Args::parse(
                &words.iter().map(|w| w.to_string()).collect::<Vec<_>>(),
                &[],
            )
            .unwrap()
        };
        assert_eq!(flags(&parse(&[])), Ok(Flags::default()));
        assert_eq!(
            flags(&parse(&["--max-msat", "5", "--max-fee-msat", "1"])),
            Ok(Flags {
                max_msat: Some(5),
                max_fee_msat: Some(1),
            })
        );
        assert!(flags(&parse(&["--max-msat", "0"])).is_err());
    }

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
