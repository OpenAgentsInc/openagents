//! Throwaway signed public sources shared by discovery and CLI acceptance tests.
#![allow(dead_code)]

use discovery::curated::{Catalog, Item, Review, SCHEMA, Source};
use nostr::contracts::{ArtifactRef, digest_bytes, jcs};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::{eval_ext, ext};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct Memory {
    pub heads: BTreeMap<(u16, String, String), Vec<Event>>,
    pub events: BTreeMap<String, Event>,
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

impl Source for Memory {
    fn heads(&mut self, kind: u16, publisher: &str, slug: &str) -> Result<Vec<Event>, String> {
        self.heads
            .get(&(kind, publisher.into(), slug.into()))
            .cloned()
            .ok_or("Head unavailable.".into())
    }
    fn event(&mut self, id: &str) -> Result<Event, String> {
        self.events
            .get(id)
            .cloned()
            .ok_or("Event unavailable.".into())
    }
    fn artifact(&mut self, reference: &ArtifactRef) -> Result<Vec<u8>, String> {
        self.artifacts
            .get(&reference.digest)
            .cloned()
            .ok_or("Artifact unavailable.".into())
    }
}

pub fn signer(byte: &str) -> RelaySigner {
    RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
}
pub fn pointer(event: &Event) -> Value {
    json!({"id":event.id,"pubkey":event.pubkey,"kind":event.kind})
}
pub fn art(bytes: &[u8], schema: Option<&str>) -> Value {
    let mut value =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
pub fn sign(
    signer: &RelaySigner,
    now: u64,
    kind: u16,
    ty: &str,
    slug: Option<&str>,
    body: Value,
) -> Event {
    let mut tags = vec![Tag::new(vec!["t".into(), format!("oa:ext:{ty}:v1")])];
    if let Some(slug) = slug {
        tags.push(Tag::new(vec!["d".into(), slug.into()]));
    }
    signer.sign(now, kind, tags, body.to_string())
}
pub fn manifest_value(
    package: &str,
    component: &str,
    kind: &str,
    definition: Value,
    files: Vec<(&str, Value)>,
) -> Value {
    let mut selected = json!({"slug":component,"kind":kind,"definition":definition});
    if kind == "program" {
        selected["descriptor"] = files
            .iter()
            .find(|(path, _)| *path == "package.json")
            .unwrap()
            .1
            .clone();
        selected["descriptor"]["schema"] = json!("openagents.coder-package.v1");
    }
    json!({"v":"openagents.package.v1","requires":[],"package":package,"version":"1.0.0","license":"MIT","provenance":{"source":"local","receipts":[],"unknowns":["independent operator"]},"components":[selected],"files":files.into_iter().map(|(path,a)|json!({"path":path,"digest":a["digest"],"size":a["size"],"media_type":a["media_type"]})).collect::<Vec<_>>(),"dependencies":[]})
}

pub struct Fixture {
    pub catalog: Catalog,
    pub source: Memory,
    pub publisher: RelaySigner,
    pub now: u64,
    pub program: Value,
    pub manifest: Value,
    pub release: Event,
    pub listing: Event,
    pub checkpoint: Event,
    pub evaluation: Event,
}

impl Fixture {
    pub fn new(now: u64) -> Self {
        let publisher = signer("11");
        let evaluator = signer("22");
        let package = format!("{}:demo", publisher.pubkey());
        let component = "explain-error";
        let qualified = format!("{package}/{component}");
        let mut program: Value = serde_json::from_str(include_str!(
            "../../../plugin-explain-error/programs/explain-error.json"
        ))
        .unwrap();
        program["definition"]["id"] = json!(qualified);
        program["definition"]["steps"][0]["target"]["id"] = json!(qualified);
        let program_bytes = serde_json::to_vec(&program).unwrap();
        let program_pin = digest_bytes(
            &serde_json::to_vec(std::str::from_utf8(&program_bytes).unwrap()).unwrap(),
        );
        let record = json!({"v":1,"slug":"demo","name":"Demo","summary":"Fixture program","version":"1.0.0","publisher":publisher.pubkey(),"provenance":"fixture","program":{"name":component,"digest":program_pin.trim_start_matches("sha256:")}});
        let record_bytes = serde_json::to_vec(&record).unwrap();
        let manifest = manifest_value(
            &package,
            component,
            "program",
            art(&program_bytes, None),
            vec![
                ("package.json", art(&record_bytes, None)),
                ("programs/explain-error.json", art(&program_bytes, None)),
            ],
        );
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let release = sign(
            &publisher,
            now - 10,
            ext::RELEASE_KIND,
            "release",
            None,
            json!({"v":1,"requires":[],"type":"release","package":package,"version":"1.0.0","manifest":art(&manifest_bytes,Some("openagents.package.v1")),"fee_msat":2000,"payout":"publisher@example.com"}),
        );
        let listing = sign(
            &publisher,
            now - 9,
            ext::LISTING_KIND,
            "listing",
            Some("demo"),
            json!({"v":1,"requires":[],"type":"listing","package":package,"state":"published","release":pointer(&release),"title":"Demo error explanation","description":"A bounded public fixture."}),
        );
        let checkpoint = sign(
            &publisher,
            now - 8,
            ext::CHECKPOINT_KIND,
            "checkpoint",
            Some("demo"),
            json!({"v":1,"requires":[],"type":"checkpoint","package":package,"revision":1,"as_of":now-8,"valid_until":now+100,"revocations":[]}),
        );
        let mut source = Memory::default();
        for bytes in [&program_bytes, &record_bytes, &manifest_bytes] {
            source.artifacts.insert(digest_bytes(bytes), bytes.clone());
        }
        source.heads.insert(
            (ext::LISTING_KIND, publisher.pubkey().into(), "demo".into()),
            vec![listing.clone()],
        );
        source.heads.insert(
            (
                ext::CHECKPOINT_KIND,
                publisher.pubkey().into(),
                "demo".into(),
            ),
            vec![checkpoint.clone()],
        );
        for event in [&release, &listing, &checkpoint] {
            source.events.insert(event.id.clone(), event.clone());
        }
        let mut store = |bytes: Vec<u8>, schema: Option<&str>| {
            let reference = art(&bytes, schema);
            source.artifacts.insert(digest_bytes(&bytes), bytes);
            reference
        };
        let prompt = store(b"Explain an error.".to_vec(), Some(eval_ext::CASE_SCHEMA));
        let grader = store(b"Check its citation.".to_vec(), Some(eval_ext::CASE_SCHEMA));
        let cases_bytes=eval_ext::case_manifest(&[json!({"id":"error","kind":"should-fire","runs":3,"prompt":prompt,"config":null,"graders":[{"name":"correct","artifact":grader}],"fixtures":[]})]).unwrap();
        let cases = store(cases_bytes.clone(), Some(eval_ext::CASE_SCHEMA));
        let generic = store(b"{}".to_vec(), None);
        let gate = store(b"{\"gate\":\"ext-eval-v2\"}".to_vec(), None);
        let suite_bytes=jcs(&json!({"v":eval_ext::SUITE_SCHEMA,"requires":[],"id":format!("{}:tests/suite",evaluator.pubkey()),"purpose":"operation","workload":generic,"cases":cases,"partition":generic,"labels":generic,"metrics":generic,"acceptance":{"id":format!("{}:gym/ext-eval-v2",evaluator.pubkey()),"artifact":gate},"environment":generic})).unwrap();
        let suite_ref = store(suite_bytes.clone(), Some(eval_ext::SUITE_SCHEMA));
        let suite_manifest = manifest_value(
            &format!("{}:tests", evaluator.pubkey()),
            "suite",
            eval_ext::COMPONENT_KIND,
            suite_ref.clone(),
            vec![
                ("suite.json", suite_ref.clone()),
                ("cases.json", cases.clone()),
                ("error/prompt.md", prompt.clone()),
                ("error/graders/correct.md", grader.clone()),
            ],
        );
        let suite_manifest_ref = store(
            serde_json::to_vec(&suite_manifest).unwrap(),
            Some("openagents.package.v1"),
        );
        let suite_release = sign(
            &evaluator,
            now - 20,
            ext::RELEASE_KIND,
            "release",
            None,
            json!({"v":1,"requires":[],"type":"release","package":format!("{}:tests",evaluator.pubkey()),"version":"1.0.0","manifest":suite_manifest_ref}),
        );
        let definition = json!({"id":qualified,"artifact":art(&record_bytes,Some("openagents.coder-package.v1")),"event":pointer(&release)});
        let mut lock_definition = definition.clone();
        lock_definition.as_object_mut().unwrap().remove("event");
        let lock=store(serde_json::to_vec(&json!({"v":"openagents.ext-eval-lock.v1","requires":[],"arm":"subject","definition":lock_definition,"package":{"publisher":publisher.pubkey(),"slug":"demo"},"programs":[{"slug":component,"digest":digest_bytes(&program_bytes),"size":program_bytes.len()}],"skills":[],"agent":{"digest":digest_bytes(b"agent"),"size":5,"questions":[]}})).unwrap(),Some("openagents.ext-eval-lock.v1"));
        let configuration = generic.clone();
        let mut published_suite = suite_ref;
        published_suite["event"] = pointer(&suite_release);
        let counts = json!({"planned":1,"attempted":3,"completed":3,"refused":0,"failed":0,"cancelled":0,"unknown":0,"excluded":0});
        let report = json!({"v":"openagents.eval-report.v1","requires":[],"suite":published_suite,"partition":generic,"subject":{"definition":definition,"lock":lock,"configuration":configuration},"baseline":{"definition":{"id":format!("{}:baseline/coder",evaluator.pubkey()),"artifact":generic},"lock":generic,"configuration":generic},"evaluator":evaluator.pubkey(),"started_at":now-7,"ended_at":now-6,"runs":generic,"coverage":{"subject":counts,"baseline":counts},"measurements":[],"verdict":"pass","limitations":generic,"meta":{"ext_eval":{"v":eval_ext::PROFILE_SCHEMA,"gate":gate["digest"],"cases":[{"id":"error","kind":"should-fire"}],"headline":{"subject_passed":1,"baseline_passed":0,"total":1},"requester":null}}});
        let parts = eval_ext::publication(&report.to_string(), None).unwrap();
        let evaluation = evaluator.sign(now - 5, parts.kind, parts.tags, parts.content);
        source
            .events
            .insert(suite_release.id.clone(), suite_release);
        source
            .events
            .insert(evaluation.id.clone(), evaluation.clone());
        let review = Review {
            reviewer: "fixture operator".into(),
            reviewed_at: now - 4,
            valid_until: now + 100,
            event: release.id.clone(),
            digest: digest_bytes(&manifest_bytes),
            operation: component.into(),
            evaluations: vec![evaluation.id.clone()],
            publisher_fee_msat: Some(2000),
            data_requirements: vec!["explicit error request".into()],
            recipients: vec!["selected invocation provider".into()],
            limitations: vec!["One fixture distribution; no live independent delivery.".into()],
        };
        let catalog = Catalog {
            schema: SCHEMA.into(),
            curator: "fixture curator".into(),
            max_age_seconds: 3600,
            skew_seconds: 0,
            items: vec![Item {
                id: package,
                kind: "extension".into(),
                event: release.id.clone(),
                operation: component.into(),
                digest: digest_bytes(&manifest_bytes),
                evaluations: vec![evaluation.id.clone()],
                review: Some(review),
                reputation: None,
            }],
        };
        Self {
            catalog,
            source,
            publisher,
            now,
            program,
            manifest,
            release,
            listing,
            checkpoint,
            evaluation,
        }
    }
    pub fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.catalog).unwrap()
    }
    pub fn set_head(&mut self, event: Event) {
        let slug = event.tag_values("d").next().unwrap().to_owned();
        self.source.events.insert(event.id.clone(), event.clone());
        self.source
            .heads
            .insert((event.kind, event.pubkey.clone(), slug), vec![event]);
    }
    pub fn hidden(&self) -> Event {
        sign(
            &self.publisher,
            self.now - 1,
            ext::LISTING_KIND,
            "listing",
            Some("demo"),
            json!({"v":1,"requires":[],"type":"listing","package":self.catalog.items[0].id,"state":"hidden","release":null}),
        )
    }
}
