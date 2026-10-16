use env_host::{
    gce::{Config, GceEnvironments},
    provider::{Cost, Create, Lookup, Provider, Teardown},
};
use oa_boat::gce::{EnvironmentCompute, GceError};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tenancy::money::{CreditKind, Ledger, Mutation, Operation};
#[derive(Default)]
struct State {
    creates: usize,
    operations: Vec<Value>,
    instances: BTreeMap<String, Value>,
    lost_reply: bool,
    hide_operations: bool,
    image_changed: bool,
    disk_changed: bool,
}
#[derive(Clone, Default)]
struct Compute(Arc<Mutex<State>>);
impl EnvironmentCompute for Compute {
    async fn create_environment(
        &self,
        zone: &str,
        request: &str,
        body: Value,
    ) -> Result<Value, GceError> {
        let mut s = self.0.lock().unwrap();
        s.creates += 1;
        let name = body["name"].as_str().unwrap();
        let id = (1000 + s.creates).to_string();
        let mut actual = body.clone();
        actual["id"] = json!(id);
        actual["zone"] = json!(format!("projects/fixture/zones/{zone}"));
        actual["status"] = json!("RUNNING");
        actual["disks"] = json!([{"boot":true,"autoDelete":true,"source":format!("projects/fixture/zones/{zone}/disks/{name}")}]);
        let op = json!({"name":format!("insert-{}",s.creates),"operationType":"insert","clientOperationId":request,"targetId":id,"targetLink":format!("projects/fixture/zones/{zone}/instances/{name}"),"status":"DONE"});
        s.instances.insert(name.into(), actual);
        s.operations.push(op.clone());
        if s.lost_reply {
            Err(GceError::new(0, "transport", "reply lost"))
        } else {
            Ok(op)
        }
    }
    async fn environment_instance(&self, _: &str, name: &str) -> Result<Option<Value>, GceError> {
        Ok(self.0.lock().unwrap().instances.get(name).cloned())
    }
    async fn environment_image(&self, _: &str) -> Result<Option<Value>, GceError> {
        Ok(Some(
            json!({"id":if self.0.lock().unwrap().image_changed{"888"}else{"777"},"status":"READY"}),
        ))
    }
    async fn environment_disk(&self, _: &str, _: &str) -> Result<Option<Value>, GceError> {
        Ok(Some(
            json!({"sourceImageId":if self.0.lock().unwrap().disk_changed{"888"}else{"777"},"sizeGb":"10"}),
        ))
    }
    async fn environment_operations(
        &self,
        _: &str,
        field: &str,
        value: &str,
    ) -> Result<Vec<Value>, GceError> {
        let s = self.0.lock().unwrap();
        if s.hide_operations {
            return Ok(vec![]);
        }
        Ok(s.operations
            .iter()
            .filter(|op| op[field] == value)
            .cloned()
            .collect())
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    config: Config,
    compute: Compute,
}
impl Fixture {
    fn new(funds: u64) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("money.jsonl");
        let mut money = Ledger::open(&path).unwrap();
        money
            .apply(Mutation {
                workspace: "provider".into(),
                source: "create".into(),
                audit: "fixture".into(),
                operation: Operation::Create {
                    currency: "USD".into(),
                    spend_limit: funds,
                    topups_allowed: false,
                },
            })
            .unwrap();
        if funds > 0 {
            money
                .apply(Mutation {
                    workspace: "provider".into(),
                    source: "fund".into(),
                    audit: "fixture".into(),
                    operation: Operation::Credit {
                        amount: funds,
                        credit_kind: CreditKind::Grant,
                    },
                })
                .unwrap();
        }
        drop(money);
        let config = Config {
            root: temp.path().join("provider"),
            project: "fixture".into(),
            zone: "us-central1-a".into(),
            machine: "e2-standard-2".into(),
            image: "projects/fixture/global/images/pinned".into(),
            image_id: "777".into(),
            network: "projects/fixture/regions/us-central1/subnetworks/private".into(),
            disk_gb: 10,
            lifetime_seconds: 300,
            funding_ledger: path,
            funding_workspace: "provider".into(),
            funding_hold_microunits: 100,
            adopt_instances: BTreeSet::from(["external".into()]),
        };
        Self {
            _temp: temp,
            config,
            compute: Compute::default(),
        }
    }
    fn provider(&self) -> GceEnvironments<Compute> {
        GceEnvironments::open(self.config.clone(), self.compute.clone()).unwrap()
    }
    fn deletion(&self, id: &str, name: &str) {
        self.compute.0.lock().unwrap().operations.push(json!({"name":"delete-original","operationType":"compute.instances.deferredDelete","targetId":id,"targetLink":format!("projects/fixture/zones/us-central1-a/instances/{name}"),"status":"DONE"}));
    }
}
#[test]
fn funded_original_create_recovers_without_another_paid_call_and_retains_unknown_cost() {
    let fx = Fixture::new(100);
    fx.compute.0.lock().unwrap().lost_reply = true;
    let key = "11".repeat(32);
    let mut provider = fx.provider();
    assert!(matches!(
        provider.create("fixture", &key),
        Create::Unknown(_)
    ));
    drop(provider);
    let mut restarted = fx.provider();
    let Lookup::Found(resource) = restarted.lookup("fixture", &key) else {
        panic!("original lookup");
    };
    assert_eq!(fx.compute.0.lock().unwrap().creates, 1);
    let Create::Created(same, Cost::Unknown) = restarted.create("fixture", &key) else {
        panic!("original retry");
    };
    assert_eq!(same, resource);
    assert_eq!(fx.compute.0.lock().unwrap().creates, 1);
    let bytes = nostr::contracts::jcs(&restarted.profile()["component"]).unwrap();
    assert_eq!(
        restarted
            .materialize(&resource, &[("image".into(), bytes)])
            .unwrap()
            .len(),
        1
    );
    fx.compute.0.lock().unwrap().disk_changed = true;
    assert!(restarted.materialize(&resource, &[]).is_err());
    fx.compute.0.lock().unwrap().disk_changed = false;
    assert_eq!(restarted.collect(&resource).unwrap()[0].0, "placement.json");
    assert!(matches!(restarted.destroy(&resource), Teardown::Unknown(_)));
    let name = resource.resource.rsplit('/').next().unwrap();
    fx.compute.0.lock().unwrap().instances.remove(name);
    assert!(
        matches!(
            restarted.reconcile_teardown(&resource, "destroy"),
            Teardown::Unknown(_)
        ),
        "missing VM is not deletion evidence"
    );
    fx.deletion("9999", name);
    assert!(
        matches!(restarted.destroy(&resource), Teardown::Unknown(_)),
        "wrong generation is not deletion evidence"
    );
    fx.deletion(&resource.generation, name);
    assert!(matches!(
        restarted.reconcile_teardown(&resource, "destroy"),
        Teardown::Confirmed(_, Cost::Unknown)
    ));
    drop(restarted);
    let money = Ledger::open(&fx.config.funding_ledger).unwrap();
    let balance = money.balance("provider").unwrap();
    assert_eq!(balance.reserved, 100);
    assert_eq!(balance.available, 0);
    assert_eq!(
        money.holds("provider")[0].1.phase,
        tenancy::money::Phase::Unknown
    );
}
#[test]
fn unfunded_or_exhausted_allowance_never_submits_another_vm() {
    let fx = Fixture::new(100);
    let mut p = fx.provider();
    assert!(matches!(
        p.create("fixture", &"22".repeat(32)),
        Create::Created(..)
    ));
    assert!(matches!(
        p.create("fixture", &"33".repeat(32)),
        Create::Refused(_)
    ));
    assert_eq!(fx.compute.0.lock().unwrap().creates, 1);
    drop(p);
    let empty = Fixture::new(0);
    let mut p = empty.provider();
    assert!(matches!(
        p.create("fixture", &"44".repeat(32)),
        Create::Refused(_)
    ));
    assert_eq!(empty.compute.0.lock().unwrap().creates, 0);
}
#[test]
fn a_new_store_empty_operation_list_and_changed_incarnation_never_prove_creation() {
    let fx = Fixture::new(100);
    let mut p = fx.provider();
    let key = "55".repeat(32);
    assert!(matches!(p.lookup("fixture", &key), Lookup::Unknown(_)));
    let Create::Created(r, _) = p.create("fixture", &key) else {
        panic!("create");
    };
    fx.compute.0.lock().unwrap().hide_operations = true;
    assert!(matches!(p.lookup("fixture", &key), Lookup::Unknown(_)));
    fx.compute.0.lock().unwrap().hide_operations = false;
    let name = r.resource.rsplit('/').next().unwrap();
    fx.compute
        .0
        .lock()
        .unwrap()
        .instances
        .get_mut(name)
        .unwrap()["id"] = json!("9999");
    assert!(matches!(p.lookup("fixture", &key), Lookup::Unknown(_)));
    assert!(p.materialize(&r, &[]).is_err());
    assert_eq!(fx.compute.0.lock().unwrap().creates, 1);
}
#[test]
fn external_adoption_and_original_expiry_proof_never_delete_a_replacement() {
    let fx = Fixture::new(100);
    let mut p = fx.provider();
    let Create::Created(r, _) = p.create("fixture", &"66".repeat(32)) else {
        panic!("create");
    };
    let name = r.resource.rsplit('/').next().unwrap();
    let mut replacement = fx.compute.0.lock().unwrap().instances[name].clone();
    replacement["id"] = json!("9999");
    fx.compute
        .0
        .lock()
        .unwrap()
        .instances
        .insert(name.into(), replacement.clone());
    fx.deletion(&r.generation, name);
    assert!(matches!(
        p.destroy(&r),
        Teardown::Confirmed(_, Cost::Unknown)
    ));
    assert_eq!(fx.compute.0.lock().unwrap().instances[name]["id"], "9999");
    replacement["name"] = json!("external");
    fx.compute
        .0
        .lock()
        .unwrap()
        .instances
        .insert("external".into(), replacement);
    let observed = p.observe_existing("external").unwrap();
    let resource: env_host::provider::Realized =
        serde_json::from_value(observed["resource"].clone()).unwrap();
    assert_eq!(
        p.adopt(
            "fixture",
            &resource.resource,
            &resource.generation,
            &resource.ownership
        )
        .unwrap(),
        resource
    );
    assert!(matches!(p.destroy(&resource), Teardown::Unknown(_)));
    assert!(matches!(p.detach(&resource), Teardown::Confirmed(..)));
    assert!(p.observe_existing("outside-allowlist").is_err());
    assert_eq!(
        fx.compute.0.lock().unwrap().instances["external"]["id"],
        "9999"
    );
}

