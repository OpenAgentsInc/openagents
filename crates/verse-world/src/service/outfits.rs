//! Owned whole-outfit selection, separate from combat identity and resources.
use super::rewards::Transaction;
use serde::{Deserialize, Serialize};
const DOMAIN: &[u8; 8] = b"VOUTFIT1";
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outfit {
    pub id: u64,
    pub name: String,
    pub model: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u16,
    pub outfits: Vec<Outfit>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            outfits: vec![],
        }
    }
}
pub fn model_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.outfits.len() > 64 {
            return Err("Invalid outfit catalog version or budget".into());
        }
        let mut previous = 0;
        for outfit in &self.outfits {
            if outfit.id <= previous
                || outfit.name.trim().is_empty()
                || outfit.name.len() > 80
                || !outfit.name.bytes().all(|c| (32..=126).contains(&c))
                || !model_name(&outfit.model)
            {
                return Err("Invalid outfit definition".into());
            }
            previous = outfit.id;
        }
        Ok(())
    }
    pub fn outfit(&self, id: u64) -> Result<&Outfit, String> {
        self.outfits
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| "Outfit is not defined".into())
    }
    pub fn validate_items(&self, items: &super::items::Catalog) -> Result<(), String> {
        self.validate()?;
        items.validate()?;
        if self
            .outfits
            .iter()
            .any(|o| items.items.iter().any(|i| i.id == o.id))
        {
            return Err("Recovery items and outfits require distinct IDs".into());
        }
        Ok(())
    }
    pub(super) fn validate_change(&self, tx: &Transaction) -> Result<(), String> {
        if !reserved(&tx.source)
            || tx.source[8..16] != tx.instance.to_be_bytes()
            || tx.source[16..] == [0; 16]
            || tx.equipment.is_some()
            || tx.experience != 0
            || !tx.items.is_empty()
            || !tx.quests.is_empty()
            || !tx.spent.is_empty()
        {
            return Err("Invalid saved outfit change".into());
        }
        let id = tx.outfit.ok_or("Outfit change is missing its selection")?;
        if id != 0 {
            self.outfit(id)?;
        }
        Ok(())
    }
}
pub(super) fn reserved(source: &[u8; 32]) -> bool {
    source[..8] == DOMAIN[..]
}
pub(super) fn transaction(
    instance: u64,
    actor: u64,
    outfit: u64,
    operation: [u8; 16],
) -> Result<Transaction, String> {
    if operation == [0; 16] {
        return Err("Outfit change requires a nonzero retry identity".into());
    }
    let mut source = [0; 32];
    source[..8].copy_from_slice(DOMAIN);
    source[8..16].copy_from_slice(&instance.to_be_bytes());
    source[16..].copy_from_slice(&operation);
    Ok(Transaction {
        instance,
        actor,
        source,
        experience: 0,
        items: vec![],
        quests: vec![],
        spent: vec![],
        outfit: Some(outfit),
        equipment: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_refuse_ambiguous_ids_models_and_consumable_overlap() {
        let good = Catalog {
            version: 1,
            outfits: vec![Outfit {
                id: 2,
                name: "Ranger outfit".into(),
                model: "universal-male-ranger".into(),
            }],
        };
        good.validate().unwrap();
        for case in 0..6 {
            let mut bad = good.clone();
            match case {
                0 => bad.version = 2,
                1 => bad.outfits[0].id = 0,
                2 => bad.outfits.push(bad.outfits[0].clone()),
                3 => bad.outfits[0].model = "../../model".into(),
                4 => bad.outfits[0].name = "bad\nname".into(),
                _ => bad.outfits[0].model = "".into(),
            };
            assert!(bad.validate().is_err());
        }
        let items = super::super::items::Catalog {
            version: 1,
            items: vec![super::super::items::Item {
                id: 2,
                name: "Ember".into(),
                health: 1,
                mana: 0,
            }],
        };
        assert!(good.validate_items(&items).is_err());
    }
}
