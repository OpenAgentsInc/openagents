//! Finite snapshot/catch-up reads with recipient-bound server-side cursors.
use crate::{
    Result,
    artifact::{self, MAX_BYTES},
    host::{Host, State},
};
use nostr::{contracts, domain::Event, workspace};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Image {
    pub cut: Value,
    pub rows: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Cursor {
    pub mode: String,
    pub actor: String,
    pub grant_epoch: u64,
    pub cut: String,
    pub head: String,
    pub index: usize,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub(crate) definition: Value,
    pub(crate) reference: Value,
    pub(crate) binding: Value,
    pub(crate) recipient: String,
    pub(crate) grant_epoch: u64,
    pub(crate) resources: BTreeSet<String>,
    pub(crate) images: Vec<Image>,
    pub(crate) deltas: BTreeMap<String, Value>,
    pub(crate) cursors: BTreeMap<String, Cursor>,
    pub(crate) reads: BTreeMap<String, (String, Value)>,
    pub(crate) allowed: BTreeSet<String>,
    pub(crate) expired: bool,
}
impl Projection {
    /// Create a view after its source owner independently admits the recipient
    /// and selection. A definition or cursor alone grants nothing.
    pub fn new(
        definition: Value,
        binding: Value,
        epoch: u64,
        resources: BTreeSet<String>,
        store: &crate::artifact::Artifacts,
    ) -> Result<Self> {
        workspace::validate(&definition).map_err(|e| e.to_string())?;
        if definition["v"] != "openagents.projection-definition.v1" {
            return Err("projection definition required".into());
        }
        let reference = store.put_json(&definition)?;
        let recipient = definition["recipient"].as_str().ok_or("recipient")?.into();
        Ok(Self {
            definition,
            reference,
            binding,
            recipient,
            grant_epoch: epoch,
            resources,
            images: Vec::new(),
            deltas: BTreeMap::new(),
            cursors: BTreeMap::new(),
            reads: BTreeMap::new(),
            allowed: BTreeSet::new(),
            expired: false,
        })
    }
    pub fn definition(&self) -> &Value {
        &self.definition
    }
    pub fn reference(&self) -> &Value {
        &self.reference
    }
    pub fn recipient(&self) -> &str {
        &self.recipient
    }
    pub fn authorization_epoch(&self) -> u64 {
        self.grant_epoch
    }
    pub fn cut(&self) -> Result<&Value> {
        Ok(&self.current()?.cut)
    }
    pub fn permitted(&self, digest: &str) -> bool {
        !self.expired && self.allowed.contains(digest)
    }
    pub fn permit(&mut self, reference: &Value) -> Result<()> {
        self.allow(reference)
    }
    pub fn known_read(&self, key: &str, fingerprint: &str) -> Result<Option<Value>> {
        match self.reads.get(key) {
            Some((known, result)) if known == fingerprint => Ok(Some(result.clone())),
            Some(_) => Err("idempotency_conflict: read bytes differ".into()),
            None => Ok(None),
        }
    }
    pub fn remember_read(&mut self, key: String, fingerprint: String, result: Value) -> Result<()> {
        if self.reads.len() >= 512 {
            return Err("view read retention exceeded".into());
        }
        self.reads.insert(key, (fingerprint, result));
        Ok(())
    }
    /// Append only source-confirmed rows while the source owner's admission
    /// transaction is held. The owner supplies authorized retained frontiers.
    pub fn append(
        &mut self,
        store: &crate::artifact::Artifacts,
        rows: BTreeMap<String, Value>,
        frontiers: Vec<Value>,
        coverage: &str,
        now: u64,
    ) -> Result<()> {
        if self.expired || rows.len() > 256 || self.cursors.len() >= 2048 {
            return Err("projection retention exceeded".into());
        }
        let schema = store.get(&self.definition["row_schema"])?;
        let schema_digest = contracts::digest_bytes(&schema);
        let closure =
            contracts::prepare_closure(&BTreeMap::from([(schema_digest.clone(), schema)]))
                .map_err(|e| e.to_string())?;
        for (key, row) in &rows {
            if row["key"] != *key {
                return Err("row key differs".into());
            }
            let value = store.json(&row["value"])?;
            contracts::validate_instance(&closure, &schema_digest, &value)
                .map_err(|e| e.to_string())?;
            self.allow(&row["value"])?;
            for source in row["sources"].as_array().ok_or("row sources")? {
                store.get(source)?;
                self.allow(source)?;
            }
        }
        let previous = self.images.last().cloned();
        if previous.as_ref().is_some_and(|i| {
            i.rows == rows
                && i.cut["frontiers"] == json!(frontiers)
                && i.cut["coverage"] == coverage
        }) {
            return Ok(());
        }
        let (epoch, sequence) = if let Some(previous) = &previous {
            (
                previous.cut["epoch"]
                    .as_str()
                    .ok_or("view epoch")?
                    .to_string(),
                previous.cut["sequence"]
                    .as_u64()
                    .ok_or("view sequence")?
                    .checked_add(1)
                    .ok_or("view counter exhausted")?,
            )
        } else {
            (artifact::token(), 0)
        };
        let cut = json!({"v":"openagents.projection-cut.v1","requires":[],"projection":self.reference,"epoch":epoch,"sequence":sequence,"frontiers":frontiers,"consistency":"single_authority","coverage":coverage,"captured_at":now});
        workspace::validate(&cut).map_err(|e| e.to_string())?;
        let to = store.put_json(&cut)?;
        self.allow(&to)?;
        if let Some(previous) = previous {
            let from = artifact::json_ref(&previous.cut)?;
            let mut changes = Vec::new();
            for (key, row) in &rows {
                if previous.rows.get(key) != Some(row) {
                    changes.push(json!({"key":key,"action":"upsert","value":row["value"],"sources":row["sources"]}));
                }
            }
            for key in previous.rows.keys() {
                if !rows.contains_key(key) {
                    changes.push(json!({"key":key,"action":"remove","value":null,"sources":[]}));
                }
            }
            let delta = json!({"v":"openagents.projection-delta.v1","requires":[],"projection":self.reference,"from":from,"to":to,"changes":changes});
            workspace::verify_delta(&delta, &previous.cut, &cut).map_err(|e| e.to_string())?;
            let reference = store.put_json(&delta)?;
            self.allow(&reference)?;
            self.deltas.insert(
                from["digest"].as_str().ok_or("cut digest")?.into(),
                reference,
            );
        }
        self.images.push(Image { cut, rows });
        while self.images.len() > 64 {
            let old = self.images.remove(0);
            let reference = artifact::json_ref(&old.cut)?;
            self.deltas
                .remove(reference["digest"].as_str().ok_or("old cut")?);
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 8 * MAX_BYTES {
            self.expired = true;
            return Err("projection byte retention exceeded".into());
        }
        Ok(())
    }
    fn image(&self, digest: &str) -> Result<&Image> {
        self.images
            .iter()
            .find(|v| artifact::json_ref(&v.cut).is_ok_and(|r| r["digest"] == digest))
            .ok_or("stale: projection history is no longer retained".into())
    }
    fn current(&self) -> Result<&Image> {
        self.images.last().ok_or("projection has no image".into())
    }
    fn allow(&mut self, reference: &Value) -> Result<()> {
        let r = contracts::parse_artifact(reference).map_err(|e| e.to_string())?;
        self.allowed.insert(r.digest);
        Ok(())
    }
    fn new_cursor(&mut self, cursor: Cursor) -> String {
        let token = artifact::token();
        self.cursors.insert(token.clone(), cursor);
        token
    }
    pub fn page(
        &mut self,
        artifacts: &crate::artifact::Artifacts,
        actor: &str,
        request: &Value,
        reference: &Value,
    ) -> Result<Value> {
        let p = self;
        let mode = request["mode"].as_str().ok_or("mode")?;
        let max_items = request["max_items"].as_u64().ok_or("max items")? as usize;
        let max_bytes = request["max_bytes"].as_u64().ok_or("max bytes")? as usize;
        let current = p.current()?.clone();
        let current_ref = artifact::json_ref(&current.cut)?;
        let current_digest = current_ref["digest"]
            .as_str()
            .ok_or("cut digest")?
            .to_string();
        let cursor = if request["after"].is_null() {
            if mode != "snapshot" {
                return Err("missing changes cursor".into());
            }
            Cursor {
                mode: "snapshot".into(),
                actor: actor.into(),
                grant_epoch: p.grant_epoch,
                cut: current_digest.clone(),
                head: current_digest,
                index: 0,
                complete: false,
            }
        } else {
            p.cursors
                .get(request["after"].as_str().ok_or("cursor")?)
                .ok_or("stale cursor")?
                .clone()
        };
        if cursor.actor != actor
            || cursor.grant_epoch != p.grant_epoch
            || !cursor.complete && cursor.mode != mode
        {
            return Err("cursor scope differs".into());
        }
        if mode == "snapshot" {
            if cursor.complete {
                return Err("completed snapshot cursor is only valid for changes".into());
            }
            let image = p.image(&cursor.cut)?.clone();
            let cut_ref = artifact::json_ref(&image.cut)?;
            let rows = image.rows.into_values().collect::<Vec<_>>();
            if cursor.index > rows.len() {
                return Err("future cursor".into());
            }
            let mut supplied = Vec::new();
            let mut bytes = 1024usize;
            let mut index = cursor.index;
            while index < rows.len() && supplied.len() < max_items {
                let row = &rows[index];
                let size = contracts::jcs(row).map_err(|e| e.to_string())?.len()
                    + row["value"]["size"].as_u64().ok_or("row byte size")? as usize;
                if bytes.saturating_add(size) > max_bytes {
                    break;
                }
                supplied.push(row.clone());
                bytes += size;
                index += 1;
            }
            if index == cursor.index && index < rows.len() {
                return Err("limit_exceeded: first row exceeds page bound".into());
            }
            let complete = index == rows.len();
            let token = p.new_cursor(Cursor {
                index,
                complete,
                ..cursor
            });
            let page = json!({"v":"openagents.projection-page.v1","requires":[],"request":reference,"cut":cut_ref,"rows":supplied,"next":if complete{None}else{Some(token.clone())},"coverage":image.cut["coverage"],"resume":if complete{Some(token)}else{None}});
            workspace::validate(&page).map_err(|e| e.to_string())?;
            if contracts::jcs(&page).map_err(|e| e.to_string())?.len() > max_bytes {
                return Err("limit_exceeded: snapshot framing exceeds its bound".into());
            }
            return Ok(page);
        }
        let head = if cursor.complete {
            current_ref
        } else {
            artifact::json_ref(&p.image(&cursor.head)?.cut)?
        };
        let from = artifact::json_ref(&p.image(&cursor.cut)?.cut)?;
        let head_cut = p
            .image(head["digest"].as_str().ok_or("head digest")?)?
            .cut
            .clone();
        let from_cut = p.image(&cursor.cut)?.cut.clone();
        if head_cut["epoch"] != from_cut["epoch"] {
            return Err("projection epoch changed".into());
        }
        let mut to = from.clone();
        let mut deltas = Vec::new();
        let mut bytes = 1536usize;
        while to != head && deltas.len() < max_items {
            let digest = to["digest"].as_str().ok_or("cut digest")?;
            let delta_ref = p.deltas.get(digest).ok_or("projection history gap")?;
            let delta = artifacts.json(delta_ref)?;
            let size = delta_ref["size"].as_u64().ok_or("delta size")? as usize + 256;
            if bytes.saturating_add(size) > max_bytes {
                break;
            }
            let next_cut = p.image(delta["to"]["digest"].as_str().ok_or("to cut")?)?;
            workspace::verify_delta(&delta, &p.image(digest)?.cut, &next_cut.cut)
                .map_err(|e| e.to_string())?;
            to = delta["to"].clone();
            deltas.push(delta_ref.clone());
            bytes += size;
        }
        if to == from && to != head {
            return Err("limit_exceeded: first delta exceeds page bound".into());
        }
        let complete = to == head;
        let token = p.new_cursor(Cursor {
            mode: "changes".into(),
            actor: actor.into(),
            grant_epoch: p.grant_epoch,
            cut: to["digest"].as_str().ok_or("to cut")?.into(),
            head: head["digest"].as_str().ok_or("head cut")?.into(),
            index: 0,
            complete,
        });
        let marker = if complete {
            let marker = json!({"v":"openagents.projection-marker.v1","requires":[],"projection":p.reference,"cut":head,"state":"caught_up"});
            let reference = artifacts.put_json(&marker)?;
            p.allow(&reference)?;
            Some(reference)
        } else {
            None
        };
        let page = json!({"v":"openagents.projection-change-page.v1","requires":[],"request":reference,"from":from,"to":to,"head":head,"deltas":deltas,"next":if complete{None}else{Some(token.clone())},"resume":if complete{Some(token)}else{None},"marker":marker});
        workspace::validate(&page).map_err(|e| e.to_string())?;
        if contracts::jcs(&page).map_err(|e| e.to_string())?.len() > max_bytes {
            return Err("limit_exceeded: changes framing exceeds its bound".into());
        }
        Ok(page)
    }
}
impl Host {
    /// Host administration publishes one precise recipient/selection. The
    /// definition never admits a wildcard query or widens a resource grant.
    pub fn define_projection(
        &self,
        recipient: &str,
        resources: BTreeSet<String>,
        retain_until: u64,
        now: u64,
    ) -> Result<Value> {
        if resources.is_empty()
            || resources.len() > 256
            || retain_until <= now
            || retain_until > now + 7 * 86_400
        {
            return Err("invalid projection selection or retention".into());
        }
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        if state.projections.len() >= 64 {
            return Err("projection bound exceeded".into());
        }
        let grant = self.check(&state, recipient, None, false, now)?.clone();
        for id in &resources {
            self.check(&state, recipient, Some(id), false, now)?;
            if !state.resources.contains_key(id) {
                return Err("not_admitted: resource is unavailable".into());
            }
        }
        let scope=self.artifacts.put_json(&json!({"v":"openagents.ws-document-selection.v1","requires":[],"binding":state.binding_ref,"resources":resources}))?;
        let parameters=self.artifacts.put_json(&json!({"v":"openagents.ws-document-view-parameters.v1","requires":[],"content":"exact_utf8","parents":"roots_only","withheld":[{"artifact":state.binding_ref,"state":"redacted"},{"artifact":grant.reference,"state":"redacted"}]}))?;
        let transform=self.definition("document-view",json!({"v":"openagents.ws-document-view.v1","requires":[],"row":"document_snapshot_with_resource_descriptor","source_order":"git_reflog_prefix_digest","completeness":"only_enumerated_admitted_roots"}))?;
        let wire: Value =
            serde_json::from_slice(workspace::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let exact = |schema: &str| json!({"allOf":[{"$ref":"#/$defs/artifact"},{"required":["schema"],"properties":{"schema":{"const":schema}}}]});
        let schema_bytes=contracts::jcs(&json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":{"artifact":wire["$defs"]["artifact"],"event":wire["$defs"]["event"]},"type":"object","additionalProperties":false,"required":["v","requires","resource","snapshot","dependencies"],"properties":{"v":{"const":"openagents.ws-document-row.v1"},"requires":{"type":"array","maxItems":0},"resource":exact("openagents.workspace-resource.v1"),"snapshot":exact("openagents.document-snapshot.v1"),"dependencies":{"type":"array","maxItems":2,"items":{"type":"object","additionalProperties":false,"required":["artifact","state"],"properties":{"artifact":{"$ref":"#/$defs/artifact"},"state":{"enum":["redacted","unavailable"]}}}}}})).map_err(|e|e.to_string())?;
        let row_schema = self
            .artifacts
            .put(&schema_bytes, "application/schema+json", None)?;
        let mut sources = Vec::new();
        for id in &resources {
            let resource = state.resources.get(id).ok_or("resource")?;
            sources.push(self.artifacts.put_json(&json!({"v":"openagents.ws-git-source.v1","requires":[],"resource":resource.reference,"binding":state.binding_ref,"adapter":resource.adapter}))?);
        }
        let definition = json!({"v":"openagents.projection-definition.v1","requires":[],"projection":artifact::token(),"authority":self.signer.pubkey(),"recipient":recipient,"scope":scope,"policy":state.binding["policy"],"authorization":grant.reference,"transform":transform,"parameters":parameters,"row_schema":row_schema,"sources":sources,"retention":{"retain_until":retain_until,"max_items":256,"max_bytes":8*MAX_BYTES}});
        workspace::validate(&definition).map_err(|e| e.to_string())?;
        let reference = self.artifacts.put_json(&definition)?;
        let mut projection = Projection {
            definition,
            reference: reference.clone(),
            binding: state.binding_ref.clone(),
            recipient: recipient.into(),
            grant_epoch: grant.epoch,
            resources,
            images: Vec::new(),
            deltas: BTreeMap::new(),
            cursors: BTreeMap::new(),
            reads: BTreeMap::new(),
            allowed: BTreeSet::new(),
            expired: false,
        };
        self.poll(&state, &mut projection, now)?;
        for reference in [
            &projection.reference.clone(),
            &scope,
            &parameters,
            &row_schema,
            &state.binding["policy"],
            &transform["artifact"],
        ] {
            projection.allow(reference)?;
        }
        let id = reference["digest"]
            .as_str()
            .ok_or("projection digest")?
            .to_string();
        state.projections.insert(id, projection);
        self.save(&state)?;
        Ok(reference)
    }
    fn projection_access(
        &self,
        state: &State,
        p: &Projection,
        actor: &str,
        now: u64,
    ) -> Result<()> {
        let grant = self.check(state, actor, None, false, now)?;
        if p.expired
            || p.recipient != actor
            || p.grant_epoch != grant.epoch
            || p.binding != state.binding_ref
            || now
                > p.definition["retention"]["retain_until"]
                    .as_u64()
                    .ok_or("projection retention")?
        {
            return Err("not_admitted: projection is unavailable; request a new view".into());
        }
        for id in &p.resources {
            self.check(state, actor, Some(id), false, now)?;
        }
        Ok(())
    }
    fn poll(&self, state: &State, p: &mut Projection, now: u64) -> Result<()> {
        self.projection_access(state, p, &p.recipient, now)?;
        let schema = self.artifacts.get(&p.definition["row_schema"])?;
        let schema_digest = contracts::digest_bytes(&schema);
        let schema_closure =
            contracts::prepare_closure(&BTreeMap::from([(schema_digest.clone(), schema)]))
                .map_err(|e| e.to_string())?;
        let previous = p.images.last().cloned();
        let mut rows = BTreeMap::new();
        let mut frontiers = Vec::new();
        let resources = p.resources.clone();
        let mut fork = false;
        for (index, id) in resources.iter().enumerate() {
            let resource = state
                .resources
                .get(id)
                .ok_or("projection source unavailable")?;
            let capture = resource.source.capture()?;
            let position = capture.position.clone();
            let (_, snapshot) = self.snapshot_from(state, resource, capture, now)?;
            let position = position.as_str();
            if let Some(previous) = &previous {
                let old = previous.cut["frontiers"][index]["position"]
                    .as_str()
                    .ok_or("prior source frontier")?;
                if old == position {
                    let row = previous.rows.get(id).ok_or("prior source row")?.clone();
                    rows.insert(id.clone(), row);
                    frontiers.push(previous.cut["frontiers"][index].clone());
                    continue;
                }
                if !resource.source.feed_contiguous(old, position)? {
                    fork = true;
                }
            }
            let snapshot_ref = self.artifacts.put_json(&snapshot)?;
            let descriptor = json!({"v":"openagents.workspace-resource.v1","requires":[],"ref":resource.reference,"binding":state.binding_ref,"kind":"document","parent":null,"name":resource.name,"adapter":resource.adapter,"operations":[resource.operation],"observation":snapshot["observation"],"lifecycle":"available"});
            workspace::validate(&descriptor).map_err(|e| e.to_string())?;
            let descriptor_ref = self.artifacts.put_json(&descriptor)?;
            let row_body = json!({"v":"openagents.ws-document-row.v1","requires":[],"resource":descriptor_ref,"snapshot":snapshot_ref,"dependencies":[{"artifact":state.binding_ref,"state":"redacted"}]});
            contracts::validate_instance(&schema_closure, &schema_digest, &row_body)
                .map_err(|e| e.to_string())?;
            let value = self.artifacts.put_json(&row_body)?;
            let row = json!({"key":id,"value":value,"sources":[snapshot["observation"]]});
            rows.insert(id.clone(), row);
            for r in [
                &value,
                &descriptor_ref,
                &snapshot_ref,
                &snapshot["observation"],
                &snapshot["content"],
                &resource.adapter["artifact"],
                &resource.operation["artifact"],
            ] {
                p.allow(r)?;
            }
            let source = p.definition["sources"][index].clone();
            p.allow(&source)?;
            frontiers.push(json!({"source":source,"position":position}));
        }
        if previous
            .as_ref()
            .is_some_and(|i| i.rows == rows && i.cut["frontiers"] == json!(frontiers))
            && !fork
        {
            return Ok(());
        }
        let (epoch, sequence) = if let Some(previous) = &previous {
            if fork {
                (artifact::token(), 0)
            } else {
                (
                    previous.cut["epoch"]
                        .as_str()
                        .ok_or("projection epoch")?
                        .to_string(),
                    previous.cut["sequence"]
                        .as_u64()
                        .ok_or("cut sequence")?
                        .checked_add(1)
                        .ok_or("projection counter exhausted")?,
                )
            }
        } else {
            (artifact::token(), 0)
        };
        let cut = json!({"v":"openagents.projection-cut.v1","requires":[],"projection":p.reference,"epoch":epoch,"sequence":sequence,"frontiers":frontiers,"consistency":"single_authority","coverage":"complete","captured_at":now});
        workspace::validate(&cut).map_err(|e| e.to_string())?;
        let cut_ref = self.artifacts.put_json(&cut)?;
        p.allow(&cut_ref)?;
        if let Some(previous) = previous
            && !fork
        {
            let mut changes = Vec::new();
            for (key, row) in &rows {
                if previous.rows.get(key) != Some(row) {
                    changes.push(json!({"key":key,"action":"upsert","value":row["value"],"sources":row["sources"]}));
                }
            }
            for key in previous.rows.keys() {
                if !rows.contains_key(key) {
                    changes.push(json!({"key":key,"action":"remove","value":null,"sources":[]}));
                }
            }
            let from = artifact::json_ref(&previous.cut)?;
            let delta = json!({"v":"openagents.projection-delta.v1","requires":[],"projection":p.reference,"from":from,"to":cut_ref,"changes":changes});
            workspace::verify_delta(&delta, &previous.cut, &cut).map_err(|e| e.to_string())?;
            let delta_ref = self.artifacts.put_json(&delta)?;
            p.allow(&delta_ref)?;
            p.deltas.insert(
                from["digest"].as_str().ok_or("from digest")?.into(),
                delta_ref,
            );
        }
        if fork {
            p.images.clear();
            p.deltas.clear();
            p.cursors.clear();
            p.reads.clear();
        }
        p.images.push(Image { cut, rows });
        while p.images.len() > 64 {
            let old = p.images.remove(0);
            let digest = artifact::json_ref(&old.cut)?;
            p.deltas.remove(digest["digest"].as_str().ok_or("old cut")?);
        }
        let bytes = serde_json::to_vec(p).map_err(|e| e.to_string())?.len();
        if bytes > 8 * MAX_BYTES {
            p.expired = true;
            return Err(
                "limit_exceeded: projection retention exhausted; request a new view".into(),
            );
        }
        Ok(())
    }
    pub fn projection_read(&self, event: &Event, now: u64) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, request) = self.authenticated(event, now, "openagents.projection-read.v1")?;
        workspace::validate(&request).map_err(|e| e.to_string())?;
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        let reference = self.artifacts.put_json(&request)?;
        let key = request["projection"]["digest"]
            .as_str()
            .ok_or("projection digest")?;
        let Some(mut p) = state.projections.get(key).cloned() else {
            return self.refused(&reference, "not_admitted");
        };
        if p.reference != request["projection"]
            || self.projection_access(&state, &p, &actor, now).is_err()
        {
            return self.refused(&reference, "not_admitted");
        }
        let read_id = request["request"].as_str().ok_or("read ID")?.to_string();
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&request).map_err(|e| e.to_string())?);
        if let Some((prior, result)) = p.reads.get(&read_id) {
            if prior != &fingerprint {
                return self.refused(&reference, "idempotency_conflict");
            }
            return Ok(result.clone());
        }
        if p.reads.len() >= 512 || p.cursors.len() >= 2048 {
            return self.refused(&reference, "limit_exceeded");
        }
        // Observe a finite head only at the beginning of a new enumeration.
        let start = request["after"].is_null()
            || request["after"]
                .as_str()
                .and_then(|token| p.cursors.get(token))
                .is_some_and(|cursor| cursor.complete);
        if start && let Err(error) = self.poll(&state, &mut p, now) {
            state.projections.insert(key.into(), p);
            self.save(&state)?;
            return self.refused(
                &reference,
                if error.starts_with("limit_exceeded") {
                    "limit_exceeded"
                } else {
                    "unavailable"
                },
            );
        }
        if self
            .projection_access(
                &state,
                &p,
                &actor,
                now.saturating_add(started.elapsed().as_secs()),
            )
            .is_err()
        {
            return self.refused(&reference, "not_admitted");
        }
        let result = match self.page(&mut p, &actor, &request, &reference) {
            Ok(page) => {
                let schema = page["v"].as_str().ok_or("page schema")?;
                if (request["mode"] == "snapshot" && schema != "openagents.projection-page.v1")
                    || (request["mode"] == "changes"
                        && schema != "openagents.projection-change-page.v1")
                {
                    return Err("projection page mode differs".into());
                }
                let page_ref = self.artifacts.put_json(&page)?;
                p.allow(&page_ref)?;
                json!({"v":"openagents.projection-result.v1","requires":[],"request":reference,"status":"completed","page":page_ref,"marker":null,"reason":null})
            }
            Err(error) => self.refused(
                &reference,
                if error.starts_with("limit_exceeded") {
                    "limit_exceeded"
                } else {
                    "stale"
                },
            )?,
        };
        workspace::validate(&result).map_err(|e| e.to_string())?;
        p.reads.insert(read_id, (fingerprint, result.clone()));
        state.projections.insert(key.into(), p);
        self.save(&state)?;
        Ok(result)
    }
    fn refused(&self, request: &Value, reason: &str) -> Result<Value> {
        let result = json!({"v":"openagents.projection-result.v1","requires":[],"request":request,"status":"refused","page":null,"marker":null,"reason":reason});
        workspace::validate(&result).map_err(|e| e.to_string())?;
        Ok(result)
    }
    fn page(
        &self,
        p: &mut Projection,
        actor: &str,
        request: &Value,
        reference: &Value,
    ) -> Result<Value> {
        p.page(&self.artifacts, actor, request, reference)
    }
    pub fn command_visibility(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, request) =
            self.authenticated(event, now, "openagents.ws-command-visibility-read.v1")?;
        let object = request.as_object().ok_or("visibility request")?;
        if object.len() != 4
            || object
                .keys()
                .any(|k| !matches!(k.as_str(), "v" | "requires" | "projection" | "command"))
            || request["requires"] != json!([])
        {
            return Err("unsupported visibility request".into());
        }
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        let projection_key = request["projection"]["digest"]
            .as_str()
            .ok_or("projection digest")?
            .to_string();
        let p = state
            .projections
            .get(&projection_key)
            .ok_or("not_admitted: projection unavailable")?;
        if p.reference != request["projection"] {
            return Err("not_admitted: projection unavailable".into());
        }
        self.projection_access(&state, p, &actor, now)?;
        let command = state
            .commands
            .values()
            .find(|c| {
                c.result
                    .as_ref()
                    .is_some_and(|r| r["receipt"] == request["command"])
            })
            .ok_or("not_admitted: command receipt unavailable")?;
        if command.actor != actor && actor != self.signer.pubkey() {
            return Err("not_admitted: command receipt unavailable".into());
        }
        self.check(&state, &actor, Some(&command.resource), false, now)?;
        let mut cut = None;
        let status = if !p.resources.contains(&command.resource) || !command.dispatched {
            "not_in_scope"
        } else {
            let index = p
                .resources
                .iter()
                .position(|id| id == &command.resource)
                .ok_or("resource selection")?;
            let current = p.current()?;
            let position = current.cut["frontiers"][index]["position"]
                .as_str()
                .ok_or("source frontier")?;
            let resource = state.resources.get(&command.resource).ok_or("resource")?;
            if command.candidate.as_deref().is_some_and(|candidate| {
                resource
                    .source
                    .feed_contains(position, candidate)
                    .unwrap_or(false)
            }) {
                cut = Some(artifact::json_ref(&current.cut)?);
                "visible"
            } else {
                "pending"
            }
        };
        let body = json!({"v":"openagents.projection-command-visibility.v1","requires":[],"command":request["command"],"projection":request["projection"],"cut":cut,"state":status});
        workspace::validate(&body).map_err(|e| e.to_string())?;
        let reference = self.artifacts.put_json(&body)?;
        let p = state
            .projections
            .get_mut(&projection_key)
            .ok_or("projection")?;
        p.allow(&reference)?;
        p.allow(&request["command"])?;
        self.save(&state)?;
        Ok(json!({"reference":reference,"body":body}))
    }
    pub fn projection_fetch(&self, event: &Event, now: u64) -> Result<Vec<u8>> {
        let started = std::time::Instant::now();
        let _guard = self.artifacts.lock()?;
        let (actor, request) =
            self.authenticated(event, now, "openagents.ws-projection-fetch.v1")?;
        let object = request.as_object().ok_or("fetch request")?;
        if object.len() != 5
            || object.keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "v" | "requires" | "projection" | "artifact" | "max_bytes"
                )
            })
            || request["requires"] != json!([])
        {
            return Err("unsupported projection fetch".into());
        }
        let state = self.state()?;
        let p = state
            .projections
            .get(
                request["projection"]["digest"]
                    .as_str()
                    .ok_or("projection digest")?,
            )
            .ok_or("not_admitted: projection unavailable")?;
        self.projection_access(&state, p, &actor, now)?;
        let reference =
            contracts::parse_artifact(&request["artifact"]).map_err(|e| e.to_string())?;
        let limit = request["max_bytes"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= MAX_BYTES as u64)
            .ok_or("fetch byte limit")?;
        if p.reference != request["projection"]
            || !p.allowed.contains(&reference.digest)
            || reference.size > limit
        {
            return Err("not_admitted: artifact is unavailable within this view".into());
        }
        let bytes = self.artifacts.get(&request["artifact"])?;
        self.projection_access(
            &state,
            p,
            &actor,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        Ok(bytes)
    }
}
/// A client keeps its own verified high cut across reconnects and snapshots.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Client {
    pub definition: Option<Value>,
    pub cut: Option<Value>,
    pub rows: BTreeMap<String, Value>,
    pub transitions: BTreeMap<String, String>,
    pub assembling: Option<(Value, BTreeMap<String, Value>, Value)>,
}
impl Client {
    pub fn snapshot_page(&mut self, request: &Value, page: &Value, cut: &Value) -> Result<()> {
        workspace::validate(request).map_err(|e| e.to_string())?;
        workspace::validate(page).map_err(|e| e.to_string())?;
        workspace::validate(cut).map_err(|e| e.to_string())?;
        if request["mode"] != "snapshot"
            || request["projection"] != cut["projection"]
            || page["request"] != artifact::json_ref(request)?
            || page["cut"] != artifact::json_ref(cut)?
            || page["coverage"] != cut["coverage"]
        {
            return Err("snapshot request or cut differs".into());
        }
        if let Some(definition) = &self.definition
            && *definition != cut["projection"]
        {
            return Err("projection definition changed; admit a new view".into());
        }
        if let Some(prior) = &self.cut {
            if prior["epoch"] != cut["epoch"] {
                return Err("projection epoch changed; explicitly reset the client".into());
            }
            if cut["sequence"].as_u64() < prior["sequence"].as_u64()
                || cut["sequence"] == prior["sequence"] && cut != prior
            {
                return Err("stale or conflicting snapshot cut".into());
            }
        }
        let mut rows = if let Some((expected, rows, next)) = &self.assembling {
            if expected != cut || *next != request["after"] {
                return Err("snapshot page continuity differs".into());
            }
            rows.clone()
        } else {
            if !request["after"].is_null() {
                return Err("snapshot started after its first page".into());
            }
            BTreeMap::new()
        };
        for row in page["rows"].as_array().ok_or("snapshot rows")? {
            let key = row["key"].as_str().ok_or("row key")?;
            if rows.insert(key.into(), row.clone()).is_some() {
                return Err("snapshot repeats a row".into());
            }
        }
        if rows.len() > 256
            || contracts::jcs(&json!(rows))
                .map_err(|e| e.to_string())?
                .len()
                > 8 * MAX_BYTES
        {
            return Err("client snapshot row bound exceeded".into());
        }
        if page["next"].is_null() {
            self.rows = rows;
            self.cut = Some(cut.clone());
            self.definition = Some(cut["projection"].clone());
            self.assembling = None;
            self.transitions.clear();
        } else {
            self.assembling = Some((cut.clone(), rows, page["next"].clone()));
        }
        Ok(())
    }
    pub fn delta(&mut self, delta: &Value, from: &Value, to: &Value) -> Result<()> {
        workspace::verify_delta(delta, from, to).map_err(|e| e.to_string())?;
        let key = artifact::json_ref(from)?["digest"]
            .as_str()
            .ok_or("from digest")?
            .to_string();
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(delta).map_err(|e| e.to_string())?);
        if let Some(known) = self.transitions.get(&key) {
            if known == &fingerprint {
                return Ok(());
            }
            return Err("conflicting delta bytes".into());
        }
        if self.assembling.is_some()
            || self.cut.as_ref() != Some(from)
            || self.definition.as_ref() != Some(&delta["projection"])
        {
            return Err("projection gap; repair or resnapshot before applying".into());
        }
        let mut rows = self.rows.clone();
        for change in delta["changes"].as_array().ok_or("delta changes")? {
            let key = change["key"].as_str().ok_or("change key")?;
            if change["action"] == "remove" {
                rows.remove(key);
            } else {
                rows.insert(
                    key.into(),
                    json!({"key":key,"value":change["value"],"sources":change["sources"]}),
                );
            }
        }
        if rows.len() > 256
            || self.transitions.len() >= 4096
            || contracts::jcs(&json!(rows))
                .map_err(|e| e.to_string())?
                .len()
                > 8 * MAX_BYTES
        {
            return Err("client retention bound exceeded; resnapshot is required".into());
        }
        self.rows = rows;
        self.cut = Some(to.clone());
        self.transitions.insert(key, fingerprint);
        Ok(())
    }
}
