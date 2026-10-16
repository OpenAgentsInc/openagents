//! Explicit signet-only research; never selects a production wallet or account.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use lnd_sdk::{
    Client,
    invoices::SettlementSink,
    macaroon::{Macaroon, Role},
    service::LightningRail,
    transport::Config,
};
use nostr::x402::{self, NetworkPolicy, PaymentRequirements, SupportedProfiles};
use openagents_wallet::Proof;
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_SIGNATURE, PaymentPayload,
    PaymentRequired,
    payment_scheme::{self, Challenge, Terms},
    server::{self, Receiver, Request, Resource, Response},
    wire,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const NETWORKS: NetworkPolicy = NetworkPolicy::SignetResearch;
const URL: &str = "http://127.0.0.1:21861";
const FEE_CAP: u64 = 5000;

fn read<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn create(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = path.as_ref();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    fs::File::open(path.parent().ok_or("missing parent")?)?.sync_all()?;
    Ok(())
}
fn hash(text: &[u8]) -> String {
    hex::encode(Sha256::digest(text))
}
fn point(text: &str) -> Result<[u8; 32]> {
    Ok(hex::decode(text)?.try_into().map_err(|_| "invalid hash")?)
}
fn private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

async fn client(root: &Path, name: &str, port: u16, role: Role) -> Result<Client> {
    let node = root.join(name);
    let cert = rustls_pemfile::certs(&mut std::io::Cursor::new(fs::read(node.join("tls.cert"))?))
        .next()
        .ok_or("missing certificate")??;
    let config = Config::new(
        format!("https://127.0.0.1:{port}"),
        Sha256::digest(cert.as_ref()).into(),
    );
    let credential = node.join("research.macaroon");
    if !credential.exists() {
        let admin = Macaroon::from_file(
            Role::Ops,
            0,
            &node.join("data/chain/bitcoin/signet/admin.macaroon"),
        )?;
        let admin = Client::connect(&config, admin).await?;
        if admin.network() != "signet" {
            return Err("the research node must use signet".into());
        }
        admin
            .bake_macaroon(role, 11861)
            .await?
            .write_new(&credential)?;
    }
    let client = Client::connect(&config, Macaroon::from_file(role, 11861, &credential)?).await?;
    if client.network() != "signet" {
        return Err("the research node must use signet".into());
    }
    let info = client.info().await?;
    if !info.synced_to_chain {
        return Err("the research node is syncing".into());
    }
    Ok(client)
}

struct LndReceiver {
    client: Client,
    runtime: tokio::runtime::Handle,
}
impl Receiver for LndReceiver {
    fn pay_to(&self) -> String {
        self.client.node_id()
    }
    fn invoice(
        &self,
        amount: u64,
        request: [u8; 32],
        expiry: u32,
    ) -> std::result::Result<String, String> {
        self.runtime
            .block_on(async {
                let hints = self.client.private_route_hints().await?;
                if hints.is_empty() {
                    return Err(lnd_sdk::Error::NotSynced);
                }
                Ok(self
                    .client
                    .create_invoice_with_hints(amount, request, expiry, hints)
                    .await?
                    .bolt11)
            })
            .map_err(|e: lnd_sdk::Error| e.to_string())
    }
    fn received_msat(&self, hash: [u8; 32]) -> std::result::Result<Option<u64>, String> {
        self.runtime
            .block_on(self.client.received_msat(hash))
            .map_err(|e| e.to_string())
    }
}

fn resource(
    root: &Path,
    receiver: Arc<LndReceiver>,
    path: &str,
    amount: u64,
) -> Result<Resource<FileReplayStore>> {
    let executions = root.join("executions");
    private_dir(&executions)?;
    Ok(Resource {
        url: format!("{URL}{path}"),
        network: x402::SIGNET,
        amount_msat: amount,
        timeout_secs: 3600,
        description: "Signet research counter".into(),
        mime_type: "application/json".into(),
        receiver,
        executor: Arc::new(move |body: &[u8]| {
            let result = json!({"executed":true,"body_hash":hash(body)});
            create(executions.join(format!("{}.json", hash(body))), &result)
                .map_err(|_| "Could not save the research result.".to_string())?;
            serde_json::to_vec(&result)
                .map_err(|_| "Could not encode the research result.".to_string())
        }),
        facilitator: Facilitator::new(FileReplayStore::open(&root.join("replay"))?, 60)
            .with_network_policy(NETWORKS),
    })
}

