//! NIP-LAB wire records keep their full OrderRef and exact ArtifactRefs.
//! The compact role book is an internal representation, not the wire format.
use crate::{
    Result,
    book::{Applied, Book},
    records::*,
};
use nostr::{contracts, private_artifact::OpenEnvelope};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A caller-admitted confirmed order and acceptance policy. Verify the MKT
/// negotiation before constructing this local admission; parsing alone does
/// not confirm an order or appoint any role.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    order: Value,
    acceptance_policy: Value,
    /// Complete pinned JSON schemas for this bounded JSON-deliverable profile.
    schemas: BTreeMap<String, Value>,
}
pub type Bytes = BTreeMap<String, Vec<u8>>;
fn reference(value: &Value, bytes: &Bytes, expected: Option<&str>) -> Result<String> {
    let r = contracts::parse_artifact(value).map_err(|e| e.to_string())?;
    if expected.is_some_and(|schema| r.schema.as_deref() != Some(schema)) {
        return Err("the wire reference has a different schema".into());
    }
    let raw = bytes
        .get(&r.digest)
        .ok_or("wire artifact bytes are unavailable")?;
    contracts::check_artifact_bytes(&r, raw).map_err(|e| e.to_string())?;
    if let Some(schema) = expected {
        let body = contracts::parse_strict_bounded(raw, 1_048_576).map_err(|e| e.to_string())?;
        if body["v"] != schema {
            return Err("wire reference schema differs from its bytes".into());
        }
    }
    Ok(r.digest)
}
fn refs(value: &Value, bytes: &Bytes, expected: Option<&str>) -> Result<Value> {
    let array = value
        .as_array()
        .filter(|a| a.len() <= 64)
        .ok_or("invalid wire reference list")?;
    array
        .iter()
        .map(|v| reference(v, bytes, expected).map(Value::String))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}
