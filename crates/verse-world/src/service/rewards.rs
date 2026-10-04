//! Host-created character transactions with retained retry receipts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(super) const MAX_TRANSACTIONS: usize = 4096;
const MAX_CHARACTERS: usize = 64;
const MAX_ENTRIES: usize = 64;
const MAX_COUNT: u32 = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: u64,
    pub count: u32,
}

/// Stable source identifies one authored outcome for one character across retries.
/// The host selects IDs and amounts; clients cannot submit this value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<super::progression::Acceptance>,
    pub instance: u64,
    pub actor: u64,
    pub source: [u8; 32],
    pub experience: u64,
    pub items: Vec<Entry>,
    pub quests: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spent: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outfit: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<super::equipment::Change>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Character {
    pub accepted_quests: BTreeMap<u64, u32>,
    pub experience: u64,
    pub items: BTreeMap<u64, u32>,
    pub quests: BTreeMap<u64, u32>,
    pub outfit: u64,
    pub equipment: BTreeMap<super::equipment::Slot, u64>,
}

/// Version-one cooperative reward for each defeated life of an authored NPC.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub target: u64,
    pub experience: u64,
    pub items: Vec<Entry>,
    pub quests: Vec<Entry>,
}
impl Policy {
    pub fn validate(policies: &[Self]) -> Result<(), String> {
        if policies.len() > 64 {
            return Err("Combat reward policy budget exceeded".into());
        }
        let mut previous = 0;
        for policy in policies {
            if policy.target <= previous
                || (policy.experience == 0 && policy.items.is_empty() && policy.quests.is_empty())
            {
                return Err(
                    "Combat rewards require sorted unique targets and nonempty grants".into(),
                );
            }
            entries(&policy.items)?;
            entries(&policy.quests)?;
            previous = policy.target;
        }
        Ok(())
    }
    pub(super) fn transaction(
        &self,
        recipient: u64,
        life: verse_engine::core::LifeId,
    ) -> Transaction {
        let mut source = [0; 32];
        source[..8].copy_from_slice(b"VREWARD1");
        source[8..16].copy_from_slice(&life.instance.to_be_bytes());
        source[16..24].copy_from_slice(&life.actor.to_be_bytes());
        source[24..].copy_from_slice(&life.generation.to_be_bytes());
        Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            spent: vec![],
            instance: life.instance,
            actor: recipient,
            source,
            experience: self.experience,
            items: self.items.clone(),
            quests: self.quests.clone(),
        }
    }
}

/// Exact original outcome, including the original ledger revision on retries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub revision: u64,
    pub transaction: Transaction,
}

#[derive(Clone, Default)]
pub(super) struct Ledger {
    characters: BTreeMap<u64, Character>,
    receipts: Vec<Receipt>,
}

