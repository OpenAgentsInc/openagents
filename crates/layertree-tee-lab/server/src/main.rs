//! One fresh-key signet tree per measured guest; no private-key import API.
use bitcoin::{
    OutPoint, Transaction,
    consensus::encode::serialize_hex,
    hashes::{Hash, sha256},
    key::XOnlyPublicKey,
    secp256k1::{Secp256k1, schnorr::Signature},
};
use layertree_signet_service::{Core, Request, bounded_json, client, request, response};
use layertree_tee_lab::{
    attest::report_data,
    enclave::{Enclave, Prepared},
    events::{Body, Network, Writer},
    mirror::{MirrorPolicy, Receipt},
    rpc::{self, ServiceConfig},
    rules::Record,
    tree::tree_id,
    unroll::UnrollPackage,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const CONFIG: &str = include_str!("../service.json");
const SOURCE_COMMIT: &str = match option_env!("LAYERTREE_SOURCE_COMMIT") {
    Some(value) => value,
    None => "uncommitted",
};
struct CoreProcess(std::process::Child);
impl Drop for CoreProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Service {
    enclave: Enclave,
    config: ServiceConfig,
    core: Core,
    http: reqwest::blocking::Client,
    next: Option<XOnlyPublicKey>,
    pending: Option<Prepared>,
    unroll: Option<UnrollPackage>,
    last: Option<Transaction>,
    log: Writer,
    certificate: [u8; 32],
    replies: HashMap<String, ([u8; 32], Value, u64)>,
    genesis: Option<Record>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_secs()
}
fn typed<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "invalid request fields".into())
}

