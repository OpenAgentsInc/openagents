//! Authored recovery items and stable, host-validated inventory debits.
use super::rewards::{Entry, Transaction};
use serde::{Deserialize, Serialize};
const DOMAIN: &[u8; 8] = b"VITEM001";
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub health: u32,
    pub mana: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u16,
    pub items: Vec<Item>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            items: vec![],
        }
    }
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.items.len() > 64 {
            return Err("Invalid item catalog version or budget".into());
        }
        let mut previous = 0;
        for item in &self.items {
            if item.id <= previous
                || item.name.trim().is_empty()
                || item.name.len() > 80
                || !item.name.bytes().all(|c| (32..=126).contains(&c))
                || item.health > 200
                || item.mana > 20
                || (item.health == 0 && item.mana == 0)
            {
                return Err("Invalid recovery item definition".into());
            }
            previous = item.id;
        }
        Ok(())
    }
    pub fn item(&self, id: u64) -> Result<&Item, String> {
        self.items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| "Recovery item is not defined".into())
    }
    pub(super) fn validate_use(&self, tx: &Transaction) -> Result<(), String> {
        if !reserved(&tx.source)
            || tx.source[8..16] != tx.instance.to_be_bytes()
            || tx.source[16..] == [0; 16]
            || tx.outfit.is_some()
            || tx.experience != 0
            || !tx.items.is_empty()
            || !tx.quests.is_empty()
            || tx.spent.len() != 1
            || tx.spent[0].count != 1
        {
            return Err("Invalid saved item use".into());
        }
        self.item(tx.spent[0].id)?;
        Ok(())
    }
}
pub(super) fn reserved(source: &[u8; 32]) -> bool {
    source[..8] == DOMAIN[..]
}
pub(super) fn transaction(
    instance: u64,
    actor: u64,
    item: u64,
    operation: [u8; 16],
) -> Result<Transaction, String> {
    if operation == [0; 16] {
        return Err("Item use requires a nonzero retry identity".into());
    }
    let mut source = [0; 32];
    source[..8].copy_from_slice(DOMAIN);
    source[8..16].copy_from_slice(&instance.to_be_bytes());
    source[16..].copy_from_slice(&operation);
    Ok(Transaction {
        outfit: None,
        instance,
        actor,
        source,
        experience: 0,
        items: vec![],
        quests: vec![],
        spent: vec![Entry { id: item, count: 1 }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_refuses_ambiguous_names_ids_and_unbounded_effects() {
        let good = Catalog {
            version: 1,
            items: vec![Item {
                id: 1,
                name: "Recovery ember".into(),
                health: 45,
                mana: 5,
            }],
        };
        good.validate().unwrap();
        for case in 0..8 {
            let mut bad = good.clone();
            match case {
                0 => bad.version = 2,
                1 => bad.items[0].id = 0,
                2 => bad.items.push(bad.items[0].clone()),
                3 => bad.items[0].name = " ".into(),
                4 => bad.items[0].name = "bad\nname".into(),
                5 => bad.items[0].health = 201,
                6 => bad.items[0].mana = 21,
                _ => {
                    bad.items[0].health = 0;
                    bad.items[0].mana = 0;
                }
            }
            assert!(bad.validate().is_err());
        }
    }
}
