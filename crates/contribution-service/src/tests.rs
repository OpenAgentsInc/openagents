//! Synthetic protected checkpoint evidence and fake central wallet behavior.
use super::*;
use crate::{source, types::*};
use gym::{
    ab::{Metric, MetricFloor, Rule},
    admission::*,
    gate::{Basis, Bound, Budget, Cost, Deployment, DeploymentRule, GatedPercentile, Profile},
    row::{DoorIdentity, Row},
    sales_evidence::{Reference, digest},
    sales_finance as finance,
    suite::{LockedLedger, Partition, Spend, Suite},
};
use nostr::{domain::Event, x402::test_invoice};
use openagents_wallet::{Balance, Channel, PaymentRecord, Proof, WalletError};
use pay_ledger::{Payee, PayoutState};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tenancy::training::{self, CandidateDoc, Corpus, Recipe, Role, Trial, TrialOutcome};
const NOW: i64 = 1_791_369_000;
fn key(n: u8) -> Keypair {
    Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_byte_array([n; 32]).unwrap(),
    )
}
fn pk(n: u8) -> String {
    key(n).x_only_public_key().0.to_string()
}
fn signed<T: Serialize>(n: u8, at: i64, value: &T) -> Vec<u8> {
    let pair = key(n);
    let mut event = Event {
        id: String::new(),
        pubkey: pk(n),
        created_at: at as u64,
        kind: 1,
        tags: vec![],
        content: serde_json::to_string(value).unwrap(),
        sig: String::new(),
    };
    let hash = event.computed_id_bytes().unwrap();
    event.id = hex(&hash);
    event.sig = Secp256k1::new()
        .sign_schnorr_no_aux_rand(&hash, &pair)
        .to_string();
    serde_json::to_vec(&event).unwrap()
}
fn put(root: &Path, name: &str, bytes: &[u8]) -> Reference {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    Reference {
        path: name.into(),
        sha256: digest(bytes),
    }
}
fn doc<T: Serialize>(root: &Path, name: &str, value: &T) -> Reference {
    put(root, name, &serde_json::to_vec(value).unwrap())
}
fn bound(value: f64) -> Bound {
    Bound {
        value: Some(value),
        basis: Basis::Derived,
        evidence: vec![],
        why: "Synthetic fixture only; no production threshold.".into(),
    }
}
fn floor(metric: Metric) -> MetricFloor {
    MetricFloor {
        metric,
        block_sigma: bound(0.01),
    }
}
fn make_corpus(book: &training::Book, prefix: &str) -> Corpus {
    let mut items = vec![];
    for (p, role) in Role::ALL.into_iter().enumerate() {
        for i in 0..4 {
            items.push(json!({"id":format!("{prefix}-{p}-{i}"),"group":format!("{prefix}-group-{p}-{i}"),"partition":role,"state":format!("{prefix}{p}{i}alpha {prefix}{p}{i}beta {prefix}{p}{i}gamma {prefix}{p}{i}delta {prefix}{p}{i}epsilon {prefix}{p}{i}zeta"),"question":{"type":"choice","question":"Choose the retained label","choices":["yes","no"]},"label":"yes","provenance":{"source":"synthetic-owner","license":"synthetic-license","permission":format!("{prefix}-permission-{p}-{i}")}}));
        }
    }
    book.register_corpus(&serde_json::to_string(&json!({"v":training::CORPUS_SCHEMA,"workspace":"fixture","name":prefix,"created":"2026-10-07","retention":{"days":30,"access":"operator","artifacts":"digests-only"},"items":items,"digest":""})).unwrap()).unwrap()
}
fn make_suite(corpus: &Corpus, name: &str) -> Suite {
    let items:Vec<_>=corpus.items.iter().filter(|i|i.partition!=Role::Training).map(|i|json!({"id":i.id,"family":"choice","kind":"choice","state":i.state,"question":i.question,"truth":i.label,"partition":i.partition})).collect();
    let mut suite:Suite=serde_json::from_value(json!({"schema":"openagents.gym.suite.v1","name":name,"description":"Protected synthetic fixture","created":"2026-10-07","tier":"scored","digest":"","items":items})).unwrap();
    suite.digest = suite.compute_digest().unwrap();
    Suite::load(&serde_json::to_string(&suite).unwrap()).unwrap()
}
fn plan(suite: &Suite, transfer: &Suite, candidate: &CandidateDoc, recipe: &Recipe) -> Plan {
    let base = DoorIdentity {
        model: recipe.base_model.id.clone(),
        artifact_signature: recipe.base_model.signature.clone(),
        verified: true,
        ..DoorIdentity::default()
    };
    let mut identity = base.clone();
    identity.adapter = candidate.name.clone();
    identity.artifact_signature = candidate.signature.clone();
    let mut p = Plan {
        schema: PLAN_SCHEMA.into(),
        id: "fixture-accuracy-v1".into(),
        question: "Does the protected accuracy improve?".into(),
        base: Pinned {
            door: "base".into(),
            identity: base,
        },
        candidate: Pinned {
            door: "candidate".into(),
            identity,
        },
        differences: vec!["artifact_signature".into(), "adapter".into()],
        workload: Workload {
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: None,
            question_digest: None,
            partitions: vec![Partition::Development],
            gate_digest: None,
        },
        instrument: Instrument {
            estimator: "fixture".into(),
            ..Instrument::default()
        },
        rule: Rule {
            id: "fixture-rule".into(),
            question: "Synthetic fixture only".into(),
            metric_order: vec![floor(Metric::Accuracy)],
            effect_size_sigmas: bound(1.0),
            family_regression_sigmas: bound(1.0),
            min_blocks_per_side: bound(1.0),
            requeue_limit: 0,
            covers: "Synthetic accuracy".into(),
            does_not_cover: "Commercial usefulness".into(),
            pending_measurements: vec![],
        },
        guards: Guards {
            max_new_refusals: bound(0.0),
            max_new_confident_errors: bound(0.0),
            calibration: [Metric::Ece, Metric::Brier, Metric::Nll]
                .into_iter()
                .map(floor)
                .collect(),
            transfer: TransferGuard {
                suite: transfer.name.clone(),
                suite_digest: transfer.digest.clone(),
                partitions: vec![Partition::Development],
                question_set: None,
                question_digest: None,
                block_sigma: bound(0.01),
                max_regression_sigmas: bound(1.0),
            },
            deployment: DeploymentGuard {
                rule: DeploymentRule {
                    min_calls: bound(1.0),
                    gated_percentile: GatedPercentile::P95,
                    latency_block_sigma_relative: bound(0.01),
                    regression_sigmas: bound(1.0),
                    pending_measurement: None,
                },
                budget: Budget::new("fixture", "synthetic")
                    .latency_ms(100.0)
                    .cost_usd(1.0)
                    .refusal_rate(0.0),
            },
        },
        scope: suite.families(),
        digest: String::new(),
    };
    p.seal();
    p
}
fn phase(
    root: &Path,
    name: &str,
    suite: &Suite,
    partition: Partition,
    plan: &Plan,
) -> (Phase, Vec<Row>, gym::commitment::Commitment) {
    let filename = format!("{name}.jsonl");
    let store = gym::store::Store::at(root.join(&filename));
    for pin in [&plan.base, &plan.candidate] {
        for item in suite.items.iter().filter(|i| i.partition == partition) {
            let selected = if pin.door == "candidate" { "yes" } else { "no" };
            let distribution = [
                (selected.into(), 0.8),
                (if selected == "yes" { "no" } else { "yes" }.into(), 0.2),
            ]
            .into_iter()
            .collect();
            let mut row = Row::new(&suite.name, &suite.digest, &item.id, &pin.door).scored_as(
                distribution,
                Some(selected.into()),
                selected == item.truth,
            );
            row.recorded_at = gym::eval::utc_from_unix((NOW - 50) as u64);
            row.split = partition.as_str().into();
            row.family = item.family.clone();
            row.estimator = plan.instrument.estimator.clone();
            row.door_identity = pin.identity.clone();
            row.latency_ms = Some(10.0);
            store.append(&row).unwrap();
        }
    }
    let rows: Vec<Row> = store
        .verified_rows()
        .unwrap()
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect();
    let doors = vec!["base".into(), "candidate".into()];
    let expected =
        gym::coverage::Expected::of(suite, &[partition], None, None, doors.clone()).unwrap();
    let commitment = gym::commitment::Commitment::of(
        suite,
        &expected,
        gym::commitment::Selection {
            partitions: vec![partition.as_str().into()],
            family: None,
            items: None,
            doors,
        },
        &rows,
        store.head().unwrap(),
        None,
    );
    let phase = Phase {
        store: put(root, &filename, &fs::read(root.join(&filename)).unwrap()),
        commitment: doc(root, &format!("{name}-commitment.json"), &commitment),
    };
    (phase, rows, commitment)
}
fn native_decision(
    plan: &Plan,
    suite: &Suite,
    transfer_suite: &Suite,
    dev: &(Phase, Vec<Row>, gym::commitment::Commitment),
    locked: &(Phase, Vec<Row>, gym::commitment::Commitment),
    transfer: &(Phase, Vec<Row>, gym::commitment::Commitment),
    ledger: &LockedLedger,
    deployment: &Deployment,
) -> String {
    let sides = |rows: &[Row]| {
        (
            rows.iter()
                .filter(|r| r.door == "base")
                .cloned()
                .collect::<Vec<_>>(),
            rows.iter()
                .filter(|r| r.door == "candidate")
                .cloned()
                .collect::<Vec<_>>(),
        )
    };
    let ds = sides(&dev.1);
    let ls = sides(&locked.1);
    let ts = sides(&transfer.1);
    let evidence = Evidence {
        reports: Reports {
            development: Some(ReportEvidence {
                rows: &dev.1,
                commitment: &dev.2,
            }),
            locked: Some(ReportEvidence {
                rows: &locked.1,
                commitment: &locked.2,
            }),
            transfer: Some(ReportEvidence {
                rows: &transfer.1,
                commitment: &transfer.2,
            }),
        },
        suite,
        development: Side {
            base: &ds.0,
            candidate: &ds.1,
            store_head: dev.2.head.clone(),
        },
        locked: Some(Locked {
            base: &ls.0,
            candidate: &ls.1,
            ledger,
            store_head: locked.2.head.clone(),
        }),
        transfer: Some(Transfer {
            suite: transfer_suite,
            base: &ts.0,
            candidate: &ts.1,
            store_head: transfer.2.head.clone(),
        }),
        deployment: Some(deployment.clone()),
        decided_at: gym::eval::utc_from_unix((NOW - 50) as u64),
        commitment: Some(dev.2.digest.clone()),
    };
    let record = tenancy::admission::Record::evaluate(plan, &evidence).unwrap();
    record.admitted().unwrap();
    record.reference().into()
}
struct Fixture {
    _scratch: tempfile::TempDir,
    config: Config,
    frozen: Frozen,
    evaluation: Evaluation,
    consent: Consent,
    current: Current,
}
impl Fixture {
    fn new() -> Self {
        let scratch =
            tempfile::tempdir_in(fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
        let protected = scratch.path().join("protected");
        let worker = scratch.path().join("worker");
        for root in [&protected, &worker] {
            fs::create_dir(root).unwrap();
            fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let book = training::Book::open(&protected).unwrap();
        let corpus = make_corpus(&book, "primary");
        let transfer_corpus = make_corpus(&book, "transfer");
        let baseline = put(&protected, "baseline.bin", b"synthetic baseline checkpoint");
        let recipe=book.freeze_recipe(&serde_json::to_string(&json!({"v":training::RECIPE_SCHEMA,"name":"recipe-v1","created":gym::eval::utc_from_unix((NOW-100)as u64),"base_model":{"id":"fixture","signature":format!("sha256:{}",baseline.sha256)},"adapter":{"kind":"lora","rank":1,"alpha":1,"targets":["q"],"epochs":1,"learning_rate":"0.001"},"head":{"kind":"pointer","dp":1},"seeds":[7],"trials_max":4,"budget":{"max_train_items":4,"max_steps":10,"max_seconds":30},"metric":{"statistic":"accuracy","baseline":0.0,"min_margin":0.1},"transfer_controls":{"serving":"candidate-only","publish":"digests-only"}})).unwrap()).unwrap();
        let artifacts: BTreeMap<String, Reference> = [
            ("adapter", b"synthetic adapter".as_slice()),
            ("head", b"synthetic head".as_slice()),
            ("tokenizer", b"synthetic tokenizer".as_slice()),
        ]
        .into_iter()
        .map(|(name, bytes)| (name.into(), put(&worker, name, bytes)))
        .collect();
        let pins: BTreeMap<String, String> = artifacts
            .iter()
            .map(|(k, v)| (k.clone(), format!("sha256:{}", v.sha256)))
            .collect();
        for (i, outcome) in [TrialOutcome::Failed, TrialOutcome::Kept]
            .into_iter()
            .enumerate()
        {
            book.record_trial(
                &serde_json::to_string(&Trial {
                    v: training::TRIAL_SCHEMA.into(),
                    recipe_digest: recipe.digest.clone(),
                    seed: 7,
                    params: BTreeMap::new(),
                    metrics: BTreeMap::from([("accuracy".into(), if i == 1 { 1.0 } else { 0.0 })]),
                    artifacts: if i == 1 {
                        pins.clone()
                    } else {
                        BTreeMap::new()
                    },
                    outcome,
                    reason: None,
                    recorded_at: gym::eval::utc_from_unix((NOW - 90 + i as i64) as u64),
                })
                .unwrap(),
            )
            .unwrap();
        }
        let candidate=book.seal_candidate(&serde_json::to_string(&json!({"v":training::CANDIDATE_SCHEMA,"workspace":"fixture","name":"candidate-v1","created":gym::eval::utc_from_unix((NOW-70)as u64),"identities":{"code":{"fixture":"synthetic"},"recipe_digest":recipe.digest,"corpus_digest":corpus.digest,"base_model":recipe.base_model,"adapter":pins["adapter"],"head":pins["head"],"tokenizer":pins["tokenizer"]},"evidence":{"trial":2,"metrics":{"accuracy":1.0}},"retention":corpus.retention})).unwrap()).unwrap();
        let suite = make_suite(&corpus, "primary");
        let transfer_suite = make_suite(&transfer_corpus, "transfer");
        let plan = plan(&suite, &transfer_suite, &candidate, &recipe);
        let mut train = BTreeSet::new();
        let mut eval = BTreeSet::new();
        let mut rights = vec![];
        for c in [&corpus, &transfer_corpus] {
            for item in &c.items {
                if item.partition == Role::Training {
                    train.insert(item.group.clone());
                } else {
                    eval.insert(item.group.clone());
                }
                let file = format!("rights/{}.json", item.id);
                put(
                    &protected,
                    &file,
                    &signed(
                        5,
                        NOW - 100,
                        &Rights {
                            schema: "openagents.contribution-rights.v1".into(),
                            permission: item.provenance.permission.clone(),
                            source: item.provenance.source.clone(),
                            group: item.group.clone(),
                            license: item.provenance.license.clone(),
                            corpus: c.digest.clone(),
                            actions: vec![
                                if item.partition == Role::Training {
                                    "train"
                                } else {
                                    "evaluate"
                                }
                                .into(),
                            ],
                            expires_at: NOW + 3600,
                        },
                    ),
                );
                rights.push(RightPin {
                    issuer: pk(5),
                    file,
                });
            }
        }
        let hash = |set: &BTreeSet<String>| digest(&serde_json::to_vec(set).unwrap());
        let attribution = doc(
            &protected,
            "attribution.json",
            &Attribution {
                schema: "openagents.contribution-attribution.v1".into(),
                beneficiary: pk(1),
                corpus: corpus.digest.clone(),
                transfer_corpus: transfer_corpus.digest.clone(),
            },
        );
        let terms = pay_ledger::markets::contribution::Terms {
            class: pay_ledger::markets::contribution::Class::VerifiedOptimization,
            obligation: "1".repeat(64),
            source: corpus.digest.trim_start_matches("sha256:").into(),
            source_group: hash(&train),
            evaluation_group: hash(&eval),
            license: hash(&BTreeSet::from(["synthetic-license".into()])),
            attribution: attribution.sha256.clone(),
            beneficiary: pk(1),
            protected_evaluator: pk(2),
            evaluation_policy: evaluate::plan_policy(&plan),
            acceptance_authority: pk(3),
            funding_authority: pk(4),
            artifact_contract: digest(evaluate::ARTIFACT_CONTRACT.as_bytes()),
            reward_msat: 10_000,
            committed_at: NOW - 100,
            expires_at: NOW + 3600,
        };
        let reference = |name: &str| Reference {
            path: name.into(),
            sha256: digest(&fs::read(protected.join(name)).unwrap()),
        };
        let frozen = Frozen {
            schema: SCHEMA.into(),
            terms,
            corpus: reference("training/corpora/primary.json"),
            transfer_corpus: reference("training/corpora/transfer.json"),
            recipe: reference("training/recipes/recipe-v1.json"),
            baseline,
            attribution,
            plan_policy: evaluate::plan_policy(&plan),
            rights,
            max_all_in_msat: 1_000,
            platform_fee_msat: 500,
        };
        let frozen_ref = put(&protected, "frozen.json", &signed(4, NOW - 100, &frozen));
        let frozen_event: Event = source::json(&protected, &frozen_ref).unwrap();
        let profile = Profile::timed(&[10.0; 4])
            .refusing(0)
            .costing(Cost::Metered {
                usd_per_decision: 0.01,
            });
        let deployment = Deployment::new("fixture", profile.clone(), profile);
        let dev = phase(
            &protected,
            "development",
            &suite,
            Partition::Development,
            &plan,
        );
        let locked = phase(&protected, "locked", &suite, Partition::Locked, &plan);
        let transfer = phase(
            &protected,
            "transfer",
            &transfer_suite,
            Partition::Development,
            &plan,
        );
        let locked_ledger = LockedLedger::at(protected.join("locked-ledger.jsonl"));
        locked_ledger
            .read_locked(
                &suite,
                &Spend {
                    subject: &plan.ledger_subject(),
                    reason: "Synthetic contribution confirmation",
                    at: &gym::eval::utc_from_unix((NOW - 50) as u64),
                    adapter: &plan.candidate.identity.adapter,
                },
            )
            .unwrap();
        let decision = native_decision(
            &plan,
            &suite,
            &transfer_suite,
            &dev,
            &locked,
            &transfer,
            &locked_ledger,
            &deployment,
        );
        let categories = [
            (CostClass::Training, finance::ExpenseClass::Setup, Some(1)),
            (CostClass::Compute, finance::ExpenseClass::Compute, Some(1)),
            (
                CostClass::FailedAttempt,
                finance::ExpenseClass::Repair,
                Some(1),
            ),
            (
                CostClass::Training,
                finance::ExpenseClass::Onboarding,
                Some(2),
            ),
            (CostClass::Compute, finance::ExpenseClass::Compute, Some(2)),
            (CostClass::Checking, finance::ExpenseClass::Support, None),
            (CostClass::Checking, finance::ExpenseClass::Payment, None),
            (CostClass::Search, finance::ExpenseClass::Provider, None),
            (CostClass::Search, finance::ExpenseClass::Promotion, None),
            (CostClass::License, finance::ExpenseClass::Fulfillment, None),
        ];
        let mut costs = vec![];
        let mut expenses = vec![];
        for (i, (class, expense_class, trial)) in categories.into_iter().enumerate() {
            let id = format!("bill-{i}");
            let evidence = doc(
                &protected,
                &format!("bills/{id}.json"),
                &CostLine {
                    schema: "openagents.contribution-cost-line.v1".into(),
                    expense: id.clone(),
                    obligation: frozen.terms.obligation.clone(),
                    class,
                    trial,
                    amount_msat: 10,
                },
            );
            costs.push(CostBinding {
                class,
                expense: id.clone(),
                trial,
            });
            expenses.push(finance::Expense {
                id,
                class: expense_class,
                basis: finance::Basis::Billed,
                unit: "msat".into(),
                amount: Some(10),
                payer: finance::Payer::OpenAgents,
                evidence: Some(evidence),
                price: None,
            });
        }
        let terms_evidence = put(
            &protected,
            "cost-terms.txt",
            b"Synthetic cost-only evidence; no collection",
        );
        let receipt = finance::CommercialReceipt {
            schema: "openagents.sales.commercial-receipt.v1".into(),
            id: "cost-only".into(),
            account: "customer-fixture".into(),
            offer_version: "cost-v1".into(),
            at: NOW as u64,
            kind: finance::CollectionKind::FreeTrial,
            unit: "msat".into(),
            contractual_charge: 0,
            collected: 0,
            terms_digest: terms_evidence.sha256.clone(),
            evidence: put(&protected, "cost-source.txt", b"No money collected"),
        };
        let finance_manifest = finance::Manifest {
            schema: finance::SCHEMA.into(),
            owner: pk(4),
            period_start: NOW as u64 - 200,
            period_end: NOW as u64 + 3600,
            inventory: doc(
                &protected,
                "cost-inventory.json",
                &finance::Inventory {
                    schema: "openagents.gym.sales-finance-inventory.v1".into(),
                    entries: vec!["work-costs".into()],
                    complete: true,
                },
            ),
            comparisons: BTreeMap::new(),
            offers: vec![finance::Offer {
                id: "contribution-costs".into(),
                version: "cost-v1".into(),
                account: "customer-fixture".into(),
                cohort: "synthetic".into(),
                entries: vec![finance::Entry {
                    id: "work-costs".into(),
                    at: NOW as u64,
                    terms: finance::Terms {
                        version: "cost-v1".into(),
                        evidence: terms_evidence.clone(),
                        unit: "msat".into(),
                        contractual_charge: 0,
                        billable_failure: false,
                    },
                    source: finance::Source::Commercial {
                        receipt: doc(&protected, "cost-receipt.json", &receipt),
                    },
                    delivery: finance::Delivery::Pending,
                    delivery_evidence: terms_evidence,
                    task: None,
                    expenses,
                    adjustments: vec![],
                    incidents: vec![],
                }],
                no_cost: BTreeMap::new(),
                assumptions: vec!["Synthetic costs only".into()],
            }],
            gaps: vec![],
        };
        let evaluation = Evaluation {
            schema: "openagents.contribution-evaluation.v1".into(),
            frozen: frozen_event.id.clone(),
            candidate: reference("training/candidates/candidate-v1.json"),
            trials: reference("training/trials.jsonl"),
            artifacts,
            plan: doc(&protected, "plan.json", &plan),
            suite: doc(&protected, "suite.json", &suite),
            transfer_suite: doc(&protected, "transfer-suite.json", &transfer_suite),
            development: dev.0,
            locked: locked.0,
            locked_ledger: reference("locked-ledger.jsonl"),
            transfer: transfer.0,
            deployment: doc(&protected, "deployment.json", &deployment),
            finance: doc(&protected, "finance.json", &finance_manifest),
            finance_offer: "contribution-costs".into(),
            costs,
            evaluated_at: NOW - 50,
        };
        let evaluation_ref = put(
            &protected,
            "evaluation.json",
            &signed(2, NOW - 50, &evaluation),
        );
        let event: Event = source::json(&protected, &evaluation_ref).unwrap();
        let consent = Consent {
            schema: "openagents.contribution-acceptance.v1".into(),
            frozen: frozen_event.id.clone(),
            evaluation: event.id,
            decision,
            artifact: candidate.signature,
            recipe: recipe.digest,
            improvement: 4,
            accepted_at: NOW - 40,
        };
        let acceptance = put(
            &protected,
            "acceptance.json",
            &signed(3, NOW - 40, &consent),
        );
        let current = Current {
            schema: "openagents.contribution-current.v1".into(),
            frozen: frozen_event.id,
            enabled: true,
            expires_at: NOW + 3600,
            central_node: hex(&test_invoice::payee_of([9; 32])),
            destination_kind: "spark".into(),
            destination_value: "synthetic-destination".into(),
        };
        put(&protected, "current.json", &signed(4, NOW - 30, &current));
        let ledger_path = scratch.path().join("central.sqlite");
        let mut ledger = Ledger::open(&ledger_path).unwrap();
        ledger
            .register_payee(Payee {
                party: pk(1),
                destination_kind: current.destination_kind.clone(),
                destination_value: current.destination_value.clone(),
                source: "synthetic independent owner admission".into(),
                verified_at: NOW - 30,
            })
            .unwrap();
        drop(ledger);
        fs::set_permissions(&ledger_path, fs::Permissions::from_mode(0o600)).unwrap();
        let config = Config {
            schema: SCHEMA.into(),
            authority: pk(4),
            protected_root: protected,
            worker_root: worker,
            state: scratch.path().join("state"),
            ledger: ledger_path,
            frozen: frozen_ref,
            current: "current.json".into(),
            evaluation: evaluation_ref,
            acceptance,
        };
        Self {
            _scratch: scratch,
            config,
            frozen,
            evaluation,
            consent,
            current,
        }
    }
    fn host(&self) -> Host {
        Host::open(self.config.clone()).unwrap()
    }
    fn update_evaluation(&mut self) {
        self.config.evaluation = put(
            &self.config.protected_root,
            "evaluation.json",
            &signed(2, NOW - 50, &self.evaluation),
        );
        let event: Event =
            source::json(&self.config.protected_root, &self.config.evaluation).unwrap();
        self.consent.evaluation = event.id;
        self.config.acceptance = put(
            &self.config.protected_root,
            "acceptance.json",
            &signed(3, NOW - 40, &self.consent),
        );
    }
    fn update_frozen(&mut self) {
        self.config.frozen = put(
            &self.config.protected_root,
            "frozen.json",
            &signed(4, NOW - 100, &self.frozen),
        );
        let event: Event = source::json(&self.config.protected_root, &self.config.frozen).unwrap();
        self.current.frozen = event.id.clone();
        put(
            &self.config.protected_root,
            "current.json",
            &signed(4, NOW - 30, &self.current),
        );
        self.evaluation.frozen = event.id.clone();
        self.consent.frozen = event.id;
        self.update_evaluation();
    }
}
struct Wallet {
    issued: AtomicUsize,
    bound_receives: AtomicUsize,
    bound_lookups: AtomicUsize,
    lookup_dispatches: AtomicUsize,
    resident_node: Mutex<String>,
    record: Mutex<Option<PaymentRecord>>,
    lose_issue: bool,
    minted_at: i64,
}
impl Wallet {
    fn new() -> Self {
        Self {
            issued: AtomicUsize::new(0),
            bound_receives: AtomicUsize::new(0),
            bound_lookups: AtomicUsize::new(0),
            lookup_dispatches: AtomicUsize::new(0),
            resident_node: Mutex::new(hex(&test_invoice::payee_of([9; 32]))),
            record: Mutex::new(None),
            lose_issue: false,
            minted_at: NOW,
        }
    }
    fn fund(&self, f: &Funding) {
        let invoice = f.invoice.as_ref().unwrap();
        *self.record.lock().unwrap() = Some(PaymentRecord {
            payment_hash: invoice.payment_hash.clone(),
            direction: PaymentDirection::Inbound,
            status: PaymentStatus::Succeeded,
            amount_msat: Some(invoice.amount_msat),
            fee_msat: Some(0),
            preimage: Some(hex(&[7; 32])),
            bolt11: Some(invoice.bolt11.clone()),
            updated_at: (NOW + 1) as u64,
        });
    }
}
impl LightningWallet for Wallet {
    fn node_id(&self) -> String {
        hex(&test_invoice::payee_of([9; 32]))
    }
    fn receive_exact(
        &self,
        amount: u64,
        hash: [u8; 32],
        expiry: u32,
    ) -> std::result::Result<IssuedInvoice, WalletError> {
        self.issued.fetch_add(1, Ordering::SeqCst);
        if self.lose_issue {
            return Err(WalletError::Node(
                "synthetic lost invoice acknowledgment".into(),
            ));
        }
        let payment_hash: [u8; 32] = Sha256::digest([7; 32]).into();
        let mut fields = test_invoice::tag(1, &test_invoice::words(&payment_hash));
        fields.extend(test_invoice::tag(16, &test_invoice::words(&[2; 32])));
        fields.extend(test_invoice::tag(23, &test_invoice::words(&hash)));
        fields.extend(test_invoice::tag(6, &test_invoice::number(expiry as u64)));
        let bolt11 = test_invoice::signed_by(
            [9; 32],
            &format!("lntb{}p", amount * 10),
            fields,
            true,
            false,
            self.minted_at as u64,
        );
        Ok(IssuedInvoice {
            bolt11,
            payment_hash: hex(&payment_hash),
            amount_msat: amount,
            description_hash: hex(&hash),
            expiry_secs: expiry,
            pay_to: self.node_id(),
        })
    }
    fn receive_exact_from_node(
        &self,
        expected: &str,
        amount: u64,
        hash: [u8; 32],
        expiry: u32,
    ) -> std::result::Result<IssuedInvoice, WalletError> {
        self.bound_receives.fetch_add(1, Ordering::SeqCst);
        let actual = self.resident_node.lock().unwrap();
        if *actual != expected {
            return Err(WalletError::NodeMismatch {
                expected_node: expected.into(),
                actual_node: actual.clone(),
            });
        }
        self.receive_exact(amount, hash, expiry)
    }
    fn lookup_from_node(
        &self,
        expected: &str,
        hash: [u8; 32],
    ) -> std::result::Result<Option<PaymentRecord>, WalletError> {
        self.bound_lookups.fetch_add(1, Ordering::SeqCst);
        let actual = self.resident_node.lock().unwrap();
        if *actual != expected {
            return Err(WalletError::NodeMismatch {
                expected_node: expected.into(),
                actual_node: actual.clone(),
            });
        }
        self.lookup(hash)
    }
    fn lookup(&self, _: [u8; 32]) -> std::result::Result<Option<PaymentRecord>, WalletError> {
        self.lookup_dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(self.record.lock().unwrap().clone())
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> std::result::Result<Proof, WalletError> {
        panic!("the contribution receive adapter never sends funds")
    }
    fn balance(&self) -> std::result::Result<Balance, WalletError> {
        unimplemented!()
    }
    fn channels(&self) -> std::result::Result<Vec<Channel>, WalletError> {
        unimplemented!()
    }
    fn funding_address(&self) -> std::result::Result<String, WalletError> {
        unimplemented!()
    }
    fn open_channel(
        &self,
        _: &str,
        _: &str,
        _: u64,
        _: bool,
    ) -> std::result::Result<String, WalletError> {
        unimplemented!()
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> std::result::Result<(), WalletError> {
        unimplemented!()
    }
}
#[test]
fn protected_accepted_work_creates_one_funded_central_liability_and_recovers() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let report = host.assess(&wallet, NOW).unwrap();
    assert_eq!(report.improvement, 4);
    assert_eq!(report.all_in_msat, 100);
    assert!(!report.serving_activated);
    let funding = host.prepare(&wallet, NOW).unwrap();
    assert_eq!(
        host.reconcile(&wallet, || Ok(NOW)).unwrap().state,
        "funding_unknown"
    );
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
    wallet.fund(&funding);
    assert_eq!(
        host.reconcile(&wallet, || Ok(NOW + 1)).unwrap().state,
        "funded_liability"
    );
    assert_eq!(
        host.statement().unwrap()["funding"]["receiver_evidence"]["transferred_msat"],
        10_500
    );
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 10_000);
    assert_eq!(host.ledger.accrued(pay_ledger::OPENAGENTS).unwrap(), 500);
    drop(host);
    let mut host = f.host();
    assert_eq!(
        host.reconcile(&wallet, || Ok(NOW + 2)).unwrap().state,
        "funded_liability"
    );
    assert_eq!(
        host.prepare(&wallet, NOW + 2).unwrap().invoice,
        funding.invoice
    );
    assert_eq!(wallet.issued.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.bound_receives.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.bound_lookups.load(Ordering::SeqCst), 2);
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 10_000);
    assert_eq!(
        host.statement().unwrap()["settlement"]["shares"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["amount_msat"].as_i64().unwrap() > 0)
            .count(),
        2
    );
}
#[test]
fn changed_artifacts_recipes_rights_groups_and_forged_worker_results_refuse() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    fs::write(
        f.config.worker_root.join("adapter"),
        b"different checkpoint",
    )
    .unwrap();
    assert!(
        f.host()
            .assess(&wallet, NOW)
            .unwrap_err()
            .contains("artifact")
    );
    let f = Fixture::new();
    let path = f.config.protected_root.join(&f.frozen.recipe.path);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["seeds"] = json!([8]);
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(f.host().assess(&wallet, NOW).is_err());
    let f = Fixture::new();
    let pin = &f.frozen.rights[0];
    let (_, mut rights): (_, Rights) = source::signed(
        &source::bytes(&f.config.protected_root, &pin.file).unwrap(),
        &pin.issuer,
        NOW,
    )
    .unwrap();
    rights.actions.clear();
    put(
        &f.config.protected_root,
        &pin.file,
        &signed(5, NOW - 100, &rights),
    );
    assert!(f.host().assess(&wallet, NOW).is_err());
    let mut f = Fixture::new();
    f.frozen.terms.evaluation_group = f.frozen.terms.source_group.clone();
    f.update_frozen();
    assert!(f.host().assess(&wallet, NOW).is_err());
    let mut f = Fixture::new();
    f.config.evaluation = put(
        &f.config.protected_root,
        "evaluation.json",
        &signed(1, NOW - 50, &f.evaluation),
    );
    assert!(f.host().assess(&wallet, NOW).is_err());
    let mut f = Fixture::new();
    f.consent.improvement = 500;
    f.config.acceptance = put(
        &f.config.protected_root,
        "acceptance.json",
        &signed(3, NOW - 40, &f.consent),
    );
    assert!(f.host().assess(&wallet, NOW).is_err());
}
#[test]
fn incomplete_unknown_over_budget_or_unjoined_attempt_costs_refuse() {
    let wallet = Wallet::new();
    let mut f = Fixture::new();
    f.evaluation.costs.retain(|c| c.class != CostClass::License);
    f.update_evaluation();
    assert!(f.host().assess(&wallet, NOW).is_err());
    let mut f = Fixture::new();
    f.evaluation.costs.retain(|c| c.trial != Some(1));
    f.update_evaluation();
    assert!(f.host().assess(&wallet, NOW).is_err());
    let mut f = Fixture::new();
    let mut manifest: finance::Manifest =
        source::json(&f.config.protected_root, &f.evaluation.finance).unwrap();
    manifest.offers[0].entries[0].expenses[0].basis = finance::Basis::Unknown;
    manifest.offers[0].entries[0].expenses[0].amount = None;
    f.evaluation.finance = doc(&f.config.protected_root, "finance.json", &manifest);
    f.update_evaluation();
    assert!(f.host().assess(&wallet, NOW).is_err());
    let mut f = Fixture::new();
    f.frozen.max_all_in_msat = 99;
    f.update_frozen();
    assert!(f.host().assess(&wallet, NOW).is_err());
}
#[test]
fn expired_or_revoked_terms_and_worker_custody_cannot_admit_funding() {
    let wallet = Wallet::new();
    let f = Fixture::new();
    assert!(f.host().prepare(&wallet, NOW + 3600).is_err());
    assert_eq!(wallet.issued.load(Ordering::SeqCst), 0);
    let mut f = Fixture::new();
    f.current.enabled = false;
    put(
        &f.config.protected_root,
        "current.json",
        &signed(4, NOW - 30, &f.current),
    );
    assert!(f.host().prepare(&wallet, NOW).is_err());
    let f = Fixture::new();
    let mut config = f.config.clone();
    config.protected_root = config.worker_root.clone();
    assert!(Host::open(config).is_err());
}
#[test]
fn lost_invoice_issue_never_creates_a_second_original() {
    let f = Fixture::new();
    let wallet = Wallet {
        lose_issue: true,
        ..Wallet::new()
    };
    let mut host = f.host();
    assert!(host.prepare(&wallet, NOW).is_err());
    drop(host);
    let mut host = f.host();
    assert_eq!(
        host.prepare(&wallet, NOW + 1).unwrap().state,
        "issuance_unknown"
    );
    assert!(host.reconcile(&wallet, || Ok(NOW + 1)).is_err());
    assert_eq!(wallet.issued.load(Ordering::SeqCst), 1);
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
}
#[test]
fn new_hash_outbound_wrong_amount_or_unknown_funding_cannot_earn() {
    for variation in 0..5 {
        let f = Fixture::new();
        let wallet = Wallet::new();
        let mut host = f.host();
        let funding = host.prepare(&wallet, NOW).unwrap();
        wallet.fund(&funding);
        {
            let mut guard = wallet.record.lock().unwrap();
            let p = guard.as_mut().unwrap();
            match variation {
                0 => p.payment_hash = "9".repeat(64),
                1 => p.direction = PaymentDirection::Outbound,
                2 => p.amount_msat = Some(10_000),
                3 => p.status = PaymentStatus::Pending,
                _ => p.bolt11 = Some("conflicting invoice".into()),
            }
        }
        let result = host.reconcile(&wallet, || Ok(NOW + 1));
        if variation == 3 {
            assert_eq!(result.unwrap().state, "funding_unknown");
        } else {
            assert!(result.is_err());
        }
        assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
    }
}
#[test]
fn unknown_payout_retains_exact_central_liability_without_duplicate_reward() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    host.reconcile(&wallet, || Ok(NOW + 1)).unwrap();
    let record = host
        .ledger
        .settlement(&funding.invoice.unwrap().payment_hash)
        .unwrap()
        .unwrap();
    let shares: Vec<_> = record
        .shares
        .into_iter()
        .filter(|s| s.party == pk(1))
        .collect();
    host.ledger
        .reserve_payout("synthetic-payout", &pk(1), &shares, NOW + 1)
        .unwrap();
    host.ledger
        .begin_send(
            "synthetic-payout",
            "fake-wallet-reference",
            None,
            10_000,
            NOW + 1,
        )
        .unwrap();
    host.ledger
        .finish_payout(
            "synthetic-payout",
            PayoutState::Unknown,
            None,
            Some("synthetic lost send reply"),
            NOW + 1,
        )
        .unwrap();
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
    drop(host);
    let mut host = f.host();
    host.reconcile(&wallet, || Ok(NOW + 2)).unwrap();
    assert_eq!(
        host.ledger
            .payout("synthetic-payout")
            .unwrap()
            .unwrap()
            .state,
        PayoutState::Unknown
    );
    assert_eq!(
        host.statement().unwrap()["liabilities"][0]["amount_msat"],
        10_000
    );
    assert_eq!(host.statement().unwrap()["payouts"][0]["state"], "unknown");
    assert_eq!(
        host.statement().unwrap()["payouts"][0]["lookup_required"],
        true
    );
    assert!(
        host.ledger
            .reserve_payout("duplicate", &pk(1), &shares, NOW + 2)
            .is_err()
    );
}

