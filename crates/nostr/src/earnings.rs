//! Owner-selected, service-signed earnings receipts in NIP-78 application data.
//! Amounts describe one credit, never a wallet balance or a payout batch.
use crate::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const KIND: u16 = 30_078;
pub const SCHEMA: &str = "openagents.earnings.v1";
pub const MAX_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Pylon,
    Plugin,
    Tip,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    Earned,
    Paid,
    Reversed,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub v: String,
    pub party: String,
    /// A domain-separated hash of the private credit identity.
    pub source: String,
    pub category: Category,
    pub state: State,
    pub amount_msat: u64,
    pub network: String,
    /// When the service recorded this credit, in Unix seconds.
    pub recorded_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}
fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Receipt {
    pub fn check(&self) -> Result<(), String> {
        if self.v != SCHEMA
            || !hex64(&self.party)
            || !hex64(&self.source)
            || self.network != "bitcoin"
            || self.amount_msat == 0
            || self.amount_msat > 2_100_000_000_000_000_000
            || self.recorded_at == 0
            || self.evidence.as_ref().is_some_and(|id| !hex64(id))
            || [&self.plugin, &self.release].iter().any(|value| {
                value.as_ref().is_some_and(|value| {
                    value.is_empty() || value.len() > 512 || value.chars().any(char::is_control)
                })
            })
            || (self.category != Category::Plugin
                && (self.plugin.is_some() || self.release.is_some()))
        {
            return Err("invalid public earnings receipt".into());
        }
        Ok(())
    }
    pub fn event(&self, signer: &RelaySigner, at: u64) -> Result<Event, String> {
        self.check()?;
        let content = serde_json::to_string(self).map_err(|_| "receipt encoding failed")?;
        if content.len() > MAX_BYTES {
            return Err("public receipt exceeds its bound".into());
        }
        Ok(signer.sign(
            at,
            KIND,
            vec![
                Tag::new(vec!["d".into(), format!("{SCHEMA}:{}", self.source)]),
                Tag::new(vec!["t".into(), SCHEMA.into()]),
                Tag::new(vec!["p".into(), self.party.clone()]),
                Tag::new(vec!["alt".into(), "Earnings receipt".into()]),
            ],
            content,
        ))
    }
}
pub fn read(event: &Event) -> Result<Receipt, String> {
    if event.kind != KIND || event.content.len() > MAX_BYTES || event.validate_crypto().is_err() {
        return Err("invalid signed earnings receipt".into());
    }
    let receipt: Receipt =
        serde_json::from_str(&event.content).map_err(|_| "invalid earnings receipt content")?;
    receipt.check()?;
    let d = format!("{SCHEMA}:{}", receipt.source);
    for (name, expected) in [
        ("d", d.as_str()),
        ("t", SCHEMA),
        ("p", receipt.party.as_str()),
    ] {
        let values: Vec<_> = event.tag_values(name).collect();
        if values != vec![expected] {
            return Err("earnings receipt tags do not match its content".into());
        }
    }
    Ok(receipt)
}

/// Select one current receipt per private source, from explicitly trusted issuers.
pub fn selected(
    events: &[Event],
    party: &str,
    issuers: &BTreeSet<String>,
    now: u64,
) -> Vec<(Event, Receipt)> {
    let mut found: BTreeMap<String, (Event, Receipt)> = BTreeMap::new();
    for event in events {
        if !issuers.contains(&event.pubkey) || event.created_at > now.saturating_add(60) {
            continue;
        }
        let Ok(receipt) = read(event) else {
            continue;
        };
        if receipt.party != party {
            continue;
        }
        let replace = found.get(&receipt.source).is_none_or(|(old, _)| {
            event.created_at > old.created_at
                || (event.created_at == old.created_at && event.id < old.id)
        });
        if replace {
            found.insert(receipt.source.clone(), (event.clone(), receipt));
        }
    }
    let mut values: Vec<_> = found.into_values().collect();
    values.sort_by(|(a, _), (b, _)| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> Receipt {
        Receipt {
            v: SCHEMA.into(),
            party: "11".repeat(32),
            source: "22".repeat(32),
            category: Category::Plugin,
            state: State::Earned,
            amount_msat: 123000,
            network: "bitcoin".into(),
            recorded_at: 100,
            evidence: None,
            plugin: Some("plugin".into()),
            release: Some("release".into()),
        }
    }
    #[test]
    fn receipts_bind_the_payee_amount_and_source_and_keep_only_the_latest_state() {
        let signer = RelaySigner::from_secret_hex(&"03".repeat(32)).unwrap();
        let mut receipt = receipt();
        let first = receipt.event(&signer, 101).unwrap();
        assert_eq!(read(&first).unwrap(), receipt);
        receipt.state = State::Paid;
        let paid = receipt.event(&signer, 102).unwrap();
        let trusted = BTreeSet::from([signer.pubkey().to_string()]);
        let selected = selected(
            &[first.clone(), paid.clone(), paid.clone()],
            &receipt.party,
            &trusted,
            103,
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].1.state, State::Paid);
        assert!(super::selected(&[paid.clone()], &receipt.party, &BTreeSet::new(), 103).is_empty());
        assert!(super::selected(&[paid.clone()], &"44".repeat(32), &trusted, 103).is_empty());
        let mut altered = paid;
        altered.content = altered.content.replace("123000", "999000");
        assert!(read(&altered).is_err());
        let mut tags = first.clone();
        tags.tags.push(Tag::new(vec!["p".into(), "44".repeat(32)]));
        let tags = signer.sign(103, KIND, tags.tags, tags.content);
        assert!(read(&tags).is_err());
    }
    #[test]
    fn private_fields_and_non_bitcoin_amounts_are_refused() {
        let mut value = serde_json::to_value(receipt()).unwrap();
        value["preimage"] = serde_json::json!("private");
        assert!(serde_json::from_value::<Receipt>(value).is_err());
        let mut invalid = receipt();
        invalid.network = "testnet".into();
        assert!(invalid.check().is_err());
        invalid.network = "bitcoin".into();
        invalid.amount_msat = 0;
        assert!(invalid.check().is_err());
    }
}
