//! Publication, exact paid execution, and withdrawal for supplied meeting notes.

use super::*;
use crate::plugin_registry::{Fee, Registry};
use nostr::domain::{Event, RelaySigner, Tag};

#[derive(Default)]
struct Relay(Vec<Event>);
impl Registry for Relay {
    fn query(&mut self, filter: Value) -> Result<Vec<Event>, String> {
        Ok(self
            .0
            .iter()
            .filter(|event| {
                ["ids", "authors"].iter().all(|key| {
                    let value = if *key == "ids" {
                        &event.id
                    } else {
                        &event.pubkey
                    };
                    filter[*key]
                        .as_array()
                        .is_none_or(|items| items.iter().any(|item| item == value))
                }) && filter["kinds"]
                    .as_array()
                    .is_none_or(|items| items.iter().any(|item| item == event.kind))
                    && ["d", "t", "e"].iter().all(|tag| {
                        filter[format!("#{tag}")].as_array().is_none_or(|items| {
                            event
                                .tag_values(tag)
                                .any(|value| items.iter().any(|item| item == value))
                        })
                    })
            })
            .cloned()
            .collect())
    }
    fn send(&mut self, event: Event) -> Result<(), String> {
        event.validate_crypto().map_err(|e| e.to_string())?;
        self.0.push(event);
        Ok(())
    }
}

#[derive(Default)]
struct MemoryBlobs(Mutex<BTreeMap<String, Vec<u8>>>);
impl Blobs for MemoryBlobs {
    fn put(&self, bytes: &[u8], _: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .insert(nostr::contracts::digest_bytes(bytes), bytes.to_vec());
        Ok(())
    }
    fn get(&self, digest: &str) -> Result<Vec<u8>, String> {
        self.0
            .lock()
            .unwrap()
            .get(digest)
            .cloned()
            .ok_or_else(|| "synthetic blob unavailable".into())
    }
    fn locator(&self) -> Option<String> {
        Some("https://blobs.example.invalid".into())
    }
}

struct Source {
    relay: Mutex<Relay>,
    blobs: MemoryBlobs,
    cache: PathBuf,
}
impl PluginSource for Source {
    fn resolve(&self, id: &str) -> Result<Arc<Resolved>, Unpriced> {
        let mut relay = self.relay.lock().unwrap();
        let listing = plugin_registry::find(&mut *relay, id)
            .map_err(|e| refused(404, "plugin_not_found", e))?;
        Ok(Arc::new(resolve_listing(
            &mut *relay,
            &listing,
            &[&self.blobs],
            &self.cache,
        )?))
    }
}

fn fixture_package(into: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/meeting-action-items");
    std::fs::create_dir_all(into.join("programs")).unwrap();
    std::fs::create_dir_all(into.join("examples")).unwrap();
    for name in [
        "package.json",
        "README.md",
        "LICENSE",
        "build-receipt.json",
        "programs/meeting-action-items.json",
        "examples/meeting.md",
    ] {
        std::fs::copy(source.join(name), into.join(name)).unwrap();
    }
}

pub(crate) fn signed_source(root: &Path) -> (Arc<dyn PluginSource>, String) {
    let (source, id, _, _) = served_source(root);
    (source, id)
}

pub(crate) fn served_source(
    root: &Path,
) -> (
    Arc<dyn PluginSource>,
    String,
    Vec<Event>,
    BTreeMap<String, Vec<u8>>,
) {
    let dir = root.join("package");
    fixture_package(&dir);
    let signer = RelaySigner::from_secret_hex(&"03".repeat(32)).unwrap();
    let source = Arc::new(Source {
        relay: Mutex::new(Relay::default()),
        blobs: MemoryBlobs::default(),
        cache: root.join("cache"),
    });
    let packed = plugin_registry::pack(&dir, signer.pubkey()).unwrap();
    plugin_registry::publish(
        &packed,
        &signer,
        &mut *source.relay.lock().unwrap(),
        &source.blobs,
        Some(&Fee {
            msat: 1000,
            payout: to_hex(payee_of([17; 32])),
        }),
        NOW,
    )
    .unwrap();
    let records = source.relay.lock().unwrap().0.clone();
    let blobs = source.blobs.0.lock().unwrap().clone();
    (source, packed.package, records, blobs)
}