#[test]
fn accepted_funding_exports_native_customer_costs_and_exact_liability() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    assert!(
        host.export_finance(&wallet, NOW, &f._scratch.path().join("before"))
            .is_err()
    );
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    host.reconcile(&wallet, || Ok(NOW + 1)).unwrap();
    let output = f._scratch.path().join("finance-export");
    let report = host.export_finance(&wallet, NOW + 1, &output).unwrap();
    assert_eq!(report.gross_collected_msat, 10_500);
    assert_eq!(report.platform_allocated_msat, 500);
    assert_eq!(report.beneficiary_outstanding_msat, 10_000);
    assert_eq!(report.known_operating_cost_msat, 100);
    assert_eq!(report.operating_margin_msat, Some(400));
    assert_eq!(
        report.customer_costs.offers[0]
            .costs
            .iter()
            .map(|c| c.known_subtotal)
            .sum::<u64>(),
        100
    );
    assert!(
        report.customer_costs.offers[0]
            .missing_cost_classes
            .is_empty()
    );
    assert_eq!(
        gym::sales_finance::rebuild(
            &output,
            &fs::read(output.join("contribution/finance.json")).unwrap()
        )
        .unwrap()
        .manifest_digest,
        report.customer_costs.manifest_digest
    );
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(host.export_finance(&wallet, NOW + 1, &output).is_err());
    assert!(
        !host.statement().unwrap()["serving_activated"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn re_signed_receipt_chain_cannot_relabel_a_wrong_answer_as_an_improvement() {
    let mut f = Fixture::new();
    let suite: Suite = source::json(&f.config.protected_root, &f.evaluation.suite).unwrap();
    let path = f
        .config
        .protected_root
        .join(&f.evaluation.locked.store.path);
    let store = gym::store::Store::at(&path);
    let mut rows: Vec<Row> = store
        .verified_rows()
        .unwrap()
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect();
    for r in &mut rows {
        if r.door == "candidate" {
            r.selected = Some("no".into());
            r.correct = Some(true);
            r.raw_top = Some(0.2);
        }
    }
    fs::remove_file(&path).unwrap();
    for row in &rows {
        store.append(row).unwrap();
    }
    let rows: Vec<Row> = store
        .verified_rows()
        .unwrap()
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect();
    let doors = vec!["base".into(), "candidate".into()];
    let expected =
        gym::coverage::Expected::of(&suite, &[Partition::Locked], None, None, doors.clone())
            .unwrap();
    let commitment = gym::commitment::Commitment::of(
        &suite,
        &expected,
        gym::commitment::Selection {
            partitions: vec!["locked".into()],
            family: None,
            items: None,
            doors,
        },
        &rows,
        store.head().unwrap(),
        None,
    );
    f.evaluation.locked.store.sha256 = digest(&fs::read(&path).unwrap());
    f.evaluation.locked.commitment = doc(
        &f.config.protected_root,
        "locked-commitment.json",
        &commitment,
    );
    f.update_evaluation();
    assert!(
        f.host()
            .assess(&Wallet::new(), NOW)
            .unwrap_err()
            .contains("retained label")
    );
}

#[test]
fn altered_native_seal_and_unbound_attribution_refuse_even_with_owner_source_pins() {
    let mut f = Fixture::new();
    let mut candidate: CandidateDoc =
        source::json(&f.config.protected_root, &f.evaluation.candidate).unwrap();
    candidate
        .identities
        .code
        .insert("version".into(), "forged".into());
    f.evaluation.candidate = doc(
        &f.config.protected_root,
        &f.evaluation.candidate.path,
        &candidate,
    );
    f.update_evaluation();
    assert!(
        f.host()
            .assess(&Wallet::new(), NOW)
            .unwrap_err()
            .contains("seal")
    );
    let mut f = Fixture::new();
    f.frozen.terms.attribution = "a".repeat(64);
    f.update_frozen();
    assert!(
        f.host()
            .assess(&Wallet::new(), NOW)
            .unwrap_err()
            .contains("attribution")
    );
}

#[test]
fn actual_ldk_record_shape_funds_exact_transferred_amount_and_keeps_fees_unknown() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    {
        let mut record = wallet.record.lock().unwrap();
        let p = record.as_mut().unwrap();
        p.bolt11 = None;
        p.fee_msat = None;
    }
    assert_eq!(
        host.reconcile(&wallet, || Ok(NOW + 1)).unwrap().state,
        "funded_liability"
    );
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 10_000);
    let report = host
        .export_finance(
            &wallet,
            NOW + 1,
            &f._scratch.path().join("ldk-shaped-export"),
        )
        .unwrap();
    assert!(
        report.customer_costs.offers[0]
            .costs
            .iter()
            .any(|c| c.class == finance::ExpenseClass::Payment
                && c.basis == finance::Basis::Unknown
                && c.unknown_items == 1)
    );
    assert_eq!(report.profitable, None);
    assert_eq!(report.operating_margin_msat, None);
    assert!(host.statement().unwrap()["funding"]["reported_payment_fee_msat"].is_null());
    let f = Fixture::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    {
        let mut record = wallet.record.lock().unwrap();
        let p = record.as_mut().unwrap();
        p.bolt11 = None;
        p.fee_msat = None;
        p.amount_msat = Some(10_499);
    }
    assert!(host.reconcile(&wallet, || Ok(NOW + 1)).is_err());
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
}

