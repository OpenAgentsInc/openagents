//! Sponsored, finite GCE placement over the public Compute owner. Provider
//! funding is a protected money-ledger hold, never caller-declared funds.
use crate::{
    Result,
    provider::{Cost, Create, Lookup, Provider, Realized, Teardown},
};
use nostr::contracts::{digest_bytes, digest_value, jcs};
use oa_boat::gce::EnvironmentCompute;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use tenancy::money::{Ledger, Mutation, Operation, Price, Rate, Resource};
use workspace_host::artifact::{Artifacts, atomic, read, token};
const PREFIX: &str = "environment/gce-sponsored";
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub root: PathBuf,
    pub project: String,
    pub zone: String,
    pub machine: String,
    pub image: String,
    pub image_id: String,
    pub network: String,
    pub disk_gb: u64,
    pub lifetime_seconds: u64,
    pub funding_ledger: PathBuf,
    pub funding_workspace: String,
    pub funding_hold_microunits: u64,
    #[serde(default)]
    pub adopt_instances: std::collections::BTreeSet<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    uuid: String,
    generation: String,
    name: String,
    created_at: u64,
    termination_time: String,
    operation: Option<Value>,
    instance: Option<Value>,
    dispatched: bool,
    stopping: bool,
    deleted: Option<Value>,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Book {
    configuration: String,
    entries: BTreeMap<String, Entry>,
}
/// Run this synchronous provider on a blocking thread. The ledger is held
/// exclusively for this instance; every host must share that same authority.
pub struct GceEnvironments<G> {
    config: Config,
    fingerprint: String,
    store: Artifacts,
    compute: G,
    runtime: tokio::runtime::Runtime,
    money: Ledger,
}
fn numeric(v: &Value) -> Result<&str> {
    let s = v
        .as_str()
        .ok_or("identity_mismatch: provider incarnation missing")?;
    if s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) || s == "0" {
        return Err("identity_mismatch: provider incarnation differs".into());
    }
    Ok(s)
}
fn name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn uuid(identity: &str) -> String {
    let digest = digest_bytes(identity.as_bytes());
    let key = &digest[7..];
    format!(
        "{}-{}-4{}-8{}-{}",
        &key[..8],
        &key[8..12],
        &key[13..16],
        &key[17..20],
        &key[20..32]
    )
}
impl<G: EnvironmentCompute> GceEnvironments<G> {
    pub fn open(config: Config, compute: G) -> Result<Self> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(
                "cannot_enforce: open the synchronous cloud provider on a blocking thread".into(),
            );
        }
        if !name(&config.project)
            || !name(&config.zone)
            || !name(&config.machine)
            || config.image.split('/').count() != 5
            || config.image.contains("/family/")
            || !config.image_id.bytes().all(|b| b.is_ascii_digit())
            || config.image_id.is_empty()
            || config.image_id.len() > 20
            || !config.network.starts_with("projects/")
            || config.network.len() > 256
            || !(1..=100).contains(&config.disk_gb)
            || !(30..=3600).contains(&config.lifetime_seconds)
            || config.funding_hold_microunits == 0
            || !config.root.is_absolute()
            || !config.funding_ledger.is_absolute()
        {
            return Err("malformed: finite cloud provider configuration required".into());
        }
        let fingerprint = digest_value(&serde_json::to_value(&config).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let store = Artifacts::new(config.root.clone())?;
        // Opening an empty ledger never creates an account or credit. The operator
        // must provision this workspace through the existing funding authority.
        let money = Ledger::open(&config.funding_ledger)?;
        if money.balance(&config.funding_workspace)?.currency != "USD" {
            return Err("cannot_enforce: cloud funding requires a USD workspace".into());
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let provider = Self {
            config,
            fingerprint,
            store,
            compute,
            runtime,
            money,
        };
        provider.book()?;
        Ok(provider)
    }
    fn binding_id(&self) -> String {
        format!("{PREFIX}/{}", &self.fingerprint[7..])
    }
    fn book(&self) -> Result<Book> {
        let path = self.config.root.join("placements.json");
        if !path.exists() {
            return Ok(Book {
                configuration: self.fingerprint.clone(),
                ..Default::default()
            });
        }
        let b: Book = serde_json::from_slice(&read(&path, 1048576)?)
            .map_err(|_| "malformed: cloud placement book")?;
        if b.entries.iter().any(|(key, e)| {
            key.len() != 64
                || !key.bytes().all(|b| b.is_ascii_hexdigit())
                || e.generation.len() != 64
                || !name(&e.name)
        }) {
            return Err("malformed: original cloud identity book".into());
        }
        if b.configuration != self.fingerprint {
            return Err("identity_mismatch: cloud provider configuration changed".into());
        }
        Ok(b)
    }
    fn save(&self, b: &Book) -> Result<()> {
        let bytes = serde_json::to_vec(b).map_err(|e| e.to_string())?;
        if bytes.len() > 1048576 {
            return Err("limit_exceeded: cloud placement book".into());
        }
        atomic(&self.config.root.join("placements.json"), &bytes)
    }
    fn attempt(&self, key: &str) -> String {
        format!("env-gce:{}:{key}", &self.fingerprint[7..23])
    }
    fn fund(&mut self, key: &str) -> Result<()> {
        self.money.check_source(&self.config.funding_ledger)?;
        let duration = self.config.lifetime_seconds * 1000;
        let price = Price {
            version: self.fingerprint.clone(),
            currency: "USD".into(),
            model: "gce-sponsored".into(),
            capacity: self.config.machine.clone(),
            policy: self.fingerprint.clone(),
            rates: BTreeMap::from([(
                Resource::ComputeMilliseconds,
                Rate {
                    millionths: self.config.funding_hold_microunits,
                    per_units: duration,
                },
            )]),
        };
        let attempt = self.attempt(key);
        self.money.apply(Mutation {
            workspace: self.config.funding_workspace.clone(),
            source: format!("{attempt}:reserve"),
            audit: key.into(),
            operation: Operation::Reserve {
                attempt: attempt.clone(),
                request_digest: digest_value(
                    &json!({"configuration":self.fingerprint,"request":key}),
                )
                .map_err(|e| e.to_string())?,
                price,
                maximum_usage: BTreeMap::from([(Resource::ComputeMilliseconds, duration)]),
            },
        })?;
        self.money.apply(Mutation {
            workspace: self.config.funding_workspace.clone(),
            source: format!("{attempt}:unknown"),
            audit: key.into(),
            operation: Operation::Unknown { attempt },
        })?;
        Ok(())
    }
    pub fn profile(&self) -> Value {
        let component = json!({"v":"openagents.environment-native-gce-image.v1","requires":[],"configuration":self.fingerprint,"image":self.config.image,"image_id":self.config.image_id,"machine":self.config.machine,"disk_gb":self.config.disk_gb,"lifetime_seconds":self.config.lifetime_seconds});
        let bytes = jcs(&component).expect("fixed image descriptor");
        json!({"provider":self.binding_id(),"component":component,"artifact":workspace_host::artifact::reference(&bytes,"application/json",Some("openagents.environment-native-gce-image.v1"))})
    }
    fn resource(&self, key: &str, e: &Entry) -> Result<Realized> {
        let instance = e
            .instance
            .as_ref()
            .ok_or("unknown: original instance is not recorded")?;
        let id = numeric(&instance["id"])?;
        let resource = format!(
            "projects/{}/zones/{}/instances/{}",
            self.config.project, self.config.zone, e.name
        );
        let ownership=jcs(&json!({"v":"openagents.environment-ownership.v1","requires":[],"provider":self.binding_id(),"account":self.config.project,"resource":resource,"generation":id,"key":key,"observed_at":e.created_at,"meta":{"configuration":self.fingerprint,"request_uuid":e.uuid,"generation":e.generation,"funding_workspace":self.config.funding_workspace,"funding_attempt":self.attempt(key),"termination_time":e.termination_time}})).map_err(|e|e.to_string())?;
        Ok(Realized {
            resource,
            generation: id.into(),
            account: self.config.project.clone(),
            ownership,
        })
    }
    fn owned_instance(&self, key: &str, e: &Entry, instance: &Value) -> Result<()> {
        numeric(&instance["id"])?;
        if instance["name"] != e.name
            || instance["zone"]
                .as_str()
                .is_none_or(|z| z.rsplit('/').next() != Some(&self.config.zone))
            || instance["labels"]["oa-env-key-a"] != key[..32]
            || instance["labels"]["oa-env-key-b"] != key[32..]
            || instance["labels"]["oa-env-generation"] != e.generation[..32]
            || instance["machineType"]
                .as_str()
                .is_none_or(|m| m.rsplit('/').next() != Some(&self.config.machine))
            || instance["scheduling"]["terminationTime"]
                .as_str()
                .and_then(oa_boat::time::parse)
                != oa_boat::time::parse(&e.termination_time)
            || instance["scheduling"]["instanceTerminationAction"] != "DELETE"
            || instance["serviceAccounts"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
        {
            return Err(
                "identity_mismatch: actual cloud placement differs from its original intent".into(),
            );
        }
        if e.instance
            .as_ref()
            .is_some_and(|old| old["id"] != instance["id"])
        {
            return Err("identity_mismatch: cloud instance incarnation changed".into());
        }
        Ok(())
    }
    fn observe(&self, key: &str, b: &mut Book) -> Result<Option<Realized>> {
        let Some(e) = b.entries.get_mut(key) else {
            return Err("unknown: original cloud intent is not recorded".into());
        };
        if !e.dispatched {
            return Err("unknown: cloud placement was fenced before submission".into());
        }
        self.money.check_source(&self.config.funding_ledger)?;
        let hold = self
            .money
            .hold(&self.config.funding_workspace, &self.attempt(key))
            .ok_or("unknown: original funded allowance is not recorded")?;
        let fingerprint = digest_value(&json!({"configuration":self.fingerprint,"request":key}))
            .map_err(|e| e.to_string())?;
        if hold.price.version != self.fingerprint
            || hold.request_digest != fingerprint
            || hold.reserved != self.config.funding_hold_microunits
            || hold.phase == tenancy::money::Phase::Released
        {
            return Err("identity_mismatch: original funded allowance differs".into());
        }

        let operations = self
            .runtime
            .block_on(self.compute.environment_operations(
                &self.config.zone,
                "clientOperationId",
                &e.uuid,
            ))
            .map_err(|e| e.to_string())?;
        let mut matching = operations.iter().filter(|op| {
            op["clientOperationId"] == e.uuid
                && matches!(
                    op["operationType"].as_str(),
                    Some("insert" | "compute.instances.insert")
                )
                && op["targetLink"].as_str().is_some_and(|link| {
                    link.ends_with(&format!(
                        "projects/{}/zones/{}/instances/{}",
                        self.config.project, self.config.zone, e.name
                    ))
                })
        });
        let Some(op) = matching.next() else {
            return Err("unknown: original cloud create operation is not retained".into());
        };
        if matching.next().is_some() || op.get("error").is_some_and(|e| !e.is_null()) {
            return Err("unknown: original cloud operation is ambiguous or failed".into());
        }
        let id = numeric(&op["targetId"])?;
        e.operation = Some(op.clone());
        let instance = self
            .runtime
            .block_on(
                self.compute
                    .environment_instance(&self.config.zone, &e.name),
            )
            .map_err(|e| e.to_string())?
            .ok_or("unknown: missing instance is not proof of absent creation")?;
        self.owned_instance(key, e, &instance)?;
        if numeric(&instance["id"])? != id {
            return Err("identity_mismatch: operation and instance generations differ".into());
        }
        e.instance = Some(instance);
        self.save(b)?;
        Ok(Some(self.resource(key, &b.entries[key])?))
    }
    fn create_inner(&mut self, key: &str) -> Result<Realized> {
        let _guard = self.store.lock()?;
        let mut b = self.book()?;
        if b.entries.contains_key(key) {
            return self
                .observe(key, &mut b)?
                .ok_or("unknown: original create remains pending".into());
        }
        if b.entries.len() >= 64 || self.money.has_attempt(&self.attempt(key)) {
            return Err(
                "unknown: cloud retention limit or original funding hold requires recovery".into(),
            );
        }
        let image = self
            .runtime
            .block_on(self.compute.environment_image(&self.config.image))
            .map_err(|e| e.to_string())?
            .ok_or("unavailable: configured boot image is absent")?;
        if numeric(&image["id"])? != self.config.image_id || image["status"] != "READY" {
            return Err("identity_mismatch: boot image incarnation differs".into());
        }
        let termination_time =
            oa_boat::time::format((crate::now() + self.config.lifetime_seconds) as i64);
        let entry = Entry {
            uuid: uuid(&format!("{}\n{key}", self.fingerprint)),
            generation: token(),
            name: format!("oa-env-{}", &token()[..32]),
            created_at: crate::now(),
            termination_time,
            operation: None,
            instance: None,
            dispatched: false,
            stopping: false,
            deleted: None,
        };
        b.entries.insert(key.into(), entry);
        self.save(&b)?;
        self.fund(key)?;
        b.entries.get_mut(key).ok_or("entry")?.dispatched = true;
        self.save(&b)?;
        let e = &b.entries[key];
        let body = json!({"name":e.name,"machineType":format!("zones/{}/machineTypes/{}",self.config.zone,self.config.machine),"labels":{"oa-env-key-a":&key[..32],"oa-env-key-b":&key[32..],"oa-env-generation":&e.generation[..32]},"disks":[{"boot":true,"autoDelete":true,"initializeParams":{"sourceImage":self.config.image,"diskSizeGb":self.config.disk_gb}}],"networkInterfaces":[{"subnetwork":self.config.network,"accessConfigs":[]}],"serviceAccounts":[],"scheduling":{"automaticRestart":false,"terminationTime":e.termination_time,"instanceTerminationAction":"DELETE"}});
        let op = self
            .runtime
            .block_on(
                self.compute
                    .create_environment(&self.config.zone, &e.uuid, body),
            )
            .map_err(|e| format!("unknown: {e}"))?;
        b.entries.get_mut(key).ok_or("entry")?.operation = Some(op);
        self.save(&b)?;
        self.observe(key, &mut b)?
            .ok_or("unknown: original create remains pending".into())
    }
    fn exact<'a>(&self, resource: &Realized, b: &'a Book) -> Result<(&'a str, &'a Entry)> {
        for (key, e) in &b.entries {
            if e.instance.is_some() && self.resource(key, e)? == *resource {
                return Ok((key, e));
            }
        }
        Err("identity_mismatch: exact original cloud ownership required".into())
    }
    fn current(&self, resource: &Realized) -> Result<Value> {
        let b = self.book()?;
        let (key, e) = match self.exact(resource, &b) {
            Ok(found) => found,
            Err(_) => {
                let name = resource
                    .resource
                    .rsplit('/')
                    .next()
                    .ok_or("resource name")?;
                let (actual, observed) = self.external(name)?;
                if observed != *resource {
                    return Err("identity_mismatch: external cloud generation differs".into());
                }
                return Ok(actual);
            }
        };
        self.money.check_source(&self.config.funding_ledger)?;
        let hold = self
            .money
            .hold(&self.config.funding_workspace, &self.attempt(key))
            .ok_or("unknown: original funded allowance is not recorded")?;
        if hold.price.version != self.fingerprint
            || hold.phase == tenancy::money::Phase::Released
            || hold.reserved != self.config.funding_hold_microunits
        {
            return Err("identity_mismatch: original funded allowance differs".into());
        }
        let actual = self
            .runtime
            .block_on(
                self.compute
                    .environment_instance(&self.config.zone, &e.name),
            )
            .map_err(|e| e.to_string())?
            .ok_or("unknown: original instance is not observed")?;
        self.owned_instance(key, e, &actual)?;
        if oa_boat::time::parse(&e.termination_time).is_none_or(|t| t <= crate::now() as i64) {
            return Err("stale: original cloud lifetime ended".into());
        }
        if actual["status"] != "RUNNING" {
            return Err("unavailable: original instance is not running".into());
        }
        Ok(actual)
    }
    fn external(&self, instance_name: &str) -> Result<(Value, Realized)> {
        if !self.config.adopt_instances.contains(instance_name) {
            return Err(
                "not_admitted: external cloud instance is outside the operator allowlist".into(),
            );
        }
        let actual = self
            .runtime
            .block_on(
                self.compute
                    .environment_instance(&self.config.zone, instance_name),
            )
            .map_err(|e| e.to_string())?
            .ok_or("unavailable: external cloud instance is not observed")?;
        let id = numeric(&actual["id"])?.to_string();
        if actual["name"] != instance_name
            || actual["status"] != "RUNNING"
            || actual["machineType"]
                .as_str()
                .and_then(|m| m.rsplit('/').next())
                != Some(self.config.machine.as_str())
        {
            return Err("identity_mismatch: external cloud runtime differs".into());
        }
        let resource = format!(
            "projects/{}/zones/{}/instances/{instance_name}",
            self.config.project, self.config.zone
        );
        let key = digest_value(
            &json!({"configuration":self.fingerprint,"resource":resource,"generation":id}),
        )
        .map_err(|e| e.to_string())?[7..]
            .to_string();
        let ownership=jcs(&json!({"v":"openagents.environment-ownership.v1","requires":[],"provider":self.binding_id(),"account":self.config.project,"resource":resource,"generation":id,"key":key,"observed_at":0,"meta":{"configuration":self.fingerprint,"external":true}})).map_err(|e|e.to_string())?;
        Ok((
            actual,
            Realized {
                resource,
                generation: id.into(),
                account: self.config.project.clone(),
                ownership,
            },
        ))
    }
    pub fn observe_existing(&self, instance_name: &str) -> Result<Value> {
        let _g = self.store.lock()?;
        let (_, resource) = self.external(instance_name)?;
        Ok(json!({"resource":resource,"profile":self.profile()}))
    }
    fn cleanup(&self, resource: &Realized) -> Result<Vec<u8>> {
        let _g = self.store.lock()?;
        let mut b = self.book()?;
        let (key, e) = self.exact(resource, &b)?;
        let key = key.to_string();
        if let Some(receipt) = &e.deleted {
            return jcs(receipt).map_err(|e| e.to_string());
        }
        let name = e.name.clone();
        b.entries.get_mut(&key).ok_or("entry")?.stopping = true;
        self.save(&b)?;
        let ops = self
            .runtime
            .block_on(self.compute.environment_operations(
                &self.config.zone,
                "targetId",
                &resource.generation,
            ))
            .map_err(|e| e.to_string())?;
        let proof = ops
            .iter()
            .find(|op| {
                op["targetId"] == resource.generation
                    && op["operationType"] == "compute.instances.deferredDelete"
                    && op["status"] == "DONE"
                    && op.get("error").is_none_or(Value::is_null)
                    && op["targetLink"].as_str().is_some_and(|link| {
                        link.ends_with(&format!(
                            "projects/{}/zones/{}/instances/{name}",
                            self.config.project, self.config.zone
                        ))
                    })
            })
            .ok_or("unknown: original automatic deletion is not confirmed")?;
        let current = self
            .runtime
            .block_on(self.compute.environment_instance(&self.config.zone, &name))
            .map_err(|e| e.to_string())?;
        if current
            .as_ref()
            .is_some_and(|i| i["id"] == resource.generation)
        {
            return Err("unknown: original instance still exists after deletion operation".into());
        }
        let receipt = json!({"provider":self.binding_id(),"resource":resource.resource,"generation":resource.generation,"observed":"deleted","operation":proof});
        b.entries.get_mut(&key).ok_or("entry")?.deleted = Some(receipt.clone());
        self.save(&b)?;
        jcs(&receipt).map_err(|e| e.to_string())
    }
    pub fn funding_statement(&self) -> Result<Value> {
        serde_json::to_value(self.money.statement(&self.config.funding_workspace)?)
            .map_err(|e| e.to_string())
    }
}
impl<G: EnvironmentCompute> Provider for GceEnvironments<G> {
    fn binding(&self) -> String {
        self.binding_id()
    }
    fn admit_request(&self, request: &Value, now: u64) -> Result<()> {
        if request["effects"]["process"] != true
            || !request["effects"]["network"]
                .as_array()
                .is_some_and(|n| n.iter().any(|v| v == "environment-provider"))
        {
            return Err(
                "cannot_enforce: cloud process and provider-network effects must be admitted"
                    .into(),
            );
        }
        for b in request["bounds"].as_array().ok_or("request bounds")? {
            match b["bound"].as_str(){Some("spend_microunits") if b["ceiling"]==0=>{},Some("concurrency") if b["ceiling"].as_u64().is_some_and(|n|n>=1)=>{},_=>return Err("cannot_enforce: this cloud profile supports only caller no-spend and lease concurrency ceilings".into())}
        }
        if request["mode"] == "create"
            && (request["ttl_seconds"]
                .as_u64()
                .is_none_or(|ttl| ttl > self.config.lifetime_seconds)
                || request["not_after"].as_u64().is_none_or(|deadline| {
                    deadline < now.saturating_add(self.config.lifetime_seconds)
                }))
        {
            return Err(
                "cannot_enforce: cloud lifetime must fit the original environment window".into(),
            );
        }
        Ok(())
    }
    fn check_current(&mut self, resource: &Realized) -> Result<()> {
        let _g = self.store.lock()?;
        self.current(resource).map(|_| ())
    }
    fn create(&mut self, account: &str, key: &str) -> Create {
        if account != self.config.project
            || key.len() != 64
            || !key.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Create::Refused(
                "not_admitted: cloud account or create identity differs".into(),
            );
        }
        match self.create_inner(key) {
            Ok(r) => Create::Created(r, Cost::Unknown),
            Err(e) => {
                if self
                    .book()
                    .ok()
                    .and_then(|b| b.entries.get(key).map(|entry| !entry.dispatched))
                    == Some(true)
                {
                    Create::Refused(e)
                } else {
                    Create::Unknown(e)
                }
            }
        }
    }
    fn lookup(&mut self, account: &str, key: &str) -> Lookup {
        if account != self.config.project {
            return Lookup::Unknown("cloud account differs".into());
        }
        let result = (|| {
            let _g = self.store.lock()?;
            let mut b = self.book()?;
            self.observe(key, &mut b)
        })();
        match result {
            Ok(Some(r)) => Lookup::Found(r),
            _ => Lookup::Unknown("original cloud create remains unresolved".into()),
        }
    }
    fn adopt(
        &mut self,
        account: &str,
        resource: &str,
        generation: &str,
        ownership: &[u8],
    ) -> Result<Realized> {
        if account != self.config.project {
            return Err("not_admitted: cloud account differs".into());
        }
        let r = Realized {
            account: account.into(),
            resource: resource.into(),
            generation: generation.into(),
            ownership: ownership.into(),
        };
        let _g = self.store.lock()?;
        self.current(&r)?;
        Ok(r)
    }
    fn materialize(
        &mut self,
        resource: &Realized,
        components: &[(String, Vec<u8>)],
    ) -> Result<Vec<(String, String)>> {
        let _g = self.store.lock()?;
        let actual = self.current(resource)?;
        let disks = actual["disks"].as_array().ok_or("actual boot disk")?;
        if disks.len() != 1 || disks[0]["boot"] != true || disks[0]["autoDelete"] != true {
            return Err("identity_mismatch: cloud disk layout differs".into());
        }
        let disk = disks[0]["source"]
            .as_str()
            .and_then(|s| s.rsplit('/').next())
            .ok_or("actual boot disk")?;
        let disk = self
            .runtime
            .block_on(self.compute.environment_disk(&self.config.zone, disk))
            .map_err(|e| e.to_string())?
            .ok_or("unavailable: boot disk not observed")?;
        if disk["sourceImageId"] != self.config.image_id
            || disk["sizeGb"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .or(disk["sizeGb"].as_u64())
                != Some(self.config.disk_gb)
        {
            return Err("identity_mismatch: actual cloud boot image or disk size differs".into());
        }
        let descriptor = jcs(&self.profile()["component"]).map_err(|e| e.to_string())?;
        Ok(components
            .iter()
            .filter(|(_, b)| *b == descriptor)
            .map(|(id, b)| (id.clone(), digest_bytes(b)))
            .collect())
    }
    fn collect(&mut self, resource: &Realized) -> Result<Vec<(String, Vec<u8>)>> {
        let _g = self.store.lock()?;
        let b = self.book()?;
        let value = match self.exact(resource, &b) {
            Ok((key, e)) => {
                json!({"provider":self.binding_id(),"resource":resource.resource,"generation":resource.generation,"create_operation":e.operation,"image":self.profile()["component"],"funding_workspace":self.config.funding_workspace,"funding_attempt":self.attempt(key),"provider_cost":"unknown"})
            }
            Err(_) => {
                self.current(resource)?;
                json!({"provider":self.binding_id(),"resource":resource.resource,"generation":resource.generation,"adopted":true,"image":self.profile()["component"]})
            }
        };
        Ok(vec![(
            "placement.json".into(),
            jcs(&value).map_err(|e| e.to_string())?,
        )])
    }
    fn destroy(&mut self, r: &Realized) -> Teardown {
        match self.cleanup(r) {
            Ok(bytes) => Teardown::Confirmed(bytes, Cost::Unknown),
            Err(e) => Teardown::Unknown(e),
        }
    }
    fn reconcile_teardown(&mut self, r: &Realized, action: &str) -> Teardown {
        if action == "detach" {
            self.detach(r)
        } else {
            self.destroy(r)
        }
    }
    fn detach(&mut self, r: &Realized) -> Teardown {
        if r.account != self.config.project {
            return Teardown::Failed("cloud account differs".into());
        }
        Teardown::Confirmed(jcs(&json!({"provider":self.binding_id(),"resource":r.resource,"generation":r.generation,"observed":"detached"})).unwrap_or_default(),Cost::Measured(0))
    }
}
