//! Shared reference projections retain existing Nostr semantic validation.
use engine_proto::common::v1 as wire;
use nostr::contracts::{self, ArtifactRef, SourceHint};
use serde_json::{Value, json};
fn event(e: &wire::EventRef) -> Value {
    let mut value = json!({"id":e.id,"pubkey":e.pubkey,"kind":e.kind});
    if let Some(c) = &e.coordinate {
        value["coordinate"] = json!(c)
    };
    value
}
pub fn artifact(record: &wire::ArtifactRef) -> Result<ArtifactRef, String> {
    let mut value =
        json!({"digest":record.digest,"size":record.size,"media_type":record.media_type});
    if let Some(schema) = &record.schema {
        value["schema"] = json!(schema)
    }
    if let Some(e) = &record.event {
        value["event"] = event(e)
    }
    let mut sources = Vec::new();
    for source in &record.sources {
        sources.push(match (&source.url, &source.event) {
            (Some(url), None) => json!({"url":url}),
            (None, Some(e)) => json!({"event":event(e)}),
            _ => return Err("A source hint requires exactly one locator".into()),
        });
    }
    value["sources"] = Value::Array(sources);
    contracts::parse_artifact(&value).map_err(|e| e.to_string())
}
pub fn project(value: &ArtifactRef) -> wire::ArtifactRef {
    let event = |e: &contracts::EventRef| wire::EventRef {
        id: e.id.clone(),
        pubkey: e.pubkey.clone(),
        kind: e.kind.into(),
        coordinate: e.coordinate.clone(),
    };
    wire::ArtifactRef {
        digest: value.digest.clone(),
        size: value.size,
        media_type: value.media_type.clone(),
        schema: value.schema.clone(),
        event: value.event.as_ref().map(event),
        sources: value
            .sources
            .iter()
            .map(|s| match s {
                SourceHint::Url(url) => wire::SourceHint {
                    url: Some(url.clone()),
                    event: None,
                },
                SourceHint::Event(e) => wire::SourceHint {
                    url: None,
                    event: Some(event(e)),
                },
            })
            .collect(),
    }
}