#[test]
fn actual_receiver_can_mint_later_within_the_bounded_call_window() {
    let f = Fixture::new();
    let wallet = Wallet {
        minted_at: NOW + 8,
        ..Wallet::new()
    };
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    assert_eq!(funding.intent_at, NOW);
    assert_eq!(funding.created_at, NOW + 8);
    assert!(funding.expires_at <= funding.admitted_expires_at);
    wallet.fund(&funding);
    wallet.record.lock().unwrap().as_mut().unwrap().updated_at = (NOW + 9) as u64;
    assert_eq!(
        host.reconcile(&wallet, || Ok(NOW + 9)).unwrap().state,
        "funded_liability"
    );
    let f = Fixture::new();
    let wallet = Wallet {
        minted_at: NOW + 61,
        ..Wallet::new()
    };
    let mut host = f.host();
    assert!(host.prepare(&wallet, NOW).is_err());
    assert_eq!(
        host.prepare(&wallet, NOW + 1).unwrap().state,
        "issuance_unknown"
    );
    assert_eq!(wallet.issued.load(Ordering::SeqCst), 1);
}

#[test]
fn customer_paid_costs_remain_separate_from_platform_operating_expense() {
    let mut f = Fixture::new();
    let mut manifest: finance::Manifest =
        source::json(&f.config.protected_root, &f.evaluation.finance).unwrap();
    manifest.offers[0].entries[0].expenses[0].payer = finance::Payer::Customer;
    f.evaluation.finance = doc(&f.config.protected_root, "finance.json", &manifest);
    f.update_evaluation();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    host.reconcile(&wallet, || Ok(NOW + 1)).unwrap();
    let report = host
        .export_finance(&wallet, NOW + 1, &f._scratch.path().join("customer-costs"))
        .unwrap();
    assert_eq!(report.evaluated.all_in_msat, 100);
    assert_eq!(report.known_operating_cost_msat, 90);
    assert_eq!(report.known_customer_cost_msat, 10);
    assert_eq!(report.operating_margin_msat, Some(410));
}

