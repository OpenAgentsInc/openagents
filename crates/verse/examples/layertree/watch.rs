//! Display only attestation and follower results actually present in the log.

use super::model::Event;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct Attestation {
    pub platform: String,
    pub measurement: String,
    pub release: String,
    pub result: String,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub height: u32,
    pub hit: bool,
    pub result: String,
}

#[derive(Clone, Debug, Default)]
pub struct Watch {
    pub observers: Option<Vec<u64>>,
    pub attestation: Attestation,
    pub clients: BTreeMap<u64, (String, Option<u32>)>,
    pub blocks: Vec<Block>,
    pub alarm: Option<String>,
    pub old_key: Option<String>,
    pub new_key: Option<String>,
    pub notice_until: Option<u32>,
    tracked: BTreeSet<String>,
}

impl Watch {
    pub fn apply(&mut self, event: &Event) -> Result<(), String> {
        match event.kind.as_str() {
            "tree_open" => {
                self.tracked.insert(event.field("outpoint")?);
                if event.data.contains_key("observers") {
                    self.observers = Some(event.field("observers")?);
                }
            }
            "transition" => {
                self.tracked
                    .insert(format!("{}:0", event.field::<String>("txid")?));
                if event.field::<String>("op")? == "migrate" {
                    self.old_key = event
                        .data
                        .get("old_enclave_key")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                    self.new_key = event
                        .data
                        .get("new_enclave_key")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                    self.notice_until = event
                        .data
                        .get("notice_until")
                        .and_then(|v| v.as_u64())
                        .and_then(|n| n.try_into().ok());
                }
            }
            "attestation" => {
                let entry: serde_json::Value = event.field("log_entry")?;
                self.attestation = Attestation {
                    platform: event.field("platform")?,
                    measurement: event.field("measurement")?,
                    result: event.field("result")?,
                    release: entry
                        .get("source_commit")
                        .or_else(|| entry.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unlisted")
                        .into(),
                };
            }
            "block" => {
                let height = event.height.ok_or("A block needs its height.")?;
                let spent: Vec<String> = event.field("spent")?;
                let result = self
                    .clients
                    .values()
                    .filter(|(_, h)| *h == Some(height))
                    .map(|(r, _)| r.clone())
                    .max_by_key(|r| r != "ok")
                    .unwrap_or_default();
                self.blocks.push(Block {
                    height,
                    hit: spent.iter().any(|p| self.tracked.contains(p)),
                    result,
                });
                if self.blocks.len() > 16 {
                    self.blocks.remove(0);
                }
            }
            "light_client" => {
                let result: String = event.field("result")?;
                self.clients
                    .insert(event.field("member")?, (result.clone(), event.height));
                if result != "ok" {
                    self.alarm = Some(result.clone());
                }
                if let Some(block) = self
                    .blocks
                    .iter_mut()
                    .rev()
                    .find(|b| Some(b.height) == event.height)
                {
                    block.result = result;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layertree::model::Run;
    use std::path::Path;
    #[test]
    fn every_logged_client_alarms_and_attestation_errors_keep_their_names() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/verse/fixtures/layertree-alarm.jsonl");
        let run = Run::load(&path).unwrap();
        let end = run.snapshot(run.events.len() as u64 - 1).unwrap();
        assert_eq!(end.watch.clients.len(), 3);
        assert_eq!(end.watch.observers.as_deref(), Some([0, 1, 2].as_slice()));
        assert!(
            end.watch
                .clients
                .values()
                .all(|(status, height)| status == "OutputMismatch" && *height == Some(102))
        );
        assert_eq!(end.watch.alarm.as_deref(), Some("OutputMismatch"));
        assert!(end.watch.blocks.iter().any(|b| b.hit));
        for error in [
            "UnknownMeasurement",
            "ReportDataMismatch",
            "VendorSignature",
        ] {
            let mut event = run
                .events
                .iter()
                .find(|e| e.kind == "attestation")
                .unwrap()
                .clone();
            event.data.insert("result".into(), error.into());
            let mut watch = Watch::default();
            watch.apply(&event).unwrap();
            assert_eq!(watch.attestation.result, error);
        }
        let honest = run.snapshot(5).unwrap();
        assert!(
            honest
                .watch
                .clients
                .values()
                .all(|(result, _)| result == "ok")
        );
        assert!(honest.watch.alarm.is_none());
    }
}