#[test]
fn unsupported_cloud_bounds_refuse_before_funding_or_provisioning() {
    let fx = Fixture::new(100);
    let p = fx.provider();
    let now = env_host::now();
    let mut request = json!({"mode":"create","ttl_seconds":300,"not_after":now+300,"effects":{"process":true,"network":["environment-provider"]},"bounds":[{"bound":"spend_microunits","ceiling":0},{"bound":"concurrency","ceiling":1}]});
    p.admit_request(&request, now).unwrap();
    request["bounds"]
        .as_array_mut()
        .unwrap()
        .push(json!({"bound":"memory_bytes","ceiling":1}));
    assert!(p.admit_request(&request, now).is_err());
    assert_eq!(fx.compute.0.lock().unwrap().creates, 0);
}

#[test]
fn the_full_request_key_separates_downstream_operation_ids() {
    let fx = Fixture::new(200);
    let mut p = fx.provider();
    let prefix = "ab".repeat(16);
    let one = format!("{prefix}{}", "11".repeat(16));
    let two = format!("{prefix}{}", "22".repeat(16));
    assert!(matches!(p.create("fixture", &one), Create::Created(..)));
    assert!(matches!(p.create("fixture", &two), Create::Created(..)));
    let state = fx.compute.0.lock().unwrap();
    assert_eq!(state.creates, 2);
    assert_ne!(
        state.operations[0]["clientOperationId"],
        state.operations[1]["clientOperationId"]
    );
}
#[test]
fn a_replacement_funding_store_cannot_confirm_the_original_allocation() {
    let fx = Fixture::new(100);
    let mut p = fx.provider();
    let key = "77".repeat(32);
    let Create::Created(resource, _) = p.create("fixture", &key) else {
        panic!("create");
    };
    drop(p);
    std::fs::rename(
        &fx.config.funding_ledger,
        fx.config.root.join("original-money.jsonl"),
    )
    .unwrap();
    let mut replacement = Ledger::open(&fx.config.funding_ledger).unwrap();
    replacement
        .apply(Mutation {
            workspace: "provider".into(),
            source: "replacement".into(),
            audit: "fixture".into(),
            operation: Operation::Create {
                currency: "USD".into(),
                spend_limit: 100,
                topups_allowed: false,
            },
        })
        .unwrap();
    drop(replacement);
    let mut restarted = fx.provider();
    assert!(matches!(
        restarted.lookup("fixture", &key),
        Lookup::Unknown(_)
    ));
    assert!(restarted.check_current(&resource).is_err());
    assert!(matches!(
        restarted.create("fixture", &key),
        Create::Unknown(_)
    ));
    assert_eq!(fx.compute.0.lock().unwrap().creates, 1);
}
