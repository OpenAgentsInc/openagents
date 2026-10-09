//! The GCE adapter packaged with the owners, over the in-memory GCE.

use crate::gce::{Selected, janitor, reconcile, retained};
use crate::*;
use coder_working_computer::gce::fake::{FakeCompute, paths};
use coder_working_computer::gce::{GceConfig, GceImage, GceProvider, instance_name};
use coder_working_computer::provider::{Outcome, Provider};
use coder_working_computer::{Bounds, Computer, CreateAttempt, Fact, Principal, Spec};

fn config() -> GceConfig {
    GceConfig {
        project: "oa-test".into(),
        zone: "us-central1-a".into(),
        machine: "c3-standard-8".into(),
        disk_gb: 50,
        base: GceImage {
            project: "oa-test".into(),
            name: "oa-coder-host-1".into(),
            id: "42".into(),
        },
    }
}
fn provider(dir: &Path) -> GceProvider<FakeCompute> {
    GceProvider {
        compute: FakeCompute::new(dir.join("gce"), "oa-test", &config().base),
        config: config(),
        credentials: Default::default(),
        fresh: None,
        paths: paths(),
        ready_attempts: 1,
        ready_pause: Duration::ZERO,
    }
}
fn builder(id: &str) -> Computer {
    let spec = Spec {
        id: id.into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "user-1".into(),
        },
        chat: "job-1".into(),
        project: coder_environment::ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        source: coder_environment::SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        },
        base: None,
        size: "c3-standard-8".into(),
        credential_names: Default::default(),
        services: vec![],
        bounds: Bounds {
            idle_ms: 3_600_000,
            observed_extension_ms: 0,
            absolute_ms: 3_600_000,
        },
    };
    let mut c = Computer::for_build(spec, "env-1", "build-1", 0).unwrap();
    c.provider = coder_environment::Provider::Gce;
    c
}

#[tokio::test]
async fn selected_dispatches_to_the_gce_adapter() {
    let dir = tempfile::tempdir().unwrap();
    let p: Selected<FakeCompute> = Selected::Gce(provider(dir.path()));
    assert_eq!(p.kind(), coder_environment::Provider::Gce);
    assert!(p.admits_base(&config().base.pin()).is_ok());
    let c = builder("builder-1");
    let Outcome::Done { value: r } = p.create(&c, "op-1").await else {
        panic!("create");
    };
    assert_eq!(r, instance_name("op-1"));
}

#[tokio::test]
async fn the_janitor_sweeps_orphans_and_retains_usage() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::under(dir.path().join("cloud-operator"));
    let p = Arc::new(provider(dir.path()));
    // A computer whose create intent is retained but whose reply was not
    // recorded yet: its instance is held, not swept.
    let mut held = builder("builder-held");
    held.creates.push(CreateAttempt {
        operation: "op-held".into(),
        fact: Fact::Requested {
            operation: None,
            at_ms: 1,
        },
        resource: None,
        deletion: None,
    });
    coder_working_computer::store::Store::under(layout.computers())
        .create(&held)
        .unwrap();
    let held_instance = match p.create(&held, "op-held").await {
        Outcome::Done { value } => value,
        o => panic!("{o:?}"),
    };
    let orphan = match p.create(&builder("builder-gone"), "op-orphan").await {
        Outcome::Done { value } => value,
        o => panic!("{o:?}"),
    };
    assert!(
        retained(&layout)
            .unwrap()
            .instances
            .contains(&held_instance)
    );
    let report = reconcile(&p, &layout).await.unwrap();
    assert_eq!(report.orphans(), vec![orphan.as_str()]);
    assert!(p.compute.instance(&orphan).is_none());
    assert!(p.compute.instance(&held_instance).is_some());
    let doc: serde_json::Value =
        serde_json::from_slice(&fs::read(gce::last_report(&layout).unwrap()).unwrap()).unwrap();
    assert_eq!(doc["report"]["disk_gb"], 100);
    assert_eq!(doc["swept"][0]["instance"], orphan.as_str());

    // Packaged: the janitor runs on a recovery visit, then waits its turn.
    let state = layout.state().to_path_buf();
    let fake = Arc::new(coder_working_computer::provider::fake::FakeProvider::new(
        Default::default(),
        true,
    ));
    let owners = Owners::open(
        &state,
        Providers {
            setup: fake.clone(),
            build: fake.clone(),
            verify: fake,
        },
        Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
    )
    .unwrap()
    .with_janitor(Some(janitor(p.clone(), Duration::from_secs(3600))));
    let first = owners.recover(1).await;
    assert!(first.janitor.is_some(), "{first:?}");
    assert!(first.errors.is_empty(), "{first:?}");
    assert_eq!(owners.recover(2).await.janitor, None);
}
