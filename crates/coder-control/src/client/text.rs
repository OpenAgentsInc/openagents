//! Bounded encrypted delivery of exact UTF-8 bytes referenced by CTRL.
//! A carrier locates bytes; it grants no instruction or task authority.
use super::*;
use nostr::contracts::SourceHint;

pub const TEXT_SCHEMA: &str = "openagents.control-text.v1";
pub const CARRIER_SCHEMA: &str = "openagents.control-text-carrier.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextDelivery {
    pub reference: Value,
    pub declaration: Event,
    pub carrier: Event,
}

pub fn text(
    text: &str,
    secret: &SecretKey,
    authority: &str,
    now: u64,
    retain_until: u64,
) -> Result<TextDelivery> {
    if text.trim().is_empty() || text.len() > 32 * 1024 {
        return Err("steering text must contain at most 32768 UTF-8 bytes".into());
    }
    let raw = reference(text.as_bytes(), "text/plain", TEXT_SCHEMA);
    let carrier = envelope(
        &json!({"v":CARRIER_SCHEMA,"requires":[],"artifact":raw,"utf8":text}),
        secret,
        authority,
        now,
        retain_until,
    )?;
    let source = json!({"event":{"id":carrier.id,"pubkey":carrier.pubkey,"kind":carrier.kind}});
    let mut reference = raw;
    reference["sources"] = json!([source]);
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference,"inline":null,"issued_at":now,"retain_until":retain_until});
    let declaration = nostr::private_artifact::seal(
        &body,
        secret,
        &XOnlyPublicKey::from_str(authority).map_err(|e| e.to_string())?,
        &random_id(),
        now,
        random_bytes(),
    )
    .map_err(|e| e.to_string())?;
    let reference = event_reference(&declaration, &reference)?;
    Ok(TextDelivery {
        reference,
        declaration,
        carrier,
    })
}
impl TextDelivery {
    pub fn verify(&self, expected: &Value, principal: &str, secret: &SecretKey) -> Result<Vec<u8>> {
        if &self.reference != expected {
            return Err("text delivery differs from the command's exact reference".into());
        }
        let mut reference = parse_artifact(expected).map_err(|e| e.to_string())?;
        if reference.media_type != "text/plain"
            || reference.schema.as_deref() != Some(TEXT_SCHEMA)
            || reference.size > 32 * 1024
        {
            return Err("unsupported steering text profile".into());
        }
        let event = reference
            .event
            .take()
            .ok_or("text declaration is missing")?;
        if event.id != self.declaration.id || event.pubkey != principal || event.kind != 3188 {
            return Err("text declaration identity differs".into());
        }
        let declaration =
            nostr::private_artifact::open(&self.declaration, secret).map_err(|e| e.to_string())?;
        if declaration.signer() != principal
            || declaration.artifact() != &reference
            || declaration.inline_bytes().is_some()
        {
            return Err("external text declaration does not match".into());
        }
        let [SourceHint::Event(source)] = reference.sources.as_slice() else {
            return Err(
                "text carrier requires one exact event hint; URLs are not supported".into(),
            );
        };
        if source.id != self.carrier.id || source.pubkey != principal || source.kind != 3188 {
            return Err("text carrier identity differs".into());
        }
        let carrier =
            nostr::private_artifact::open(&self.carrier, secret).map_err(|e| e.to_string())?;
        if carrier.signer() != principal || carrier.recipient() != declaration.recipient() {
            return Err("text carrier author or recipient differs".into());
        }
        let value = nostr::contracts::parse_strict(
            carrier.inline_bytes().ok_or("text carrier unavailable")?,
        )
        .map_err(|e| e.to_string())?;
        if value["v"] != CARRIER_SCHEMA
            || value["requires"] != json!([])
            || value.as_object().is_none_or(|v| v.len() != 4)
        {
            return Err("unsupported text carrier shape".into());
        }
        reference.sources.clear();
        if parse_artifact(&value["artifact"]).map_err(|e| e.to_string())? != reference {
            return Err("text carrier names different bytes".into());
        }
        let text = value["utf8"]
            .as_str()
            .ok_or("text carrier is not UTF-8 text")?;
        if text.trim().is_empty() || text.len() > 32 * 1024 {
            return Err("text carrier exceeds its bound".into());
        }
        declaration
            .check_external(text.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(text.as_bytes().to_vec())
    }
}

/// Call only after the host admits the command's principal, rights, and scope.
pub async fn fetch_text(url: &str, secret: &SecretKey, reference: &Value) -> Result<TextDelivery> {
    let parsed = parse_artifact(reference).map_err(|e| e.to_string())?;
    if parsed.size > 32 * 1024
        || parsed.media_type != "text/plain"
        || parsed.schema.as_deref() != Some(TEXT_SCHEMA)
    {
        return Err("unsupported text fetch".into());
    }
    let event = parsed.event.as_ref().ok_or("text declaration missing")?;
    let [SourceHint::Event(source)] = parsed.sources.as_slice() else {
        return Err("text carrier event missing".into());
    };
    let declaration = nostr_transport::artifacts::fetch(url, secret, &event.id).await?;
    let carrier = nostr_transport::artifacts::fetch(url, secret, &source.id).await?;
    Ok(TextDelivery {
        reference: reference.clone(),
        declaration,
        carrier,
    })
}