impl Service {
    fn new(certificate: [u8; 32]) -> Result<Self, String> {
        let config: ServiceConfig =
            serde_json::from_str(CONFIG).map_err(|_| "invalid measured configuration")?;
        let mut enclave = Enclave::launch_in_guest(config.enclave()?);
        enclave.set_guardian(
            config
                .operator_key
                .parse()
                .map_err(|_| "invalid guardian")?,
        );
        enclave.set_mirror_policy(MirrorPolicy {
            mirrors: config
                .mirrors
                .iter()
                .map(|m| m.key.parse().map_err(|_| "invalid mirror key".to_string()))
                .collect::<Result<_, _>>()?,
            m: config.mirror_threshold,
        });
        Ok(Self {
            enclave,
            config,
            core: Core::new("/state/bitcoin/signet/.cookie")?,
            http: client()?,
            next: None,
            pending: None,
            unroll: None,
            last: None,
            log: Writer::new(Network::Signet),
            certificate,
            replies: HashMap::new(),
            genesis: None,
        })
    }
    fn mirror(
        &self,
        record: &Record,
        unroll: Option<&UnrollPackage>,
    ) -> Result<Vec<Receipt>, String> {
        let payload = rpc::Stored {
            record: rpc::hex(&record.encode()),
            unroll: unroll.map(|p| rpc::hex(&p.encode())),
        };
        let mut receipts = Vec::new();
        for mirror in &self.config.mirrors {
            let result: Result<Receipt, String> = (|| {
                let value = bounded_json(
                    self.http
                        .post(format!("{}/store", mirror.url))
                        .json(&payload)
                        .send()
                        .map_err(|_| "mirror unavailable")?,
                )?;
                let receipt: rpc::Receipt = typed(value)?;
                if receipt.mirror != mirror.key {
                    return Err("unexpected mirror key".into());
                }
                Ok(Receipt {
                    mirror: receipt.mirror.parse().map_err(|_| "invalid mirror key")?,
                    sig: receipt
                        .signature
                        .parse()
                        .map_err(|_| "invalid mirror signature")?,
                })
            })();
            if let Ok(receipt) = result {
                receipts.push(receipt);
            }
        }
        let policy = MirrorPolicy {
            mirrors: self
                .config
                .mirrors
                .iter()
                .map(|m| m.key.parse().expect("validated mirror"))
                .collect(),
            m: self.config.mirror_threshold,
        };
        if policy.count(
            &Secp256k1::verification_only(),
            layertree_tee_lab::mirror::receipt_msg(record, unroll),
            &receipts,
        ) < policy.m
        {
            return Err("not enough mirrors stored this state".into());
        }
        Ok(receipts)
    }
    fn handle(&mut self, request: Request) -> Result<Value, String> {
        if request.method == "GET" {
            if request.path == "/health" {
                return self.core.call("getblockchaininfo", json!([]));
            }
            if request.path == "/state" {
                return Ok(serde_json::to_value(rpc::snapshot(
                    &self.enclave,
                    self.unroll.as_ref(),
                    self.core.ready_height().ok(),
                    self.last.as_ref(),
                ))
                .expect("snapshot"));
            }
            if request.path == "/events" {
                return Ok(json!({"events":self.log.to_jsonl()}));
            }
            if let Some(challenge) = request.path.strip_prefix("/attestation?challenge=") {
                let challenge = rpc::array(challenge)?;
                let token = layertree_tee_lab::guest::channel_token(
                    &report_data(self.enclave.params()),
                    &challenge,
                    &self.certificate,
                )
                .map_err(|_| "attestation is unavailable")?;
                return Ok(
                    json!({"schema":"openagents.layertree.live-attestation.v1","token":token,"params":rpc::Parameters::from(self.enclave.params()),"certificate_sha256":rpc::hex(&self.certificate),"source_commit":SOURCE_COMMIT }),
                );
            }
            return Err("unknown route".into());
        }
        if request.method != "POST" {
            return Err("unknown method".into());
        }
        let value: Value = serde_json::from_slice(&request.body).map_err(|_| "invalid JSON")?;
        // These messages carry their own member or guardian signature.
        if request.path == "/exit-request" {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Exit {
                pos: u64,
                signature: String,
            }
            let input: Exit = typed(value)?;
            let height = self.core.ready_height()?;
            self.enclave.set_height(height);
            let sig = input
                .signature
                .parse()
                .map_err(|_| "invalid member signature")?;
            self.enclave
                .request_exit(input.pos, &sig)
                .map_err(|e| format!("exit refused: {e:?}"))?;
            return Ok(json!({"accepted_at_height":height}));
        }
        if request.path == "/halt" {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Halt {
                signature: String,
            }
            let input: Halt = typed(value)?;
            self.enclave
                .halt(
                    &input
                        .signature
                        .parse()
                        .map_err(|_| "invalid guardian signature")?,
                )
                .map_err(|e| format!("HALT refused: {e:?}"))?;
            return Ok(json!({"halted":true}));
        }
        let signed: rpc::Authorized = typed(value)?;
        let operator = self
            .config
            .operator_key
            .parse()
            .map_err(|_| "invalid operator key")?;
        let digest = rpc::authenticate(&request.path, &signed, &operator, now())?;
        self.replies
            .retain(|_, (_, _, stamp)| now().saturating_sub(*stamp) < 300);
        if let Some((previous, result, _)) = self.replies.get(&signed.nonce) {
            return if *previous == digest {
                Ok(result.clone())
            } else {
                Err("nonce was already used".into())
            };
        }
        if self.replies.len() >= 1024 {
            return Err("too many recent requests".into());
        }
        let result = self.authorized(&request.path, signed.payload)?;
        self.replies
            .insert(signed.nonce, (digest, result.clone(), now()));
        Ok(result)
    }
    fn authorized(&mut self, path: &str, payload: Value) -> Result<Value, String> {
        let height = self.core.ready_height()?;
        self.enclave.set_height(height);
        match path {
            "/next" => {
                if self.pending.is_some() {
                    return Err("finish the prepared transition first".into());
                }
                let key = *self
                    .next
                    .get_or_insert_with(|| self.enclave.next_unroll_key());
                Ok(json!({"unroll_key":key.to_string()}))
            }
            "/genesis" => {
                let input: rpc::Genesis = typed(payload)?;
                let tx = rpc::transaction(&input.transaction)?;
                let tree = rpc::tree(self.config.depth, &input.leaves)?;
                if let (Some(record), Some(state), Some(pkg), Some(previous)) = (
                    &self.genesis,
                    self.enclave.state(),
                    &self.unroll,
                    &self.last,
                ) {
                    if tx != *previous || tree != state.tree {
                        return Err("another creation is already prepared".into());
                    }
                    self.mirror(record, Some(pkg))?;
                    if self.log.events().is_empty() {
                        self.log
                            .push(Some(height), Body::tree_open(self.enclave.params(), state));
                    }
                    return Ok(
                        json!({"outpoint":state.outpoint.to_string(),"unroll":rpc::hex(&pkg.encode()),"ephemeral_keys":self.enclave.ephemeral_keys_held()}),
                    );
                }
                if self.enclave.state().is_some() || self.next.is_none() {
                    return Err("tree is already open or needs an unroll key".into());
                }
                let funding = tx
                    .input
                    .first()
                    .ok_or("missing creation input")?
                    .previous_output;
                if tree_id(&funding.txid.to_byte_array(), funding.vout)
                    != self.enclave.params().tree_id
                {
                    return Err("creation input names another tree".into());
                }
                for item in &tx.input {
                    self.core.output(item.previous_output, true)?;
                }
                if tree
                    .leaves()
                    .values()
                    .any(|leaf| leaf.amount < self.config.dust_min)
                {
                    return Err("member amount is below dust".into());
                }
                let root = tree.root().map_err(|_| "invalid tree")?;
                let output = layertree_tee_lab::output::TreeOutput {
                    root_hash: root.hash,
                    unroll_key: self.next.expect("checked"),
                };
                let expected = bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(root.sum),
                    script_pubkey: output.script_pubkey(
                        &Secp256k1::verification_only(),
                        &self.enclave.params().enclave_key,
                        self.config.unroll_delay,
                    ),
                };
                if tx.output.first() != Some(&expected) {
                    return Err("creation output does not match the tree".into());
                }
                let point = OutPoint {
                    txid: tx.compute_txid(),
                    vout: 0,
                };
                let pkg = self
                    .enclave
                    .open(tree, point)
                    .map_err(|e| format!("creation refused: {e:?}"))?;
                self.next = None;
                // The genesis record has no chain OP_RETURN; mirrors also index
                // the package by the exact creation outpoint for discovery.
                let record = Record {
                    prev: funding,
                    changes: Vec::new(),
                    new_root: root,
                    next_unroll_key: Some(output.unroll_key),
                };
                self.unroll = Some(pkg.clone());
                self.last = Some(tx);
                self.genesis = Some(record.clone());
                self.mirror(&record, Some(&pkg))?;
                self.log.push(
                    Some(height),
                    Body::tree_open(self.enclave.params(), self.enclave.state().expect("opened")),
                );
                Ok(
                    json!({"outpoint":point.to_string(),"unroll":rpc::hex(&pkg.encode()),"ephemeral_keys":self.enclave.ephemeral_keys_held()}),
                )
            }
            "/prepare" => {
                if self.pending.is_some() {
                    return Err("finish the prepared transition first".into());
                }
                let input: rpc::Prepare = typed(payload)?;
                let tx = rpc::transaction(&input.transaction)?;
                if tx.input.len() > 16 {
                    return Err("too many inputs".into());
                }
                let state = self.enclave.state().ok_or("tree is not open")?;
                if self.core.output(state.outpoint, true)?
                    != state.txout(&Secp256k1::verification_only(), self.enclave.params())
                {
                    return Err("confirmed tree output differs".into());
                }
                let prevouts = tx
                    .input
                    .iter()
                    .map(|i| self.core.output(i.previous_output, true))
                    .collect::<Result<Vec<_>, _>>()?;
                let op = input.operation.op()?;
                let prepared = self
                    .enclave
                    .prepare(tx, &prevouts, &op)
                    .map_err(|e| format!("transition refused: {e:?}"))?;
                self.next = None;
                let wire = rpc::Prepared {
                    transaction: serialize_hex(&prepared.tx),
                    record: rpc::hex(&prepared.record.encode()),
                    unroll: prepared.unroll.as_ref().map(|p| rpc::hex(&p.encode())),
                    sighash: rpc::hex(&prepared.sighash),
                };
                self.pending = Some(prepared);
                Ok(serde_json::to_value(wire).expect("prepared"))
            }
            "/finish" => {
                let input: rpc::Finish = typed(payload)?;
                let pending = self.pending.as_ref().ok_or("no prepared transition")?;
                if input.transaction_id != pending.tx.compute_txid().to_string()
                    || input.signatures.len() > 16
                {
                    return Err("prepared transaction differs".into());
                }
                let signatures = input
                    .signatures
                    .iter()
                    .map(|s| s.parse().map_err(|_| "invalid party signature".to_string()))
                    .collect::<Result<Vec<Signature>, _>>()?;
                let before = self.enclave.state().ok_or("tree is not open")?.clone();
                self.core.output(before.outpoint, true)?;
                let receipts = self.mirror(&pending.record, pending.unroll.as_ref())?;
                let signed = self
                    .enclave
                    .finish(&signatures, &receipts)
                    .map_err(|e| format!("signature refused: {e:?}"))?;
                self.log.push(
                    Some(height),
                    Body::transition(
                        &pending.op,
                        &before,
                        &signed.record,
                        &signed.tx.compute_txid(),
                    ),
                );
                self.pending = None;
                self.genesis = None;
                self.unroll = signed.unroll;
                self.last = Some(signed.tx.clone());
                Ok(
                    json!({"transaction":serialize_hex(&signed.tx),"record":rpc::hex(&signed.record.encode()),"unroll":self.unroll.as_ref().map(|p|rpc::hex(&p.encode())),"ephemeral_keys":self.enclave.ephemeral_keys_held()}),
                )
            }
            _ => Err("unknown route".into()),
        }
    }
}