fn handle(
    resource: &Resource<FileReplayStore>,
    root: &Path,
    key: &[u8],
    request: &Request,
) -> Response {
    let mut translated;
    let request = if let Some(authorization) = request.header("authorization") {
        let credential = match payment_scheme::parse_credential(authorization) {
            Ok(c) => c,
            Err(_) => return Response::json(402, &json!({"error":"Invalid payment."})),
        };
        let charge = match payment_scheme::verify_binding(
            key,
            &credential,
            &request.method,
            &resource.url,
            &request.body,
            openagents_x402::unix_now(),
        ) {
            Ok(c) => c,
            Err(_) => {
                return Response::json(
                    402,
                    &json!({"error":"Payment does not match this request."}),
                );
            }
        };
        // The HMAC is checked before the payment hash can select any file.
        if point(&charge.payment_hash).is_err() {
            return Response::json(402, &json!({"error":"Invalid payment hash."}));
        }
        let accepted: PaymentRequirements =
            match read(root.join(format!("issued-{}.json", charge.payment_hash))) {
                Ok(r) => r,
                Err(_) => return Response::json(402, &json!({"error":"Unknown payment."})),
            };
        if accepted.extra["invoice"].as_str() != Some(charge.invoice.as_str())
            || charge.amount_sats.checked_mul(1000) != Some(resource.amount_msat)
        {
            return Response::json(402, &json!({"error":"Payment amount does not match."}));
        }
        let payload = PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted,
            payload: credential.payload,
            extensions: None,
        };
        let signature = match wire::encode_header(&payload) {
            Ok(s) => s,
            Err(_) => return Response::json(402, &json!({"error":"Invalid payment."})),
        };
        translated = Request {
            method: request.method.clone(),
            target: request.target.clone(),
            body: request.body.clone(),
            headers: vec![(PAYMENT_SIGNATURE.into(), signature)],
        };
        &mut translated
    } else {
        request
    };
    let (mut response, _) = resource.handle(request, openagents_x402::unix_now());
    // The companion scheme prices whole sats; the exact-msat case stays x402.
    if resource.amount_msat.is_multiple_of(1000)
        && let Some((_, header)) = response.headers.iter().find(|(n, _)| n == PAYMENT_REQUIRED)
    {
        let required = match wire::decode_payment_required(header) {
            Ok(r) => r,
            Err(_) => {
                return Response::json(500, &json!({"error":"Could not read the challenge."}));
            }
        };
        let accepted = &required.accepts[0];
        let invoice = accepted.extra["invoice"].as_str().unwrap();
        let invoice_info = match x402::decode_invoice_with_network_policy(invoice, NETWORKS) {
            Ok(i) => i,
            Err(_) => return Response::json(500, &json!({"error":"Invalid research invoice."})),
        };
        let payment_hash = hex::encode(invoice_info.payment_hash());
        if create(root.join(format!("issued-{payment_hash}.json")), accepted).is_err() {
            return Response::json(500, &json!({"error":"Could not save the challenge."}));
        }
        let terms = Terms {
            realm: "signet research",
            amount_sats: resource.amount_msat / 1000,
            invoice,
            payment_hash: &payment_hash,
            network: x402::SIGNET,
            http_method: &request.method,
            url: &resource.url,
            body: &request.body,
            expires_at: invoice_info.created_at() + invoice_info.expiry_seconds(),
            description: None,
        };
        let challenge = Challenge::issue_with_network_policy(key, &terms, NETWORKS).unwrap();
        // Save the exact public challenge so retrying never issues a replacement.
        if create(
            root.join(format!("companion-{payment_hash}.json")),
            &challenge,
        )
        .is_err()
        {
            return Response::json(500, &json!({"error":"Could not save the challenge."}));
        }
        response
            .headers
            .push(("WWW-Authenticate".into(), challenge.header_value()));
    }
    response
}