impl Admission {
    pub fn from_confirmed(
        book: &Book,
        order: Value,
        acceptance_policy: Value,
        schemas: BTreeMap<String, Value>,
        negotiation: &nostr::market_contracts::Negotiation,
    ) -> Result<Self> {
        let parsed = nostr::market_contracts::parse_order_ref(&order).map_err(|e| e.to_string())?;
        negotiation
            .check_order_ref(&parsed)
            .map_err(|e| e.to_string())?;
        let admission = Self {
            order,
            acceptance_policy,
            schemas,
        };
        admission.check(book)?;
        Ok(admission)
    }
    #[cfg(test)]
    pub(crate) fn fixture(
        order: Value,
        acceptance_policy: Value,
        schemas: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            order,
            acceptance_policy,
            schemas,
        }
    }
    fn check(&self, book: &Book) -> Result<()> {
        let order =
            nostr::market_contracts::parse_order_ref(&self.order).map_err(|e| e.to_string())?;
        let terms = &book.terms;
        if order.market != terms.market
            || order.order_id != terms.order
            || order.buyer != terms.buyer
            || order.provider != terms.provider
            || self.schemas.len() != terms.deliverables.len()
            || terms
                .deliverables
                .iter()
                .any(|d| !self.schemas.contains_key(&d.id))
        {
            return Err("wire admission differs from the frozen role terms".into());
        }
        contracts::parse_artifact(&self.acceptance_policy).map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Decode only the declared wire fields and validate every supplied
    /// reference against exact bytes. Caller authentication stays separate.
    pub fn decode(&self, book: &Book, wire: &Value, bytes: &Bytes) -> Result<Record> {
        self.check(book)?;
        if wire["order"] != self.order || wire["requires"] != json!([]) {
            return Err("wire order or required features differ".into());
        }
        let mut value = wire.clone();
        value
            .as_object_mut()
            .ok_or("wire record is not an object")?
            .remove("requires");
        value["order"] = json!(book.terms.order);
        let mut fields: Vec<(&str, Option<&str>)> = Vec::new();
        let mut lists: Vec<(&str, Option<&str>)> = Vec::new();
        match wire["v"].as_str().ok_or("wire record version")? {
            SUBMISSION_V => {
                fields.extend([
                    ("previous", Some(SUBMISSION_V)),
                    ("rework", Some(REVIEW_V)),
                    ("limitations", None),
                ]);
                lists.extend([
                    ("executions", Some("openagents.labor-execution.v1")),
                    ("run_evidence", None),
                ]);
                let items = wire["deliverables"]
                    .as_array()
                    .filter(|a| a.len() == self.schemas.len())
                    .ok_or("incomplete wire deliverables")?;
                let mut decoded = Vec::new();
                let mut seen = std::collections::BTreeSet::new();
                for item in items {
                    let m = item.as_object().ok_or("wire deliverable")?;
                    if m.len() != 2 || !m.contains_key("id") || !m.contains_key("content") {
                        return Err("wire deliverable fields".into());
                    }
                    let id = item["id"].as_str().ok_or("wire deliverable id")?;
                    if !seen.insert(id) {
                        return Err("duplicate wire deliverable".into());
                    }
                    let schema = self.schemas.get(id).ok_or("unknown wire deliverable")?;
                    let digest = reference(&item["content"], bytes, None)?;
                    let raw = &bytes[&digest];
                    let content = contracts::parse_strict_bounded(raw, 1_048_576)
                        .map_err(|e| e.to_string())?;
                    let raw_schema = contracts::jcs(schema).map_err(|e| e.to_string())?;
                    let schema_digest = contracts::digest_bytes(&raw_schema);
                    let closure = contracts::prepare_closure(&BTreeMap::from([(
                        schema_digest.clone(),
                        raw_schema,
                    )]))
                    .map_err(|e| e.to_string())?;
                    contracts::validate_instance(&closure, &schema_digest, &content)
                        .map_err(|e| e.to_string())?;
                    decoded.push(json!({"id":id,"digest":digest,"size":raw.len()}));
                }
                value["deliverables"] = json!(decoded);
            }
            DELIVERY_V => fields.push(("submission", Some(SUBMISSION_V))),
            VERIFICATION_V => {
                if wire["policy"] != self.acceptance_policy {
                    return Err("wire verification changes the acceptance policy".into());
                }
                fields.extend([
                    ("submission", Some(SUBMISSION_V)),
                    ("policy", None),
                    ("limitations", None),
                ]);
                lists.push(("checker_receipts", None));
                let mut criteria = wire["criteria"].as_array().ok_or("wire criteria")?.clone();
                for criterion in &mut criteria {
                    criterion["evidence"] = refs(&criterion["evidence"], bytes, None)?;
                }
                value["criteria"] = json!(criteria);
            }
            REVIEW_V => fields.extend([
                ("submission", Some(SUBMISSION_V)),
                ("verification", Some(VERIFICATION_V)),
                ("reason", None),
            ]),
            DISPUTE_V => {
                fields.push(("subject", None));
                lists.push(("evidence", None));
            }
            CANCELLATION_V => fields.extend([
                ("reason", None),
                ("execution", Some("openagents.labor-execution.v1")),
            ]),
            RESOLUTION_V => {
                fields.extend([
                    ("submission", Some(SUBMISSION_V)),
                    ("verification", Some(VERIFICATION_V)),
                    ("reason", None),
                ]);
                lists.extend([("disputes", Some(DISPUTE_V)), ("evidence", None)]);
            }
            ACCEPTANCE_V => {
                fields.extend([
                    ("submission", Some(SUBMISSION_V)),
                    ("verification", Some(VERIFICATION_V)),
                    ("review", Some(REVIEW_V)),
                    ("resolution", Some(RESOLUTION_V)),
                ]);
                lists.extend([("supersedes", Some(ACCEPTANCE_V)), ("evidence", None)]);
            }
            _ => return Err("unsupported LAB wire record".into()),
        }
        for (field, schema) in fields {
            if !wire[field].is_null() {
                value[field] = json!(reference(&wire[field], bytes, schema)?);
            }
        }
        for (field, schema) in lists {
            value[field] = refs(&wire[field], bytes, schema)?;
        }
        Record::parse(&value)
    }
    /// Original signature and recipient are checked before the role book
    /// receives a canonical wire identity. A forwarder's key never substitutes.
    pub fn ingest(
        &self,
        book: &mut Book,
        opened: &OpenEnvelope,
        reader: &str,
        bytes: &Bytes,
        now: u64,
    ) -> Result<Applied> {
        if !book.terms.rights.permits(
            &book.terms,
            crate::terms::Use::PerformAndReview,
            reader,
            &[],
        ) || opened.recipient() != reader
            || opened.body().issued_at > now.saturating_add(300)
            || opened.body().retain_until < now
        {
            return Err("wire record recipient or lifetime differs".into());
        }
        let raw = opened.inline_bytes().ok_or("wire record is not inline")?;
        let wire = contracts::parse_strict_bounded(raw, 1_048_576).map_err(|e| e.to_string())?;
        if opened.artifact().schema.as_deref() != wire["v"].as_str() {
            return Err("wire envelope schema differs".into());
        }
        let record = self.decode(book, &wire, bytes)?;
        let applied = book.apply_canonical(
            record,
            opened.signer(),
            now,
            opened.artifact().digest.clone(),
        )?;
        book.remember_original(opened.artifact().digest.clone(), wire);
        Ok(applied)
    }
}
