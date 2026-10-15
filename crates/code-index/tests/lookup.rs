mod common;
use code_index::{
    lookup::*,
    recipes::{Condition, Context},
};
use std::collections::BTreeMap;

fn request(context: &Context) -> Request {
    Request {
        question: "register a kind".into(),
        operation: None,
        snapshot: context.snapshot.clone(),
        max_bytes: 16000,
        conditions: BTreeMap::new(),
        file_candidates: vec![],
        query_vector: None,
        recipe_vectors: BTreeMap::new(),
        decision: None,
    }
}

#[test]
fn lookup_handles_exclusion_missing_and_stale_queries() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let artifact = common::artifact(&context);
    let mut request = request(&context);
    let answer = query(&artifact, &request).unwrap();
    assert_eq!(answer.status, Status::Matched);
    assert!(answer.required_roles_complete);
    request.question = "register an inference model".into();
    assert_eq!(query(&artifact, &request).unwrap().status, Status::NoMatch);
    request.question = "unrelated foobar".into();
    assert_eq!(query(&artifact, &request).unwrap().status, Status::NoMatch);
    request.snapshot.commit = "changed".into();
    assert_eq!(query(&artifact, &request).unwrap().status, Status::Stale);
}

#[test]
fn budget_never_silently_cuts_required_companions_and_conditions() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let mut artifact = common::artifact(&context);
    let mut request = request(&context);
    artifact.recipes[0].conditions.push(Condition {
        id: "new-wire".into(),
        description: "Does this change the wire shape?".into(),
    });
    artifact.recipes[0].roles[0].when = Some("new-wire".into());
    assert_eq!(
        query(&artifact, &request).unwrap().status,
        Status::Ambiguous
    );
    request.conditions.insert("new-wire".into(), true);
    assert!(query(&artifact, &request).unwrap().required_roles_complete);
    request.max_bytes = 1024;
    artifact.recipes[0].roles[0].purpose = "Large required context ".repeat(1000);
    let answer = query(&artifact, &request).unwrap();
    assert!(!answer.required_roles_complete);
    assert_eq!(answer.status, Status::Ambiguous);
    assert!(serde_json::to_vec(&answer).unwrap().len() <= 1024);
}

#[test]
fn ties_and_supplied_selections_are_explicit() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let mut artifact = common::artifact(&context);
    let mut second = artifact.recipes[0].clone();
    second.id = "other".into();
    second.operation = "other-operation".into();
    artifact.recipes.push(second);
    let mut request = request(&context);
    let answer = query(&artifact, &request).unwrap();
    assert_eq!(answer.status, Status::Ambiguous);
    request.decision = Some(Decision {
        question_digest: code_index::digest(request.question.as_bytes()),
        artifact_digest: answer.artifact_digest,
        chosen: Some("other".into()),
    });
    assert_eq!(
        query(&artifact, &request).unwrap().candidates[0].id,
        "other"
    );
    request.decision.as_mut().unwrap().chosen = Some("invented".into());
    assert!(query(&artifact, &request).is_err());
}