fn marker(kind: &str) -> Tag {
    Tag::new(vec!["t".into(), format!("oa:ext:{kind}:v1")])
}

#[test]
fn signed_useful_release_quotes_runs_and_accrues_the_full_author_fee() {
    let work = tempfile::tempdir().unwrap();
    let dir = work.path().join("package");
    fixture_package(&dir);
    let signer = RelaySigner::from_secret_hex(&"03".repeat(32)).unwrap();
    let fee = Fee {
        msat: 1_000,
        payout: to_hex(payee_of([17; 32])),
    };
    let source = Arc::new(Source {
        relay: Mutex::new(Relay::default()),
        blobs: MemoryBlobs::default(),
        cache: work.path().join("cache"),
    });
    let packed = plugin_registry::pack(&dir, signer.pubkey()).unwrap();
    let published = plugin_registry::publish(
        &packed,
        &signer,
        &mut *source.relay.lock().unwrap(),
        &source.blobs,
        Some(&fee),
        NOW,
    )
    .unwrap();
    let release = nostr::ext::parse_record(&published.release).unwrap();
    assert_eq!(release["fee_msat"], fee.msat);
    assert_eq!(release["payout"], fee.payout);
    let destination = pay_ledger::payee::resolve(&pay_ledger::payee::Sources {
        pubkey: Some(signer.pubkey().to_owned()),
        release: Some(published.release.clone()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(destination.value, fee.payout);
    assert_eq!(destination.source, pay_ledger::payee::Source::Release);
    let resolved = source.resolve(&packed.package).unwrap();
    assert_eq!(resolved.release, published.release.id);
    assert_eq!(resolved.packet.operation, "items");
    assert_eq!(resolved.packet.request_key.as_deref(), Some("text"));
    let notes = include_str!("../../../../../plugins/meeting-action-items/examples/meeting.md");
    let receiver = Arc::new(FakeReceiver {
        counter: AtomicU64::new(0),
        preimages: Mutex::new(HashMap::new()),
    });
    let sink = Arc::new(LedgerSink::in_memory());
    let front = front_with(work.path(), receiver.clone(), sink.clone(), source.clone());
    let target = format!("/v1/plugins/{}/invoke", packed.package);
    let supplied = approved(&front, &target, notes);
    assert_eq!(receiver.counter.load(Ordering::SeqCst), 0);
    let (challenge, _) = front.handle(&post(&target, &supplied, vec![]), NOW);
    assert_eq!(challenge.status, 402);
    let quote = json_body(&challenge);
    assert_eq!(quote["price_msat"], 6_000);
    assert_eq!(
        quote["price_parts"],
        json!([{"name":"endpoint","msat":5_000},{"name":"author_fee","msat":1_000}])
    );
    assert_eq!(quote["release"], published.release.id);
    let required = decode_payment_required(header(&challenge, PAYMENT_REQUIRED).unwrap()).unwrap();
    let accepted = required.accepts[0].clone();
    let invoice = accepted.extra["invoice"].as_str().unwrap();
    let mut proof = Map::new();
    proof.insert("preimage".into(), json!(receiver.pay(invoice)));
    let signature = openagents_x402::wire::encode_header(&PaymentPayload {
        x402_version: 2,
        resource: None,
        accepted,
        payload: proof,
        extensions: None,
    })
    .unwrap();
    let request = post(
        &target,
        &supplied,
        vec![(PAYMENT_SIGNATURE.into(), signature.clone())],
    );
    let (paid, _) = front.handle(&request, NOW);
    assert_eq!(paid.status, 200, "{}", String::from_utf8_lossy(&paid.body));
    let output = json_body(&paid);
    let value = &output["value"];
    assert_eq!(output["release"], published.release.id);
    assert_eq!(value["items"].as_array().unwrap().len(), 3);
    assert_eq!(value["items"][0]["owner"], "Ana");
    assert_eq!(value["items"][0]["task"], "send the budget");
    assert_eq!(value["items"][0]["due"], "Friday");
    assert_eq!(
        value["items"][0]["source"],
        json!({"from":"request","line":2})
    );
    assert_eq!(value["items"][1]["owner"], "Ben");
    assert_eq!(value["items"][1]["task"], "review the launch checklist");
    assert_eq!(value["items"][1]["due"], "Tuesday");
    assert_eq!(
        value["items"][1]["source"],
        json!({"from":"request","line":3})
    );
    assert_eq!(value["items"][2]["owner"], Value::Null);
    assert_eq!(value["items"][2]["task"], "prepare the migration notes");
    assert_eq!(
        value["items"][2]["source"],
        json!({"from":"request","line":4})
    );
    assert_eq!(value["unassigned"], 1);
    assert_eq!(value["done_left_out"], 1);
    assert_eq!(value["truncated"], false);
    assert_eq!(value["read"], json!([]));
    let (again, _) = front.handle(&request, NOW);
    assert_eq!(again.status, 402);
    assert_eq!(json_body(&again)["error"]["code"], "duplicate_settlement");
    sink.with(|ledger| {
        let rows = ledger.since(0).unwrap();
        assert_eq!(rows.len(), 1);
        let share = rows[0].shares.iter().find(|s| s.role == "author").unwrap();
        assert_eq!(share.party, signer.pubkey());
        assert_eq!(share.amount_msat, fee.msat as i64);
        assert_eq!(
            rows[0].release_id.as_deref(),
            Some(published.release.id.as_str())
        );
    });
    // A digest cache is a byte optimization, never authority to skip checks.
    let program_bytes = &packed
        .files
        .iter()
        .find(|f| f.path == "programs/meeting-action-items.json")
        .unwrap()
        .bytes;
    let digest = nostr::contracts::digest_bytes(program_bytes);
    let cached_program = source
        .cache
        .join("blobs")
        .join(digest.strip_prefix("sha256:").unwrap());
    std::fs::write(&cached_program, b"{}").unwrap();
    source
        .blobs
        .0
        .lock()
        .unwrap()
        .insert(digest.clone(), b"{}".to_vec());
    let invoices = receiver.counter.load(Ordering::SeqCst);
    let (tampered, _) = front.handle(&post(&target, notes, vec![]), NOW);
    assert_eq!(tampered.status, 422);
    assert_eq!(receiver.counter.load(Ordering::SeqCst), invoices);
    source
        .blobs
        .0
        .lock()
        .unwrap()
        .insert(digest, program_bytes.clone());
    assert_eq!(
        source.resolve(&packed.package).unwrap().release,
        published.release.id
    );
    {
        let mut relay = source.relay.lock().unwrap();
        relay
            .0
            .iter_mut()
            .find(|e| e.id == published.release.id)
            .unwrap()
            .sig = "01".repeat(64);
    }
    let (forged, _) = front.handle(&post(&target, notes, vec![]), NOW);
    assert_eq!(forged.status, 422);
    assert_eq!(receiver.counter.load(Ordering::SeqCst), invoices);
    {
        let mut relay = source.relay.lock().unwrap();
        *relay
            .0
            .iter_mut()
            .find(|e| e.id == published.release.id)
            .unwrap() = published.release.clone();
    }
    let receipt = json!({
        "schema":"openagents.paid-plugin-acceptance.v1","synthetic":true,"deployed_available":false,
        "customer_qualified":false,"funded_payment_qualified":false,"author_payout_qualified":false,
        "publisher_buyer_independence_qualified":false,
        "publisher":signer.pubkey(),"package":packed.package,"version":packed.version,
        "package_record_digest":nostr::contracts::digest_bytes(&packed.files.iter().find(|f| f.path=="package.json").unwrap().bytes),
        "manifest_digest":nostr::contracts::digest_bytes(&packed.manifest),
        "program_digest":nostr::contracts::digest_bytes(&packed.files.iter().find(|f| f.path=="programs/meeting-action-items.json").unwrap().bytes),
        "wasm_digest":nostr::contracts::digest_bytes(&resolved.packet.wasm),
        "release":published.release,"listing":published.listing,"declared_author_fee_msat":fee.msat,
        "supported_payout":fee.payout,"endpoint_charge_msat":5_000,"total_quote_msat":6_000,
        "author_share_msat":1_000,"checks":{"signed_publication":true,"all_blob_digests":true,
            "exact_release_execution":true,"independent_expected_rows":true,"empty_snapshot":true,"duplicate_settlement":true,
            "tampered_cache_refused":true,"forged_release_refused":true,"signed_payout_resolution":true,
            "quote_approval_bound":true,"same_price_head_requires_new_approval":true,"revocation_refused":true,"withdrawal_refused":true},
        "output":output,"comparative_performance":"not claimed","costs":"fake fee decomposition; host CPU, storage, and energy unmetered"
    });

    let changed_fee = Fee {
        msat: 500,
        payout: fee.payout.clone(),
    };
    assert!(
        plugin_registry::publish(
            &packed,
            &signer,
            &mut *source.relay.lock().unwrap(),
            &source.blobs,
            Some(&changed_fee),
            NOW + 1
        )
        .unwrap_err()
        .contains("raise the version")
    );
    let mut package: Value =
        serde_json::from_slice(&std::fs::read(dir.join("package.json")).unwrap()).unwrap();
    package["version"] = json!("0.1.1");
    std::fs::write(
        dir.join("package.json"),
        serde_json::to_vec_pretty(&package).unwrap(),
    )
    .unwrap();
    let versioned = plugin_registry::pack(&dir, signer.pubkey()).unwrap();
    let next = plugin_registry::publish(
        &versioned,
        &signer,
        &mut *source.relay.lock().unwrap(),
        &source.blobs,
        Some(&fee),
        NOW + 2,
    )
    .unwrap();
    assert_ne!(next.release.id, published.release.id);
    assert_eq!(
        source.resolve(&packed.package).unwrap().release,
        next.release.id
    );
    // A same-price head change invalidates old approval and its paid proof.
    // The existing payment must never execute or accrue for a different release.
    let (changed, event) = front.handle(&post(&target, &supplied, vec![]), NOW + 2);
    assert_eq!(changed.status, 409);
    assert_eq!(event.outcome, "unpriced");
    assert!(header(&changed, PAYMENT_REQUIRED).is_none());
    assert_eq!(receiver.counter.load(Ordering::SeqCst), invoices);
    assert_eq!(json_body(&changed)["quote"]["release"], next.release.id);
    let (changed_paid, _) = front.handle(&request, NOW + 2);
    assert_eq!(changed_paid.status, 409);
    assert_eq!(receiver.counter.load(Ordering::SeqCst), invoices);
    sink.with(|ledger| assert_eq!(ledger.since(0).unwrap().len(), 1));
    let newly_approved =
        json!({"quote_digest":json_body(&changed)["quote_digest"],"request":notes}).to_string();
    let (rebound_proof, _) = front.handle(
        &post(
            &target,
            &newly_approved,
            vec![(PAYMENT_SIGNATURE.into(), signature)],
        ),
        NOW + 2,
    );
    assert_eq!(rebound_proof.status, 402);
    assert_eq!(
        json_body(&rebound_proof)["error"]["type"],
        "payment_refused"
    );
    sink.with(|ledger| assert_eq!(ledger.since(0).unwrap().len(), 1));
    let invoices = receiver.counter.load(Ordering::SeqCst);
    let revoked = signer.sign(NOW+3, nostr::ext::REVOCATION_KIND,
        vec![marker("revocation"), Tag::new(vec!["e".into(), next.release.id.clone()])],
        json!({"v":1,"requires":[],"type":"revocation","package":packed.package,"release":{"id":next.release.id,"pubkey":signer.pubkey(),"kind":nostr::ext::RELEASE_KIND},"reason":"synthetic withdrawal","effective_at":NOW+3}).to_string());
    source.relay.lock().unwrap().send(revoked).unwrap();
    let (unavailable, _) = front.handle(&post(&target, notes, vec![]), NOW + 3);
    assert_eq!(unavailable.status, 422);
    assert_eq!(receiver.counter.load(Ordering::SeqCst), invoices);
    assert!(
        source
            .resolve(&packed.package)
            .err()
            .unwrap()
            .message
            .contains("revoked")
    );
    let mut withdrawal: Value = serde_json::from_str(&next.listing.content).unwrap();
    withdrawal["state"] = json!("withdrawn");
    let event = signer.sign(
        NOW + 4,
        nostr::ext::LISTING_KIND,
        vec![
            marker("listing"),
            Tag::new(vec!["d".into(), packed.slug.clone()]),
        ],
        withdrawal.to_string(),
    );
    source.relay.lock().unwrap().send(event).unwrap();
    assert_eq!(source.resolve(&packed.package).err().unwrap().status, 404);
    let retained = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/plugins/meeting-action-items/receipt.json");
    if std::env::var_os("PAID_PLUGIN_RECEIPT_WRITE").is_some() {
        std::fs::create_dir_all(retained.parent().unwrap()).unwrap();
        std::fs::write(&retained, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    } else {
        let existing: Value = serde_json::from_slice(
            &std::fs::read(retained).expect("regenerate with PAID_PLUGIN_RECEIPT_WRITE=1"),
        )
        .unwrap();
        for field in [
            "manifest_digest",
            "program_digest",
            "wasm_digest",
            "release",
            "declared_author_fee_msat",
            "supported_payout",
        ] {
            assert_eq!(
                existing[field], receipt[field],
                "stale fixture field {field}; regenerate with PAID_PLUGIN_RECEIPT_WRITE=1"
            );
        }
    }
}

#[test]
fn unsupported_published_packets_never_offer_payment() {
    for case in ["multistep", "capability", "captured", "skills-only"] {
        let work = tempfile::tempdir().unwrap();
        let dir = work.path().join("package");
        fixture_package(&dir);
        let program_path = dir.join("programs/meeting-action-items.json");
        let mut program: Value =
            serde_json::from_slice(&std::fs::read(&program_path).unwrap()).unwrap();
        let package_path = dir.join("package.json");
        let mut package: Value =
            serde_json::from_slice(&std::fs::read(&package_path).unwrap()).unwrap();
        match case {
            "multistep" => {
                let mut next = program["definition"]["steps"][0].clone();
                next["name"] = json!("second");
                next["after"] = json!(["action_items"]);
                program["definition"]["steps"]
                    .as_array_mut()
                    .unwrap()
                    .push(next);
                program["binding"]["steps"]["second"] =
                    program["binding"]["steps"]["action_items"].clone();
            }
            "capability" => program["definition"]["requires"] = json!(["host-access"]),
            "captured" => {
                program["binding"]["steps"]["action_items"]["bounds"]["captured_input"] =
                    json!(true)
            }
            "skills-only" => {
                package.as_object_mut().unwrap().remove("program");
                std::fs::create_dir_all(dir.join("skills")).unwrap();
                std::fs::write(dir.join("skills/review.md"), "Review supplied notes.").unwrap();
            }
            _ => unreachable!(),
        }
        let text = serde_json::to_string_pretty(&program).unwrap();
        std::fs::write(program_path, &text).unwrap();
        if case != "skills-only" {
            package["program"]["digest"] = json!(coder::package::digest(&text));
        }
        std::fs::write(package_path, serde_json::to_vec_pretty(&package).unwrap()).unwrap();
        let signer = RelaySigner::from_secret_hex(&"03".repeat(32)).unwrap();
        let source = Arc::new(Source {
            relay: Mutex::new(Relay::default()),
            blobs: MemoryBlobs::default(),
            cache: work.path().join("cache"),
        });
        let packed = plugin_registry::pack(&dir, signer.pubkey()).unwrap();
        plugin_registry::publish(
            &packed,
            &signer,
            &mut *source.relay.lock().unwrap(),
            &source.blobs,
            Some(&Fee {
                msat: 1_000,
                payout: to_hex(payee_of([17; 32])),
            }),
            NOW,
        )
        .unwrap();
        let receiver = Arc::new(FakeReceiver {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
        });
        let sink = Arc::new(LedgerSink::in_memory());
        let front = front_with(work.path(), receiver.clone(), sink.clone(), source);
        let (response, event) = front.handle(
            &post(
                &format!("/v1/plugins/{}/invoke", packed.package),
                "ACTION: Ana sends notes.",
                vec![],
            ),
            NOW,
        );
        assert_eq!(response.status, 422, "{case}");
        assert_eq!(event.outcome, "unpriced", "{case}");
        assert_eq!(receiver.counter.load(Ordering::SeqCst), 0, "{case}");
        assert!(header(&response, PAYMENT_REQUIRED).is_none(), "{case}");
        sink.with(|ledger| assert!(ledger.since(0).unwrap().is_empty(), "{case}"));
    }
}