struct Sink {
    path: PathBuf,
    position: u64,
    hashes: Vec<String>,
}
impl SettlementSink for Sink {
    fn position(&self) -> lnd_sdk::Result<u64> {
        Ok(self.position)
    }
    fn commit(&mut self, invoice: &lnd_sdk::proto::lnrpc::Invoice) -> lnd_sdk::Result<()> {
        let mut hashes = self.hashes.clone();
        hashes.push(hex::encode(&invoice.r_hash));
        create(
            self.path
                .join(format!("settlement-{}.json", invoice.settle_index)),
            &json!({"settle_index":invoice.settle_index,"payment_hashes":hashes}),
        )
        .map_err(|_| lnd_sdk::Error::Checkpoint)?;
        self.position = invoice.settle_index;
        self.hashes = hashes;
        Ok(())
    }
}

async fn run(root: &Path) -> Result<Value> {
    let payer = client(root, "a", 10039, Role::Payout).await?;
    let receiver = client(root, "b", 10049, Role::Invoice).await?;
    let identity =
        json!({"payer":payer.node_id(),"receiver":receiver.node_id(),"network":"signet"});
    let identities = root.join("research-identities.json");
    if identities.exists() {
        if read::<Value>(&identities)? != identity {
            return Err("Research node identity changed.".into());
        }
    } else {
        create(&identities, &identity)?;
    }
    let mut wrong_pin = Config::new("https://127.0.0.1:10039".into(), [0; 32]);
    wrong_pin.timeout = Duration::from_secs(3);
    let wrong = Client::connect(
        &wrong_pin,
        Macaroon::from_file(Role::Payout, 11861, &root.join("a/research.macaroon"))?,
    )
    .await;
    assert!(matches!(wrong, Err(lnd_sdk::Error::CertificatePin)));
    assert!(matches!(
        payer.create_invoice(1000, [0; 32], 60).await,
        Err(lnd_sdk::Error::PermissionDenied)
    ));
    let live_receiver = Arc::new(LndReceiver {
        client: receiver.clone(),
        runtime: tokio::runtime::Handle::current(),
    });
    let exact = Arc::new(resource(root, live_receiver.clone(), "/exact", 1_001_001)?);
    let companion = Arc::new(resource(root, live_receiver, "/companion", 2_000_000)?);
    let mcp = Arc::new(openagents_x402::mcp::PaidTools {
        server: format!("{URL}/mcp"),
        network: x402::SIGNET,
        amount_msat: 1_000_000,
        timeout_secs: 3600,
        description: "Signet research counter".into(),
        receiver: Arc::new(LndReceiver {
            client: receiver.clone(),
            runtime: tokio::runtime::Handle::current(),
        }),
        facilitator: Facilitator::with_profiles(
            FileReplayStore::open(&root.join("replay"))?,
            60,
            openagents_x402::facilitator::MCP_ONLY,
        )
        .with_network_policy(NETWORKS),
    });
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let server_root = root.to_owned();
    let listener = TcpListener::bind("127.0.0.1:21861")?;
    let key_path = root.join("companion-key.json");
    if !key_path.exists() {
        let mut key = [0u8; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut key)?;
        create(&key_path, &hex::encode(key))?;
    }
    let key = point(&read::<String>(&key_path)?)?;
    let thread = std::thread::spawn(move || {
        server::serve_with(listener, stopped, move |request| {
            if request.target == "/mcp" {
                return handle_mcp(&mcp, &server_root, request);
            }
            let resource = if request.target == "/exact" {
                &exact
            } else {
                &companion
            };
            handle(resource, &server_root, &key, request)
        })
    });
    let result = cases(root, &payer, &receiver).await;
    stop.store(true, Ordering::SeqCst);
    let _ = thread.join();
    result
}