pub(super) fn entries(entries: &[Entry]) -> Result<(), String> {
    if entries.len() > MAX_ENTRIES {
        return Err("Reward entry budget exceeded".into());
    }
    let mut previous = 0;
    for entry in entries {
        if entry.id <= previous || entry.count == 0 || entry.count > MAX_COUNT {
            return Err(
                "Reward entries require sorted unique IDs and bounded positive counts".into(),
            );
        }
        previous = entry.id;
    }
    Ok(())
}
fn add(target: &mut BTreeMap<u64, u32>, additions: &[Entry]) -> Result<(), String> {
    for entry in additions {
        let count = target
            .get(&entry.id)
            .copied()
            .unwrap_or(0)
            .checked_add(entry.count)
            .filter(|count| *count <= MAX_COUNT)
            .ok_or("Character reward count exceeded")?;
        target.insert(entry.id, count);
    }
    if target.len() > MAX_ENTRIES {
        return Err("Character reward entry budget exceeded".into());
    }
    Ok(())
}
impl Ledger {
    pub(super) fn contains(&self, actor: u64, source: [u8; 32]) -> bool {
        self.receipts
            .iter()
            .any(|r| r.transaction.actor == actor && r.transaction.source == source)
    }
    pub(super) fn revision(&self) -> u64 {
        self.receipts.len() as u64
    }
    pub(super) fn batch(&mut self, transactions: Vec<Transaction>) -> Result<(), String> {
        if transactions.is_empty() {
            return Ok(());
        }
        let mut next = self.clone();
        for transaction in transactions {
            next.apply(transaction)?;
        }
        *self = next;
        Ok(())
    }
    pub(super) fn character(&self, actor: u64) -> Option<&Character> {
        self.characters.get(&actor)
    }
    pub(super) fn transactions(&self) -> Vec<Transaction> {
        self.receipts
            .iter()
            .map(|receipt| receipt.transaction.clone())
            .collect()
    }
    pub(super) fn apply(&mut self, transaction: Transaction) -> Result<Receipt, String> {
        if transaction.instance == 0
            || transaction.actor == 0
            || transaction.source == [0; 32]
            || (transaction.experience == 0
                && transaction.items.is_empty()
                && transaction.quests.is_empty()
                && transaction.spent.is_empty()
                && transaction.outfit.is_none()
                && transaction.equipment.is_none()
                && transaction.acceptance.is_none())
        {
            return Err("Reward identity or grant is empty".into());
        }
        entries(&transaction.items)?;
        entries(&transaction.quests)?;
        entries(&transaction.spent)?;
        if let Some(receipt) = self.receipts.iter().find(|receipt| {
            receipt.transaction.actor == transaction.actor
                && receipt.transaction.source == transaction.source
        }) {
            return if receipt.transaction == transaction {
                Ok(receipt.clone())
            } else {
                Err("Reward source already binds a different transaction".into())
            };
        }
        if self.receipts.len() >= MAX_TRANSACTIONS {
            return Err("Reward transaction budget exceeded".into());
        }
        if !self.characters.contains_key(&transaction.actor)
            && self.characters.len() >= MAX_CHARACTERS
        {
            return Err("Reward character budget exceeded".into());
        }
        let mut next = self
            .characters
            .get(&transaction.actor)
            .cloned()
            .unwrap_or(Character {
                accepted_quests: BTreeMap::new(),
                experience: 0,
                items: BTreeMap::new(),
                quests: BTreeMap::new(),
                outfit: 0,
                equipment: BTreeMap::new(),
            });
        if let Some(acceptance) = transaction.acceptance {
            if acceptance.quest == 0
                || acceptance.baseline > MAX_COUNT
                || next.accepted_quests.contains_key(&acceptance.quest)
                || next.accepted_quests.len() >= MAX_ENTRIES
            {
                return Err("Invalid or repeated quest acceptance".into());
            }
            next.accepted_quests
                .insert(acceptance.quest, acceptance.baseline);
        }
        next.experience = next
            .experience
            .checked_add(transaction.experience)
            .ok_or("Character experience exceeded")?;
        add(&mut next.items, &transaction.items)?;
        add(&mut next.quests, &transaction.quests)?;
        for entry in &transaction.spent {
            let count = next
                .items
                .get(&entry.id)
                .copied()
                .unwrap_or(0)
                .checked_sub(entry.count)
                .ok_or("Not enough owned items")?;
            if count == 0 {
                next.items.remove(&entry.id);
            } else {
                next.items.insert(entry.id, count);
            }
        }
        if let Some(outfit) = transaction.outfit {
            if outfit != 0 && next.items.get(&outfit).copied().unwrap_or(0) == 0 {
                return Err("Outfit item is not owned".into());
            }
            next.outfit = outfit;
        }
        if let Some(change) = transaction.equipment {
            if change.item == 0 {
                next.equipment.remove(&change.slot);
            } else {
                if next.items.get(&change.item).copied().unwrap_or(0) == 0 {
                    return Err("Equipment item is not owned".into());
                }
                next.equipment.insert(change.slot, change.item);
            }
        }
        if next
            .equipment
            .values()
            .any(|id| next.items.get(id).copied().unwrap_or(0) == 0)
        {
            return Err("Cannot spend equipped gear".into());
        }
        if next.outfit != 0 && next.items.get(&next.outfit).copied().unwrap_or(0) == 0 {
            return Err("Cannot spend the equipped outfit".into());
        }
        let receipt = Receipt {
            revision: self.receipts.len() as u64 + 1,
            transaction,
        };
        self.characters.insert(receipt.transaction.actor, next);
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transaction(source: u8) -> Transaction {
        Transaction {
            acceptance: None,
            outfit: None,
            equipment: None,
            spent: vec![],
            instance: 4,
            actor: 10,
            source: [source; 32],
            experience: 45,
            items: vec![Entry { id: 1, count: 2 }],
            quests: vec![Entry { id: 3, count: 1 }],
        }
    }
    #[test]
    fn retries_return_original_receipts_and_conflicting_reuse_is_atomic() {
        let mut ledger = Ledger::default();
        let first = ledger.apply(transaction(1)).unwrap();
        ledger.apply(transaction(2)).unwrap();
        assert_eq!(ledger.apply(transaction(1)).unwrap(), first);
        let before = ledger.characters.clone();
        let mut conflict = transaction(1);
        conflict.experience += 1;
        assert!(ledger.apply(conflict).is_err());
        assert_eq!(ledger.characters, before);
        assert_eq!(ledger.character(10).unwrap().experience, 90);
        assert_eq!(ledger.character(10).unwrap().items[&1], 4);
        let mut other = transaction(1);
        other.actor = 11;
        assert_eq!(ledger.apply(other).unwrap().revision, 3);
        let mut restored = Ledger::default();
        for tx in ledger.transactions() {
            restored.apply(tx).unwrap();
        }
        assert_eq!(restored.characters, ledger.characters);
        assert_eq!(restored.apply(transaction(1)).unwrap(), first);
    }
    #[test]
    fn invalid_and_overflowing_grants_preserve_every_field() {
        let mut ledger = Ledger::default();
        let mut maximum = transaction(1);
        maximum.experience = u64::MAX;
        maximum.items[0].count = MAX_COUNT;
        ledger.apply(maximum).unwrap();
        let before = ledger.characters.clone();
        for case in 0..7 {
            let mut tx = transaction(2);
            tx.experience = 0;
            match case {
                0 => tx.experience = 1,
                1 => tx.items[0].count = 0,
                2 => tx.items.push(tx.items[0].clone()),
                3 => tx.source = [0; 32],
                4 => tx.items[0].id = 0,
                5 => tx.quests[0].count = MAX_COUNT + 1,
                _ => tx.items = (1..=65).map(|id| Entry { id, count: 1 }).collect(),
            }
            assert!(ledger.apply(tx).is_err());
            assert_eq!(ledger.characters, before);
            assert_eq!(ledger.receipts.len(), 1);
        }
        let mut tx = transaction(2);
        tx.experience = 0;
        tx.items = vec![Entry { id: 2, count: 3 }];
        tx.quests[0].count = MAX_COUNT;
        assert!(ledger.apply(tx).is_err());
        assert_eq!(ledger.characters, before);
    }
    #[test]
    fn cooperative_batch_failure_leaves_all_characters_and_receipts_unchanged() {
        let mut ledger = Ledger::default();
        let mut max = transaction(1);
        max.actor = 11;
        max.experience = u64::MAX;
        ledger.apply(max).unwrap();
        let before = ledger.characters.clone();
        let first = transaction(2);
        let mut second = first.clone();
        second.actor = 11;
        assert!(ledger.batch(vec![first, second]).is_err());
        assert_eq!(ledger.characters, before);
        assert_eq!(ledger.revision(), 1);
        assert!(ledger.character(10).is_none());
    }
    #[test]
    fn exhausted_receipts_refuse_new_sources_but_retain_exact_retries() {
        let mut ledger = Ledger::default();
        for n in 1..=MAX_TRANSACTIONS {
            let mut tx = transaction(1);
            tx.source[..8].copy_from_slice(&(n as u64).to_be_bytes());
            ledger.apply(tx).unwrap();
        }
        let mut next = transaction(2);
        next.source[..8].copy_from_slice(&((MAX_TRANSACTIONS + 1) as u64).to_be_bytes());
        assert!(ledger.apply(next).is_err());
        let original = ledger.receipts[0].clone();
        assert_eq!(
            ledger.apply(original.transaction.clone()).unwrap(),
            original
        );
    }
}
