use nostr::{
    contracts::{self, RefusalCode},
    workspace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn artifact(value: &Value) -> Value {
    let bytes = contracts::jcs(value).unwrap();
    let mut reference = json!({"digest":contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(v) = value.get("v") {
        reference["schema"] = v.clone();
    }
    reference
}
fn binding() -> Value {
    json!({"v":"openagents.workspace-binding.v1","requires":[],"workspace":"11".repeat(32),"owner":"22".repeat(32),"authority":"22".repeat(32),"generation":0,"revision":0,"environment":null,"projects":[],"roots":[],"policy":artifact(&json!({"v":"policy.v1"})),"authorization":artifact(&json!({"v":"authorization.v1"})),"created_at":1})
}
#[test]
fn strict_workspace_versions_features_namespaces_and_nested_reference_alternatives() {
    let good = binding();
    workspace::validate(&good).unwrap();
    let mut bad = good.clone();
    bad["v"] = json!("openagents.workspace-binding.v2");
    assert_eq!(
        workspace::validate(&bad).unwrap_err().code,
        RefusalCode::UnsupportedVersion
    );
    bad = good.clone();
    bad["requires"] = json!(["future"]);
    assert_eq!(
        workspace::validate(&bad).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    bad = good.clone();
    bad["future"] = json!(false);
    assert_eq!(
        workspace::validate(&bad).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    bad = good;
    bad["roots"] = json!([{"authority":"33".repeat(32),"workspace":"11".repeat(32),"resource":"44".repeat(32)}]);
    assert!(workspace::validate(&bad).is_err());
}
#[test]
fn failed_schema_alternatives_do_not_poison_later_reference_checks() {
    let schema = json!({"$defs":{"text":{"type":"string"},"object":{"type":"object"}},"type":"object","properties":{"a":{"anyOf":[{"$ref":"#/$defs/text"},{"$ref":"#/$defs/object"}]},"b":{"$ref":"#/$defs/text"}}});
    let bytes = contracts::jcs(&schema).unwrap();
    let digest = contracts::digest_bytes(&bytes);
    let closure = contracts::prepare_closure(&BTreeMap::from([(digest.clone(), bytes)])).unwrap();
    contracts::validate_instance(&closure, &digest, &json!({"a":{},"b":"valid"})).unwrap();
    assert!(contracts::validate_instance(&closure, &digest, &json!({"a":{},"b":false})).is_err());
}
#[test]
fn worktree_and_change_results_require_effect_confirmation() {
    let resource =
        json!({"authority":"22".repeat(32),"workspace":"11".repeat(32),"resource":"44".repeat(32)});
    let mut repository = resource.clone();
    repository["resource"] = json!("55".repeat(32));
    let r = artifact(&json!({"v":"openagents.execution-receipt.v1"}));
    let mut worktree = json!({"v":"openagents.worktree-binding.v1","requires":[],"resource":resource,"repository":repository,"binding":artifact(&binding()),"creation":null,"base":artifact(&json!({"v":"openagents.observation.v1"})),"owners":[],"lifecycle":"ready","policy":artifact(&json!({"v":"policy.v1"})),"removal":null});
    assert!(workspace::validate(&worktree).is_err());
    worktree["creation"] = r.clone();
    workspace::validate(&worktree).unwrap();
    worktree["lifecycle"] = json!("removed");
    assert!(workspace::validate(&worktree).is_err());
    worktree["removal"] = r;
    workspace::validate(&worktree).unwrap();
}
#[test]
fn ranges_bind_exact_snapshots_and_utf8_boundaries() {
    let bytes = "é\n".as_bytes();
    let content = json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":"text/plain"});
    let definition = json!({"id":format!("{}:workspace/git-document","22".repeat(32)),"artifact":artifact(&json!({"v":"definition.v1"}))});
    let observation = json!({"v":"openagents.observation.v1","requires":[],"resource":{"scope":"11".repeat(32),"id":"44".repeat(32)},"adapter":definition,"captured_at":1,"provider_revision":"revision","content":content,"consistency":"conditional","valid_until":null});
    let snapshot = json!({"v":"openagents.document-snapshot.v1","requires":[],"resource":{"authority":"22".repeat(32),"workspace":"11".repeat(32),"resource":"44".repeat(32)},"binding":artifact(&binding()),"observation":artifact(&observation),"content":content,"encoding":"openagents.git-blob-utf8.v1","derivation":null,"coverage":"complete","editable":true});
    workspace::verify_document(&snapshot, &observation, Some(bytes)).unwrap();
    let mut range = json!({"snapshot":artifact(&snapshot),"unit":"utf8_byte","start":0,"end":2});
    assert_eq!(
        workspace::verify_range(&range, &snapshot, bytes).unwrap(),
        (0, 2)
    );
    range["start"] = json!(1);
    assert!(workspace::verify_range(&range, &snapshot, bytes).is_err());
    range["start"] = json!(0);
    let mut another = snapshot.clone();
    another["editable"] = json!(false);
    assert!(workspace::verify_range(&range, &another, bytes).is_err());
    let mut derived = snapshot;
    derived["derivation"] = artifact(&json!({"v":"openagents.evidence.v1"}));
    assert!(workspace::validate(&derived).is_err());
}
#[test]
fn resource_graphs_refuse_missing_parents_cycles_and_aliases() {
    let mut binding = binding();
    let root =
        json!({"authority":"22".repeat(32),"workspace":"11".repeat(32),"resource":"01".repeat(32)});
    binding["roots"] = json!([root]);
    let definition = json!({"id":format!("{}:workspace/git-document","22".repeat(32)),"artifact":artifact(&json!({"v":"definition.v1"}))});
    let descriptor = |r: Value, parent: Value| json!({"v":"openagents.workspace-resource.v1","requires":[],"ref":r,"binding":artifact(&binding),"kind":"collection","parent":parent,"name":"Same name","adapter":definition,"operations":[],"observation":artifact(&json!({"v":"openagents.observation.v1"})),"lifecycle":"available"});
    let mut child = root.clone();
    child["resource"] = json!("02".repeat(32));
    let mut other = root.clone();
    other["resource"] = json!("03".repeat(32));
    let root_row = descriptor(root.clone(), Value::Null);
    let child_row = descriptor(child.clone(), root.clone());
    workspace::verify_resources(&binding, &[root_row.clone(), child_row.clone()]).unwrap();
    assert!(
        workspace::verify_resources(
            &binding,
            &[root_row.clone(), descriptor(child.clone(), other.clone())]
        )
        .is_err()
    );
    assert!(
        workspace::verify_resources(
            &binding,
            &[
                root_row.clone(),
                descriptor(child.clone(), other.clone()),
                descriptor(other, child)
            ]
        )
        .is_err()
    );
    assert!(
        workspace::verify_resources(&binding, &[root_row.clone(), child_row.clone(), child_row])
            .is_err()
    );
}