async fn original_proof(
    directory: &Path,
    payer: &Client,
    invoice: &x402::Invoice,
    invoice_text: &str,
    amount: u64,
) -> Result<Proof> {
    let payment_hash = hex::encode(invoice.payment_hash());
    let proof_path = directory.join("proof.json");
    let proof: Proof = if proof_path.exists() {
        read(&proof_path)?
    } else if directory.join("send-started.json").exists() {
        // A lost send response is recovered solely by its original hash.
        let payment = payer.track(invoice.payment_hash()).await?;
        if payment.status != lnd_sdk::proto::lnrpc::payment::PaymentStatus::Succeeded as i32 {
            return Err(
                "Original research payment is unresolved; no second send is allowed.".into(),
            );
        }
        let proof = Proof {
            payment_hash: payment.payment_hash,
            preimage: payment.payment_preimage,
            amount_msat: u64::try_from(payment.value_msat)?,
            fee_msat: u64::try_from(payment.fee_msat)?,
            bolt11: invoice_text.into(),
        };
        create(&proof_path, &proof)?;
        proof
    } else {
        create(
            directory.join("send-started.json"),
            &json!({"payment_hash":payment_hash,"amount_msat":amount,"fee_cap_msat":FEE_CAP}),
        )?;
        let proof = payer
            .send_payment(invoice_text, FEE_CAP, Duration::from_secs(60))
            .await?;
        create(&proof_path, &proof)?;
        proof
    };
    Ok(proof)
}

