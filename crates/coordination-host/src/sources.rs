//! Source identities come from the admitted workspace owner.
use crate::{
    Result,
    book::{Coordinator, Resource, Scope},
};
use nostr::{contracts, workspace};
use serde_json::Value;
impl Coordinator {
    pub fn mount_source(mut self, name: String, host: workspace_host::host::Host) -> Result<Self> {
        if name.is_empty() || name.len() > 128 || self.sources.contains_key(&name) {
            return Err("invalid or duplicate source mount".into());
        }
        self.sources.insert(name, host);
        Ok(self)
    }
    pub fn resource(
        &self,
        id: String,
        source: String,
        reference: Value,
        now: u64,
    ) -> Result<Resource> {
        let resource = workspace::resource(&reference).map_err(|e| e.to_string())?;
        if resource.authority != self.authority() {
            return Err(
                "native sources require the local owner's separately admitted workspace".into(),
            );
        }
        let actual = self
            .sources
            .get(&source)
            .ok_or("source is not mounted")?
            .coordination_resource(&resource, now)?;
        Ok(Resource {
            id,
            domain: actual["domain"].as_str().ok_or("source domain")?.into(),
            source,
            reference,
            binding: actual["binding"].clone(),
            owner: self.authority().into(),
        })
    }
    pub(crate) fn source_current(
        &self,
        resource: &Resource,
        snapshot: &Value,
        write: bool,
        now: u64,
    ) -> Result<()> {
        let actual = self.resource(
            resource.id.clone(),
            resource.source.clone(),
            resource.reference.clone(),
            now,
        )?;
        if actual.domain != resource.domain || actual.binding != resource.binding {
            return Err("stale: admitted source identity changed".into());
        }
        if !snapshot.is_null() {
            self.sources[&resource.source].coordination_snapshot(
                &workspace::resource(&resource.reference).map_err(|e| e.to_string())?,
                snapshot,
                write,
                now,
            )?;
        } else if write {
            return Err("cannot_enforce: a write requires an exact source snapshot".into());
        }
        Ok(())
    }
    pub(crate) fn resources_current(
        &self,
        scope: &Scope,
        resources: &Value,
        now: u64,
    ) -> Result<std::collections::BTreeMap<String, String>> {
        let mut domains = std::collections::BTreeMap::new();
        for entry in resources.as_array().ok_or("claim resource list")? {
            let resource = scope
                .resources
                .get(entry["id"].as_str().ok_or("resource ID")?)
                .ok_or("not_admitted: resource outside namespace")?;
            let write = entry["mode"] == "write";
            self.source_current(resource, &entry["snapshot"], write, now)?;
            if !write && !entry["snapshot"].is_null() {
                continue;
            }
            let mode = if write { "write" } else { "read" };
            if mode == "write" || !domains.contains_key(&resource.domain) {
                domains.insert(resource.domain.clone(), mode.into());
            }
        }
        Ok(domains)
    }
    pub(crate) fn schema_input(&self, schema: &Value, input: &Value) -> Result<()> {
        let schema_ref = contracts::parse_schema_ref(schema).map_err(|e| e.to_string())?;
        let bytes = self.store.get(schema)?;
        let closure = contracts::prepare_closure(&std::collections::BTreeMap::from([(
            schema_ref.0.digest.clone(),
            bytes,
        )]))
        .map_err(|e| e.to_string())?;
        contracts::validate_instance(&closure, &schema_ref.0.digest, input)
            .map_err(|e| e.to_string())
    }
}
impl Coordinator {
    /// Owner administration: capture through the real admitted source reader.
    pub fn capture_resource(&self, resource: &Resource, now: u64) -> Result<Value> {
        self.source_current(resource, &Value::Null, false, now)?;
        let source = self
            .sources
            .get(&resource.source)
            .ok_or("source not mounted")?;
        let (read, files) = source.coordination_capture(
            &workspace::resource(&resource.reference).map_err(|e| e.to_string())?,
            now,
        )?;
        let _guard = self.store.lock()?;
        self.state()?;
        self.attachments(&files)?;
        Ok(read)
    }
}