fn run() -> Result<(), String> {
    if std::env::args_os().len() != 1 {
        return Err("this service accepts no command override".into());
    }
    if SOURCE_COMMIT == "uncommitted" {
        return Err("build this service from a committed source tree".into());
    }
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| "cannot install TLS provider")?;
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec![rpc::HOSTNAME.into()])
            .map_err(|_| "cannot create guest TLS key")?;
    let certificate = sha256::Hash::hash(cert.der()).to_byte_array();
    let tls = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
            )
            .map_err(|_| "cannot configure TLS")?,
    );
    let service = Arc::new(Mutex::new(Service::new(certificate)?));
    // Core validates the signet chain inside this measured guest. No wallet,
    // external RPC, host-reported height, or key-management command is used.
    std::fs::create_dir_all("/state/bitcoin").map_err(|_| "cannot create Bitcoin directory")?;
    let mut core = CoreProcess(
        std::process::Command::new("/usr/local/bin/bitcoind")
            .args([
                "-datadir=/state/bitcoin",
                "-signet",
                "-server",
                "-listen=0",
                "-rpcbind=127.0.0.1",
                "-rpcallowip=127.0.0.1",
                "-rpcport=38332",
                "-disablewallet",
                "-prune=550",
                "-dbcache=512",
                "-printtoconsole=0",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| "cannot start signet validation")?,
    );
    let running = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("0.0.0.0:8443").map_err(|_| "cannot listen")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "cannot configure listener")?;
    println!(
        "{}",
        json!({"schema":"openagents.layertree.service-start.v1","params":rpc::Parameters::from(service.lock().map_err(|_|"service unavailable")?.enclave.params()),"certificate_sha256":rpc::hex(&certificate),"source_commit":SOURCE_COMMIT})
    );
    loop {
        if core
            .0
            .try_wait()
            .map_err(|_| "cannot supervise Bitcoin")?
            .is_some()
        {
            return Err("Bitcoin stopped".into());
        }
        match listener.accept() {
            Ok((socket, _)) => {
                if running.fetch_add(1, Ordering::SeqCst) >= 16 {
                    running.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                socket.set_read_timeout(Some(Duration::from_secs(60))).ok();
                socket.set_write_timeout(Some(Duration::from_secs(60))).ok();
                let service = service.clone();
                let tls = tls.clone();
                let running = running.clone();
                std::thread::spawn(move || {
                    let outcome = (|| -> Result<(), String> {
                        let conn = rustls::ServerConnection::new(tls).map_err(|_| "TLS failed")?;
                        let mut stream = rustls::StreamOwned::new(conn, socket);
                        let result = request(&mut stream).and_then(|req| {
                            service
                                .lock()
                                .map_err(|_| "service unavailable".to_string())?
                                .handle(req)
                        });
                        match result {
                            Ok(body) => response(&mut stream, 200, &body)?,
                            Err(error) => response(&mut stream, 400, &json!({"error":error}))?,
                        }
                        stream.conn.send_close_notify();
                        let _ = stream.flush();
                        Ok(())
                    })();
                    if outcome.is_err() {
                        eprintln!("research connection ended");
                    }
                    running.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(_) => return Err("listener failed".into()),
        }
    }
}
use std::io::Write;
fn main() {
    if let Err(error) = run() {
        eprintln!("signet service stopped: {error}");
        std::process::exit(1);
    }
}