async fn cases(root: &Path, payer: &Client, receiver: &Client) -> Result<Value> {
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?;
    let mut results = Vec::new();
    for (name, amount) in [("exact", 1_001_001u64), ("companion", 2_000_000)] {
        let directory = root.join(format!("case-{name}"));
        private_dir(&directory)?;
        let url = format!("{URL}/{name}");
        let body = format!("signet counter {name}");
        let required_path = directory.join("challenge.json");
        let required: PaymentRequired = if required_path.exists() {
            read(&required_path)?
        } else {
            let response = http.post(&url).body(body.clone()).send().await?;
            if response.status() != 402 {
                return Err("The research service did not challenge.".into());
            }
            let required = wire::decode_payment_required(
                response
                    .headers()
                    .get(PAYMENT_REQUIRED)
                    .ok_or("Missing challenge")?
                    .to_str()?,
            )?;
            create(&required_path, &required)?;
            required
        };
        let accepted = required.accepts.first().ok_or("Missing terms")?;
        let binding = x402::http_binding("POST", &url, body.as_bytes(), &[])
            .map_err(|_| "Invalid binding")?;
        let request_hash = x402::binding_hash(&binding).map_err(|_| "Invalid hash")?;
        let now = openagents_x402::unix_now();
        let invoice = x402::validate_challenge_with_network_policy(
            accepted,
            &request_hash,
            now,
            60,
            SupportedProfiles::default(),
            NETWORKS,
        )
        .map_err(|_| "Research challenge refused")?;
        assert_eq!(invoice.amount_msat(), amount);
        assert_eq!(hex::encode(invoice.payee()), receiver.node_id());
        assert!(
            x402::validate_challenge(
                accepted,
                &request_hash,
                now,
                60,
                SupportedProfiles::default()
            )
            .is_err()
        );
        let invoice_text = accepted.extra["invoice"]
            .as_str()
            .ok_or("Missing invoice")?;
        let parsed = lnd_sdk::router::invoice(invoice_text)?;
        assert!(!parsed.route_hints().is_empty());
        let payment_hash = hex::encode(invoice.payment_hash());
        let proof = original_proof(&directory, payer, &invoice, invoice_text, amount).await?;
        if proof.payment_hash != payment_hash
            || proof.amount_msat != amount
            || proof.fee_msat > FEE_CAP
            || proof.bolt11 != invoice_text
        {
            return Err("Research payment terms changed.".into());
        }
        x402::validate_paid_proof_with_network_policy(
            accepted,
            accepted,
            &proof.preimage,
            openagents_x402::unix_now(),
            60,
            SupportedProfiles::default(),
            NETWORKS,
        )
        .map_err(|_| "Original proof refused")?;
        assert_eq!(
            receiver.received_msat(invoice.payment_hash()).await?,
            Some(amount)
        );
        assert!(matches!(
            receiver
                .send_payment(invoice_text, FEE_CAP, Duration::from_secs(5))
                .await,
            Err(lnd_sdk::Error::PermissionDenied)
        ));
        let tracked = payer.track(invoice.payment_hash()).await?;
        assert_eq!(tracked.payment_hash, payment_hash);
        assert_eq!(tracked.value_msat, i64::try_from(amount)?);
        let payload = PaymentPayload {
            x402_version: 2,
            resource: Some(required.resource.clone()),
            accepted: accepted.clone(),
            payload: serde_json::Map::from_iter([("preimage".into(), json!(proof.preimage))]),
            extensions: None,
        };
        let signature = wire::encode_header(&payload)?;
        let execution = root
            .join("executions")
            .join(format!("{}.json", hash(body.as_bytes())));
        if !execution.exists() {
            for mutation in ["body", "preimage", "marker"] {
                let mut wrong = payload.clone();
                let mut request_body = body.clone();
                match mutation {
                    "body" => request_body.push('!'),
                    "preimage" => {
                        wrong
                            .payload
                            .insert("preimage".into(), json!("00".repeat(32)));
                    }
                    _ => {
                        wrong.accepted.extra.remove("researchProfile");
                    }
                }
                assert_eq!(
                    http.post(&url)
                        .body(request_body)
                        .header(PAYMENT_SIGNATURE, wire::encode_header(&wrong)?)
                        .send()
                        .await?
                        .status(),
                    402
                );
            }
            // Send the paid request and deliberately discard its response.
            // The execution file and replay store resolve the original result.
            let (header, value) = if name == "companion" {
                let challenge: Challenge =
                    read(root.join(format!("companion-{payment_hash}.json")))?;
                let credential = json!({"challenge":challenge,"payload":payload.payload});
                (
                    "Authorization",
                    format!(
                        "Payment {}",
                        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&credential)?)
                    ),
                )
            } else {
                (PAYMENT_SIGNATURE, signature.clone())
            };
            create(
                directory.join("execution-started.json"),
                &json!({"payment_hash":payment_hash}),
            )?;
            let mut socket = TcpStream::connect("127.0.0.1:21861")?;
            socket.set_write_timeout(Some(Duration::from_secs(10)))?;
            write!(
                socket,
                "POST /{name} HTTP/1.1\r\nHost: 127.0.0.1:21861\r\n{header}: {value}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            socket.shutdown(std::net::Shutdown::Write)?;
            drop(socket);
            for _ in 0..100 {
                if execution.exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if !execution.exists() {
                return Err("Research execution is unresolved; inspect the original claim.".into());
            }
        }
        let duplicate = http
            .post(&url)
            .body(body.clone())
            .header(PAYMENT_SIGNATURE, &signature)
            .send()
            .await?;
        assert_eq!(duplicate.status(), 402);
        let response = wire::decode_header::<wire::SettlementResponse>(
            duplicate
                .headers()
                .get(wire::PAYMENT_RESPONSE)
                .ok_or("Missing settlement result")?
                .to_str()?,
        )?;
        assert_eq!(
            response.error_reason.as_deref(),
            Some("duplicate_settlement")
        );
        let restarted = Facilitator::new(FileReplayStore::open(&root.join("replay"))?, 60)
            .with_network_policy(NETWORKS);
        assert_eq!(
            restarted
                .settle(
                    accepted,
                    &payload,
                    &request_hash,
                    openagents_x402::unix_now()
                )
                .unwrap_err()
                .error_reason
                .as_deref(),
            Some("duplicate_settlement")
        );
        results.push(json!({"case":name,"payment_hash":payment_hash,"amount_msat":amount,"fee_msat":proof.fee_msat,
            "private_route_hints":parsed.route_hints().len(),"description_hash":request_hash,"received_msat":amount,
            "lost_response_execution_saved":true,"replay_after_response_loss":"duplicate_settlement","execution_count":1}));
    }
    results.push(mcp_case(root, payer, receiver, &http).await?);
    let mut sink = Sink {
        path: root.to_owned(),
        position: 0,
        hashes: Vec::new(),
    };
    // Replay the node's actual settled history without retaining preimages.
    // The second run uses existing receipts as its cursor.
    let mut prior = Vec::new();
    for entry in fs::read_dir(root)? {
        let p = entry?.path();
        if p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("settlement-") && n.ends_with(".json"))
        {
            prior.push(read::<Value>(p)?);
        }
    }
    prior.sort_by_key(|v| v["settle_index"].as_u64().unwrap_or(0));
    if let Some(last) = prior.last() {
        sink.position = last["settle_index"]
            .as_u64()
            .ok_or("Invalid settlement cursor")?;
        sink.hashes = serde_json::from_value(last["payment_hashes"].clone())?;
    }
    if sink.hashes.len() < results.len() {
        let missing = results.len() - sink.hashes.len();
        receiver.settlements(&mut sink, missing).await?;
    }
    let mut expected: Vec<_> = results
        .iter()
        .map(|r| r["payment_hash"].as_str().unwrap().to_owned())
        .collect();
    expected.sort();
    let stream_path = root.join("stream-check");
    private_dir(&stream_path)?;
    let stream_receipt = stream_path.join("public-result.json");
    if !stream_receipt.exists() {
        let mut invoices = Vec::new();
        for hash in &expected {
            invoices.push(receiver.lookup_invoice(point(hash)?).await?);
        }
        invoices.sort_by_key(|invoice| invoice.settle_index);
        let mut stream_sink = Sink {
            path: stream_path.clone(),
            position: 0,
            hashes: Vec::new(),
        };
        // Seed the consumer from one exact LookupInvoice result. A nonzero
        // cursor forces SubscribeInvoices to replay the next two settlements.
        stream_sink.commit(&invoices[0])?;
        tokio::time::timeout(
            Duration::from_secs(30),
            receiver.settlements(&mut stream_sink, invoices.len() - 1),
        )
        .await??;
        let mut streamed = stream_sink.hashes.clone();
        streamed.sort();
        assert_eq!(streamed, expected);
        create(
            &stream_receipt,
            &json!({"subscribe_from_settle_index":invoices[0].settle_index,
            "replayed_settlements":invoices.len()-1,"payment_hashes":streamed}),
        )?;
    }
    let mut actual = sink.hashes.clone();
    actual.sort();
    assert_eq!(actual, expected);
    Ok(
        json!({"network":"signet","nodes":{"payer":payer.node_id(),"receiver":receiver.node_id()},
        "checks":{"wrong_tls_pin_refused":true,"payout_cannot_issue_invoice":true,"invoice_cannot_send":true,
        "standard_profile_refused_signet":true,"body_proof_marker_substitutions_refused":true,"settlement_stream_hashes":actual},"payments":results}),
    )
}

