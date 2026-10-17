use lab_jobs::*;
use lab_kit::{AssetClass, AssetEntry, JobSpec, LabKind, Requirements, Version, echo::Echo};
use serde_json::json;
use std::collections::BTreeSet;

fn version() -> Version {
    Version {
        document: "echo-test".into(),
        number: 1,
        parent: None,
        author: "test".into(),
        summary: "Echo".into(),
        created_unix_ms: 1,
        schema: "echo.v1".into(),
        content: json!({"text":"test"}),
        content_sha256: "a".repeat(64),
        report: Default::default(),
        assets: vec![],
    }
}
fn spec(v: &Version) -> JobSpec {
    JobSpec {
        kind: "echo".into(),
        job_type: "shout".into(),
        version: v.reference(),
        params: json!({}),
        requirements: Default::default(),
    }
}
fn worker(id: &str, placement: Placement) -> Capabilities {
    Capabilities {
        worker: id.into(),
        placement,
        cpu_millis: 2000,
        memory_bytes: 512 * 1024 * 1024,
        gpu: false,
        tools: BTreeSet::new(),
        jobs: ["echo/shout".into()].into(),
        local_assets: BTreeSet::new(),
        max_seconds: 3600,
    }
}

#[test]
fn private_input_cannot_be_downgraded_by_the_request() {
    let mut v = version();
    v.assets.push(AssetEntry {
        location: "private/input".into(),
        sha256: "b".repeat(64),
        class: AssetClass::PrivateLocal,
    });
    let r = EffectiveRequirements::for_job(&spec(&v), &v, &Echo::manifest()).unwrap();
    assert!(r.private_local && r.no_training);
    let cloud = worker("cloud", Placement::Cloud);
    let mut enrolled = worker("enrolled", Placement::Enrolled);
    assert!(!r.eligible(&cloud));
    assert!(!r.eligible(&enrolled));
    enrolled.local_assets.insert("b".repeat(64));
    assert!(r.eligible(&enrolled));
    assert_eq!(
        select(&r, &[cloud, enrolled]),
        Decision::Assigned {
            worker: "enrolled".into(),
            placement: Placement::Enrolled
        }
    );
}

#[test]
fn manifest_limits_cannot_be_weakened_and_inventory_order_is_irrelevant() {
    let v = version();
    let mut m = Echo::manifest();
    m.job_requirements.insert(
        "shout".into(),
        Requirements {
            cpu_millis: 2000,
            memory_bytes: 256 * 1024 * 1024,
            gpu: true,
            tools: vec!["echo".into()],
            max_seconds: 30,
            ..Default::default()
        },
    );
    let r = EffectiveRequirements::for_job(&spec(&v), &v, &m).unwrap();
    assert_eq!(r.max_seconds, 30);
    let mut a = worker("a", Placement::Server);
    let mut z = worker("z", Placement::Cloud);
    assert!(!r.eligible(&a));
    for w in [&mut a, &mut z] {
        w.gpu = true;
        w.tools.insert("echo".into());
    }
    assert_eq!(select(&r, &[z.clone(), a.clone()]), select(&r, &[a, z]));
    let mut too_long = spec(&v);
    too_long.requirements.max_seconds = 31;
    assert!(EffectiveRequirements::for_job(&too_long, &v, &m).is_err());
}

#[test]
fn missing_capacity_keeps_work_queued_with_a_reason() {
    let v = version();
    let mut s = spec(&v);
    s.requirements.memory_bytes = 1024 * 1024 * 1024;
    let r = EffectiveRequirements::for_job(&s, &v, &Echo::manifest()).unwrap();
    assert!(
        matches!(select(&r,&[worker("small",Placement::Server)]),Decision::Queued{message} if !message.is_empty())
    );
    assert!(!r.eligible(&worker("other", Placement::Cloud)));
}

#[test]
fn no_training_is_inherited_without_requiring_local_placement() {
    let mut v = version();
    v.assets.push(AssetEntry {
        location: "shared/render".into(),
        sha256: "b".repeat(64),
        class: AssetClass::NoTraining,
    });
    let r = EffectiveRequirements::for_job(&spec(&v), &v, &Echo::manifest()).unwrap();
    assert!(r.no_training && !r.private_local);
    assert!(r.eligible(&worker("cloud", Placement::Cloud)));
}
