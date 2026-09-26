//! Install one explicit local task mapping and the four native operation pins.
use super::*;

impl Setup {
    pub fn client_configuration(&self) -> client::Configuration {
        client::Configuration {
            schema: "openagents.control-client.v1".into(),
            owner: self.owner.clone(),
            authority: self.authority.clone(),
            scope: self.scope.clone(),
            policy: self.policy.clone(),
            operations: self.operations.clone(),
            retain_until: self.retain_until,
        }
    }
    /// The local operator establishes ownership; this is not a remote API.
    /// The disclosure profile includes task instructions and ATIF content.
    pub fn for_task(
        task_directory: &Path,
        task_id: &str,
        owner: &str,
        authority: &str,
        now: u64,
        retain_until: u64,
    ) -> Result<Self> {
        if retain_until <= now.saturating_add(86400) {
            return Err("control setup needs more than one day of retention".into());
        }
        let task_directory = task_directory.canonicalize().map_err(|e| e.to_string())?;
        let task = coder::task::Store::open(&task_directory)
            .map_err(|e| e.to_string())?
            .show(task_id)
            .map_err(|e| e.to_string())?;
        let mut blobs = Blobs::default();
        let policy=blobs.insert_json(&json!({"v":"openagents.control-disclosure.v1","requires":[],"state":"task-projection","history":"atif-projection","max_items":128,"max_bytes":32768,"max_snapshot_steps":1024}),"openagents.control-disclosure.v1")?;
        let scope = json!({"task":random_id(),"controller":authority,"generation":0});
        let context=blobs.insert_json(&json!({"v":"openagents.context.v1","requires":[],"task":scope["task"],"recipient":authority,"policy":policy,"entries":[],"omissions":[],"coverage":"complete"}),"openagents.context.v1")?;
        let requirements=blobs.insert_json(&json!({"v":"openagents.control-operation-requirements.v1","requires":[],"scope":scope,"owner":owner,"policy":policy,"spend":"none","dispatch":"control-only"}),"openagents.control-operation-requirements.v1")?;
        let mut operations = BTreeMap::new();
        for (role, input, output) in [
            ("pair", control::PAIRING, control::ACCESS_RESULT),
            ("command", control::COMMAND, control::COMMAND_RESULT),
            ("read", control::READ, control::VIEW),
            ("revoke", control::REVOKE, control::ACCESS_RESULT),
        ] {
            let mut schema = |version: &str| {
                let schema =
                    json!({"type":"object","properties":{"v":{"const":version}},"required":["v"]});
                blobs.insert(
                    jcs(&schema).map_err(|e| e.to_string())?,
                    "application/schema+json",
                    "https://json-schema.org/draft/2020-12/schema",
                )
            };
            let input = schema(input)?;
            let output = schema(output)?;
            let definition = json!({"v":1,"requires":[],"id":format!("{authority}:control/{role}"),"profile":"native","summary":format!("Scoped task control: {role}"),"input":input,"output":output,"effects":{"reads":["control-scope"],"writes":if role=="read"{vec![]}else{vec!["control-scope"]},"network":[],"process":false,"delegates":false,"spend":false},"minimum":{},"support":{"bounds":{"output_bytes":"enforced","wall_ms":"unknown"},"cancellation":"before_dispatch","idempotency":"request_attempt","evidence":["private-artifact"]},"binding_contract":{"operation":role,"interface":"openagents.control-host.v1"}});
            let artifact = blobs.insert_json(&definition, "openagents.capability.v1")?;
            let target = json!({"id":definition["id"],"artifact":artifact});
            let lock=blobs.insert_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":target["id"],"definition":target,"dependencies":[]}]}),"openagents.lock.v1")?;
            operations.insert(
                role.into(),
                Operation {
                    target,
                    lock,
                    context: context.clone(),
                    requirements: requirements.clone(),
                },
            );
        }
        Ok(Self {
            schema: "openagents.control-setup.v1".into(),
            owner: owner.into(),
            authority: authority.into(),
            task_directory,
            task_id: task_id.into(),
            intent_digest: task.intent_digest,
            scope,
            policy,
            operations,
            blobs,
            max_pairing_seconds: 600,
            max_grant_seconds: 86400,
            max_command_seconds: 300,
            retain_until,
        })
    }
}