#[tokio::main]
async fn main() {
    let result = async {
        let root = std::env::args()
            .nth(1)
            .ok_or("Pass the dedicated lnd-signet-11861 directory")?;
        let root = Path::new(&root).canonicalize()?;
        if root.file_name().and_then(|s| s.to_str()) != Some("lnd-signet-11861") {
            return Err("Refusing a non-research wallet directory.".into());
        }
        let result = run(&root).await?;
        let output = root.join("public-result.json");
        if !output.exists() {
            create(output, &result)?;
        }
        println!("{}", result);
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    if let Err(error) = result {
        eprintln!("Research check stopped: {error}");
        std::process::exit(1);
    }
}

fn handle_mcp(
    tool: &openagents_x402::mcp::PaidTools<FileReplayStore>,
    root: &Path,
    request: &Request,
) -> Response {
    use openagents_x402::mcp::{self, Gate};
    let wire = match nostr::contracts::parse_strict(&request.body) {
        Ok(v) => v,
        Err(_) => return Response::json(400, &json!({"error":"Invalid tool request."})),
    };
    if wire["method"] != "tools/call" {
        return Response::json(400, &json!({"error":"Unknown research method."}));
    }
    let result = match tool.gate(&wire["params"], openagents_x402::unix_now()) {
        Ok(Gate::Challenge(result) | Gate::Refused(result)) => result,
        Ok(Gate::Admitted {
            request_hash,
            payment_hash,
            settlement,
        }) => {
            let result =
                json!({"content":[{"type":"text","text":"Counter recorded."}],"isError":false});
            if create(
                root.join("executions")
                    .join(format!("mcp-{request_hash}.json")),
                &json!({"payment_hash":payment_hash,"request_hash":request_hash}),
            )
            .is_err()
            {
                return Response::json(
                    500,
                    &json!({"error":"Could not save the research result."}),
                );
            }
            mcp::with_settlement(result, settlement)
        }
        Err(_) => {
            return Response::json(400, &json!({"error":"Could not check the tool request."}));
        }
    };
    Response::json(
        200,
        &json!({"jsonrpc":"2.0","id":wire["id"],"result":result}),
    )
}

async fn mcp_case(
    root: &Path,
    payer: &Client,
    receiver: &Client,
    http: &reqwest::Client,
) -> Result<Value> {
    use openagents_x402::{facilitator::MCP_ONLY, mcp};
    let directory = root.join("case-mcp");
    private_dir(&directory)?;
    let url = format!("{URL}/mcp");
    let params = json!({"name":"counter","arguments":{"label":"signet research"}});
    let message =
        |params: Value| json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":params});
    let path = directory.join("challenge.json");
    let required: PaymentRequired = if path.exists() {
        read(&path)?
    } else {
        let response: Value = http
            .post(&url)
            .json(&message(params.clone()))
            .send()
            .await?
            .json()
            .await?;
        let required = mcp::payment_required_from_result(&response["result"])
            .ok_or("Missing MCP challenge")?;
        create(path, &required)?;
        required
    };
    let accepted = required.accepts.first().ok_or("Missing MCP terms")?;
    let binding = x402::mcp_binding(&url, &params, &[]).map_err(|_| "Invalid MCP binding")?;
    let request_hash = x402::binding_hash(&binding).map_err(|_| "Invalid MCP hash")?;
    let invoice = x402::validate_challenge_with_network_policy(
        accepted,
        &request_hash,
        openagents_x402::unix_now(),
        60,
        MCP_ONLY,
        NETWORKS,
    )
    .map_err(|_| "MCP challenge refused")?;
    assert_eq!(invoice.amount_msat(), 1_000_000);
    assert_eq!(hex::encode(invoice.payee()), receiver.node_id());
    let invoice_text = accepted.extra["invoice"]
        .as_str()
        .ok_or("Missing MCP invoice")?;
    let proof = original_proof(&directory, payer, &invoice, invoice_text, 1_000_000).await?;
    if proof.payment_hash != hex::encode(invoice.payment_hash())
        || proof.amount_msat != 1_000_000
        || proof.fee_msat > FEE_CAP
        || proof.bolt11 != invoice_text
    {
        return Err("MCP payment terms changed.".into());
    }
    x402::validate_paid_proof_with_network_policy(
        accepted,
        accepted,
        &proof.preimage,
        openagents_x402::unix_now(),
        60,
        MCP_ONLY,
        NETWORKS,
    )
    .map_err(|_| "MCP proof refused")?;
    assert_eq!(
        receiver.received_msat(invoice.payment_hash()).await?,
        Some(1_000_000)
    );
    let payload = PaymentPayload {
        x402_version: 2,
        resource: Some(required.resource.clone()),
        accepted: accepted.clone(),
        payload: serde_json::Map::from_iter([("preimage".into(), json!(proof.preimage))]),
        extensions: None,
    };
    let execution = root
        .join("executions")
        .join(format!("mcp-{request_hash}.json"));
    let paid = message(mcp::with_payment(params.clone(), &payload)?);
    if !execution.exists() {
        let wrong = mcp::with_payment(
            json!({"name":"counter","arguments":{"label":"changed"}}),
            &payload,
        )?;
        let refused: Value = http
            .post(&url)
            .json(&message(wrong))
            .send()
            .await?
            .json()
            .await?;
        assert_eq!(refused["result"]["isError"], true);
        create(
            directory.join("execution-started.json"),
            &json!({"payment_hash":proof.payment_hash}),
        )?;
        let body = serde_json::to_vec(&paid)?;
        let mut socket = TcpStream::connect("127.0.0.1:21861")?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        write!(
            socket,
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:21861\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
        socket.write_all(&body)?;
        socket.shutdown(std::net::Shutdown::Write)?;
        drop(socket);
        for _ in 0..100 {
            if execution.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !execution.exists() {
            return Err("Original MCP execution is unresolved; inspect the saved claim.".into());
        }
    }
    let duplicate: Value = http.post(&url).json(&paid).send().await?.json().await?;
    assert_eq!(
        duplicate["result"]["_meta"][mcp::PAYMENT_RESPONSE_META]["errorReason"],
        "duplicate_settlement"
    );
    Ok(
        json!({"case":"mcp","payment_hash":proof.payment_hash,"amount_msat":proof.amount_msat,
        "fee_msat":proof.fee_msat,"description_hash":request_hash,"received_msat":1_000_000,
        "changed_arguments_refused":true,"lost_response_execution_saved":true,"replay_after_response_loss":"duplicate_settlement","execution_count":1}),
    )
}
