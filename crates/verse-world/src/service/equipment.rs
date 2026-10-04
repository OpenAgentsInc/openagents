//! Owned equipment definitions, slot changes, and derived resource limits.
use super::rewards::{Character, Transaction};
use serde::{Deserialize, Serialize};
const DOMAIN: &[u8; 8] = b"VGEAR001";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Head,
    MainHand,
}
impl Slot {
    pub fn name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::MainHand => "Main hand",
        }
    }
    pub const ALL: [Self; 2] = [Self::Head, Self::MainHand];
    pub fn socket(self) -> u16 {
        match self {
            Self::Head => 5,
            Self::MainHand => 6,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gear {
    pub id: u64,
    pub name: String,
    pub slot: Slot,
    pub model: String,
    /// Source-space translation in millimeters, before the parent transform.
    pub offset: [i32; 3],
    pub health: u32,
    pub mana: u32,
}
impl Gear {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || !self.name.bytes().all(|c| (32..=126).contains(&c))
            || !super::outfits::model_name(&self.model)
            || self.offset.iter().any(|v| !(-2000..=2000).contains(v))
            || self.health > 200
            || self.mana > 20
        {
            return Err("Invalid equipment definition".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u16,
    pub gear: Vec<Gear>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            gear: vec![],
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub slot: Slot,
    pub item: u64,
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.gear.len() > 64 {
            return Err("Invalid equipment catalog version or budget".into());
        }
        let mut previous = 0;
        for g in &self.gear {
            g.validate()?;
            if g.id <= previous {
                return Err("Equipment IDs must be sorted and unique".into());
            }
            previous = g.id;
        }
        Ok(())
    }
    pub fn validate_catalogs(
        &self,
        items: &super::items::Catalog,
        outfits: &super::outfits::Catalog,
    ) -> Result<(), String> {
        self.validate()?;
        outfits.validate_items(items)?;
        if self.gear.iter().any(|g| {
            items.items.iter().any(|i| i.id == g.id) || outfits.outfits.iter().any(|o| o.id == g.id)
        }) {
            return Err("Equipment, recovery items, and outfits require distinct IDs".into());
        }
        Ok(())
    }
    pub fn item(&self, id: u64) -> Result<&Gear, String> {
        self.gear
            .iter()
            .find(|g| g.id == id)
            .ok_or_else(|| "Equipment item is not defined".into())
    }
    pub fn limits(&self, character: &Character) -> Result<(i32, i32), String> {
        let (mut hp, mut mana) = (200, 20);
        for (&slot, &item) in &character.equipment {
            let g = self.item(item)?;
            if g.slot != slot || character.items.get(&item).copied().unwrap_or(0) == 0 {
                return Err("Equipment selection is foreign or unowned".into());
            }
            hp += g.health as i32;
            mana += g.mana as i32;
        }
        Ok((hp, mana))
    }
    pub(super) fn validate_change(&self, tx: &Transaction) -> Result<(), String> {
        if !reserved(&tx.source)
            || tx.source[8..16] != tx.instance.to_be_bytes()
            || tx.source[16..] == [0; 16]
            || tx.experience != 0
            || !tx.items.is_empty()
            || !tx.quests.is_empty()
            || !tx.spent.is_empty()
            || tx.outfit.is_some()
        {
            return Err("Invalid saved equipment change".into());
        }
        let c = tx
            .equipment
            .ok_or("Equipment change is missing its selection")?;
        if c.item != 0 && self.item(c.item)?.slot != c.slot {
            return Err("Equipment does not fit the selected slot".into());
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
    slot: Slot,
    item: u64,
    operation: [u8; 16],
) -> Result<Transaction, String> {
    if operation == [0; 16] {
        return Err("Equipment change requires a nonzero retry identity".into());
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
        outfit: None,
        equipment: Some(Change { slot, item }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_and_saved_changes_refuse_ambiguous_or_unbounded_gear() {
        let good = Catalog {
            version: 1,
            gear: vec![Gear {
                id: 1,
                name: "Ritual hat".into(),
                slot: Slot::Head,
                model: "gear-hat".into(),
                offset: [0; 3],
                health: 100,
                mana: 10,
            }],
        };
        good.validate().unwrap();
        for case in 0..9 {
            let mut bad = good.clone();
            match case {
                0 => bad.version = 2,
                1 => bad.gear[0].id = 0,
                2 => bad.gear.push(bad.gear[0].clone()),
                3 => bad.gear[0].model = "../model".into(),
                4 => bad.gear[0].name = "bad\nname".into(),
                5 => bad.gear[0].health = 201,
                6 => bad.gear[0].mana = 21,
                7 => bad.gear[0].offset = [i32::MIN; 3],
                _ => bad.gear.resize(65, bad.gear[0].clone()),
            }
            assert!(bad.validate().is_err());
        }
        assert!(
            good.validate_catalogs(
                &super::super::items::Catalog {
                    version: 1,
                    items: vec![super::super::items::Item {
                        id: 1,
                        name: "Ember".into(),
                        health: 1,
                        mana: 0
                    }]
                },
                &Default::default()
            )
            .is_err()
        );
        let tx = transaction(7, 9, Slot::Head, 1, [1; 16]).unwrap();
        good.validate_change(&tx).unwrap();
        for case in 0..6 {
            let mut bad = tx.clone();
            match case {
                0 => bad.instance = 8,
                1 => bad.source[16..].fill(0),
                2 => bad.outfit = Some(1),
                3 => bad.experience = 1,
                4 => bad.equipment.as_mut().unwrap().slot = Slot::MainHand,
                _ => bad.equipment = None,
            };
            assert!(good.validate_change(&bad).is_err());
        }
    }
}