#[test]
fn changed_resident_receiver_dispatches_neither_invoice_nor_funding_lookup() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    *wallet.resident_node.lock().unwrap() = hex(&test_invoice::payee_of([6; 32]));
    let mut host = f.host();
    assert!(
        host.prepare(&wallet, NOW)
            .unwrap_err()
            .contains("receiver changed")
    );
    assert_eq!(wallet.issued.load(Ordering::SeqCst), 0);
    assert_eq!(host.retained().unwrap().unwrap().state, "receiver_refused");
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    *wallet.resident_node.lock().unwrap() = hex(&test_invoice::payee_of([6; 32]));
    assert!(
        host.reconcile(&wallet, || Ok(NOW + 1))
            .unwrap_err()
            .contains("receiver changed")
    );
    assert_eq!(wallet.lookup_dispatches.load(Ordering::SeqCst), 0);
    assert_eq!(host.retained().unwrap().unwrap().state, "receiver_refused");
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
}

#[test]
fn funding_refreshes_time_after_lookup_and_before_central_accrual() {
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    let mut times = [NOW, NOW + 1, NOW + 1].into_iter();
    assert_eq!(
        host.reconcile(&wallet, || Ok(times.next().unwrap()))
            .unwrap()
            .state,
        "funded_liability"
    );
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    let mut times = [NOW, NOW + 1, NOW + 3600].into_iter();
    assert!(
        host.reconcile(&wallet, || Ok(times.next().unwrap()))
            .is_err()
    );
    assert_eq!(host.retained().unwrap().unwrap().state, "admission_refused");
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
    let f = Fixture::new();
    let wallet = Wallet::new();
    let mut host = f.host();
    let funding = host.prepare(&wallet, NOW).unwrap();
    wallet.fund(&funding);
    let mut times = [NOW + 1, NOW].into_iter();
    assert!(
        host.reconcile(&wallet, || Ok(times.next().unwrap()))
            .is_err()
    );
    assert_eq!(host.ledger.accrued(&pk(1)).unwrap(), 0);
}
