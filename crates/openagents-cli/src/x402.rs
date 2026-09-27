//! `openagents x402`: sell one operation over HTTP for an exact Lightning
//! payment, or buy one. Both roles use the wallet under `openagents wallet`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nostr::x402::{SupportedProfiles, binding_hash, http_binding, validate_challenge};
use openagents_wallet::ldk::LdkWallet;
use openagents_wallet::{LightningWallet, WalletConfig, WalletError, config};
use openagents_x402::server::{Executor, Receiver, Resource};
use openagents_x402::{
    FileReplayStore, PAYMENT_REQUIRED, PAYMENT_RESPONSE, PAYMENT_SIGNATURE, PaymentPayload,
    SettlementResponse, network_id, wire,
};
use serde_json::{Map, Value, json};

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
        [--wait SECONDS] [--show-proof]
                          Buy one call: read the 402, check the invoice against
                          this request, refuse above --max-msat, pay from the
                          wallet, retry with the preimage, print the body.
Replay records live in ~/.openagents/x402/replay. The preimage is printed
only with --show-proof. Add --json before `x402` for one JSON document.";

const SWITCHES: &[&str] = &["show-proof"];

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
        other => output.usage("x402", &format!("unknown command `{other}`"), USAGE),
    }
}

fn replay_dir() -> PathBuf {
    match std::env::var_os("OPENAGENTS_X402_HOME") {
        Some(home) => PathBuf::from(home),
        None => config::home()
            .parent()
            .map(|p| p.join("x402"))
            .unwrap_or_else(|| PathBuf::from("x402")),
    }
    .join("replay")
}

fn open_wallet() -> Result<(LdkWallet, WalletConfig), WalletError> {
    let home = config::home();
    let wallet_config = WalletConfig::load(&home)?;
    let (mnemonic, _) = config::load_or_create_seed(&home, false, String::new)?;
    Ok((
        LdkWallet::open(&home, &wallet_config, &mnemonic)?,
        wallet_config,
    ))
}

fn fail_wallet(output: &Output, error: WalletError) -> u8 {
    match error {
        WalletError::Invalid(message) => output.usage("x402", &message, USAGE),
        other => output.fail("x402", &other.to_string()),
    }
}

struct Node(Arc<LdkWallet>);

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
    let now = openagents_x402::unix_now();
    let profiles = SupportedProfiles {
        http: true,
        mcp: false,
        native: false,
    };
    let Some((terms, invoice)) = required.accepts.iter().find_map(|terms| {
        validate_challenge(
            terms,
            &request_hash,
            now,
            nostr::x402::DEFAULT_CLOCK_SKEW,
            profiles,
        )
        .ok()
        .map(|invoice| (terms, invoice))
    }) else {
        return output.fail(
            "x402",
            "no offered payment requirement is a valid exact/lnbtc invoice for this request",
        );
    };
    if invoice.amount_msat() > max_msat {
        return output.fail(
            "x402",
            &format!(
                "the resource costs {} msat, above --max-msat {max_msat}",
                invoice.amount_msat()
            ),
        );
    }

    let (wallet, wallet_config) = match open_wallet() {
        Ok(opened) => opened,
        Err(error) => return fail_wallet(output, error),
    };
    if network_id(wallet_config.network.as_str()) != Some(terms.network.as_str()) {
        let _ = wallet.stop();
        return output.fail(
            "x402",
            &format!(
                "the invoice is on {} but this wallet is on {}",
                terms.network,
                wallet_config.network.as_str()
            ),
        );
    }
    let bolt11 = terms
        .extra
        .get("invoice")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let proof = wallet.pay(&bolt11, max_fee, Duration::from_secs(wait));
    let stopped = wallet.stop();
    let proof = match proof {
        Ok(proof) => proof,
        Err(WalletError::Pending {
            payment_hash,
            waited_secs,
        }) => {
            return output.fail(
                "x402",
                &format!(
                    "payment {payment_hash} is still pending after {waited_secs}s; run `openagents wallet lookup {payment_hash}`, then retry this fetch to reuse the proof"
                ),
            );
        }
        Err(error) => return fail_wallet(output, error),
    };
    if let Err(error) = stopped {
        return output.fail("x402", &error.to_string());
    }

    let mut payload = Map::new();
    payload.insert("preimage".into(), Value::String(proof.preimage.clone()));
    let signature = match wire::encode_header(&PaymentPayload {
        x402_version: 2,
        resource: Some(required.resource.clone()),
        accepted: terms.clone(),
        payload,
        extensions: None,
    }) {
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
        Some(invoice.amount_msat()),
        show_proof,
    )
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
