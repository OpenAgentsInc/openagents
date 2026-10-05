//! Host-created character transactions with retained retry receipts.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "service-auth")]
pub(super) mod history;
#[cfg(feature = "service-auth")]
pub(super) const ACTIVE_RECEIPTS: usize = 128;
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub claimed_quests: BTreeSet<u64>,
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub revision: u64,
    pub transaction: Transaction,
}

#[derive(Clone, Default)]
pub(super) struct Ledger {
    characters: BTreeMap<u64, Character>,
    receipts: Vec<Receipt>,
    revision: u64,
    #[cfg(feature = "service-auth")]
    archive: Option<history::History>,
    #[cfg(feature = "service-auth")]
    root: history::Root,
    #[cfg(feature = "service-auth")]
    books: Option<BTreeMap<u64, books::Book>>,
    #[cfg(feature = "service-auth")]
    legacy_revision: Option<u64>,
}

#[cfg(feature = "service-auth")]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    characters: BTreeMap<u64, Character>,
    revision: u64,
    root: history::Root,
    receipts: Vec<Receipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    books: Option<BTreeMap<u64, books::Book>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legacy_revision: Option<u64>,
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
#[cfg(feature = "service-auth")]
mod books;
#[cfg(feature = "service-auth")]
impl Checkpoint {
    pub(super) fn realm_books(&self) -> bool {
        self.books.is_some()
    }
}
impl Ledger {
    pub(super) fn receipt(&self, actor: u64, source: [u8; 32]) -> Result<Option<Receipt>, String> {
        #[cfg(feature = "service-auth")]
        if self.books.is_some() {
            return self.book_receipt(actor, source);
        }
        if let Some(receipt) = self
            .receipts
            .iter()
            .find(|r| r.transaction.actor == actor && r.transaction.source == source)
        {
            return Ok(Some(receipt.clone()));
        }
        #[cfg(feature = "service-auth")]
        if let Some(archive) = &self.archive {
            return archive.get(self.root, actor, source);
        }
        Ok(None)
    }
    pub(super) fn contains(&self, actor: u64, source: [u8; 32]) -> Result<bool, String> {
        self.receipt(actor, source).map(|r| r.is_some())
    }
    pub(super) fn character_revision(&self, actor: u64) -> u64 {
        #[cfg(feature = "service-auth")]
        if let Some(book) = self.books.as_ref().and_then(|books| books.get(&actor)) {
            return book.revision;
        }
        let _ = actor;
        self.revision
    }
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }
    #[cfg(feature = "service-auth")]
    pub(super) fn history_capacity(&self) -> Result<bool, String> {
        self.archive
            .as_ref()
            .map_or(Ok(true), history::History::has_capacity)
    }
    #[cfg(feature = "service-auth")]
    pub(super) fn attach(&mut self, archive: history::History) -> Result<(), String> {
        if let Some(current) = &self.archive {
            if !current.same_directory(&archive) {
                return Err("Reward ledger belongs to another history directory".into());
            }
            return Ok(());
        }
        let mut root = None;
        for receipts in self.receipts.chunks(ACTIVE_RECEIPTS) {
            root = archive.insert(root, receipts)?;
        }
        self.archive = Some(archive);
        self.root = root;
        self.receipts.clear();
        Ok(())
    }
    #[cfg(feature = "service-auth")]
    pub(super) fn checkpoint(&self) -> Option<Checkpoint> {
        self.archive.as_ref().map(|_| Checkpoint {
            characters: self.characters.clone(),
            revision: self.revision,
            root: self.root,
            receipts: self.receipts.clone(),
            books: self.books.clone(),
            legacy_revision: self.legacy_revision,
        })
    }
    #[cfg(feature = "service-auth")]
    pub(super) fn restore(
        saved: Checkpoint,
        archive: Option<history::History>,
    ) -> Result<Self, String> {
        if saved.characters.len() > MAX_CHARACTERS || saved.receipts.len() > ACTIVE_RECEIPTS {
            return Err("Saved reward ledger budget exceeded".into());
        }
        if saved.books.is_some() != saved.legacy_revision.is_some()
            || (saved.books.is_some()
                && (!saved.receipts.is_empty()
                    || saved.legacy_revision.is_some_and(|r| r > saved.revision)))
        {
            return Err("Saved realm and legacy receipt lanes are inconsistent".into());
        }
        let archived = saved
            .legacy_revision
            .unwrap_or(saved.revision)
            .checked_sub(saved.receipts.len() as u64)
            .ok_or("Saved reward revisions are incompatible")?;
        if let Some(archive) = &archive {
            archive.validate(saved.root, archived)?;
        } else if archived != 0 || saved.root.is_some() {
            return Err("Saved chamber requires its reward history directory".into());
        }
        for (actor, character) in &saved.characters {
            if *actor == 0
                || character.items.len() > MAX_ENTRIES
                || character.quests.len() > MAX_ENTRIES
                || character.accepted_quests.len() > MAX_ENTRIES
                || character.claimed_quests.len() > MAX_ENTRIES
                || character.claimed_quests.contains(&0)
                || character
                    .items
                    .iter()
                    .chain(&character.quests)
                    .any(|(id, count)| *id == 0 || *count == 0 || *count > MAX_COUNT)
                || character
                    .accepted_quests
                    .iter()
                    .any(|(id, count)| *id == 0 || *count > MAX_COUNT)
                || (character.outfit != 0 && !character.items.contains_key(&character.outfit))
                || character
                    .equipment
                    .values()
                    .any(|id| !character.items.contains_key(id))
            {
                return Err("Saved reward character is invalid".into());
            }
        }
        let mut sources = BTreeSet::new();
        for (index, receipt) in saved.receipts.iter().enumerate() {
            if receipt.revision != archived + index as u64 + 1
                || !saved.characters.contains_key(&receipt.transaction.actor)
                || !sources.insert((receipt.transaction.actor, receipt.transaction.source))
                || archive
                    .as_ref()
                    .map(|a| {
                        a.get(
                            saved.root,
                            receipt.transaction.actor,
                            receipt.transaction.source,
                        )
                    })
                    .transpose()?
                    .flatten()
                    .is_some()
            {
                return Err("Saved reward receipt identity is invalid".into());
            }
            validate_transaction(&receipt.transaction)?;
        }
        if let Some(books) = &saved.books {
            books::validate(
                books,
                &saved.characters,
                archive
                    .as_ref()
                    .ok_or("Saved realm requires receipt history")?,
            )?;
        }
        Ok(Self {
            characters: saved.characters,
            receipts: saved.receipts,
            revision: saved.revision,
            archive,
            root: saved.root,
            books: saved.books,
            legacy_revision: saved.legacy_revision,
        })
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
    #[cfg(feature = "service-auth")]
    pub(super) fn actors(&self) -> impl Iterator<Item = u64> + '_ {
        self.characters.keys().copied()
    }
    pub(super) fn transactions(&self) -> Vec<Transaction> {
        self.receipts
            .iter()
            .map(|receipt| receipt.transaction.clone())
            .collect()
    }
    pub(super) fn apply(&mut self, transaction: Transaction) -> Result<Receipt, String> {
        validate_transaction(&transaction)?;
        if let Some(receipt) = self.receipt(transaction.actor, transaction.source)? {
            let equal = receipt.transaction == transaction;
            #[cfg(feature = "service-auth")]
            let equal = if self.books.is_some() {
                books::equivalent(&receipt.transaction, &transaction)
            } else {
                equal
            };
            return if equal {
                Ok(receipt)
            } else {
                Err("Reward source already binds a different transaction".into())
            };
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
            .unwrap_or_default();
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
        if transaction.source[..8] == *b"VQUEST01" {
            let quest = u64::from_be_bytes(transaction.source[16..24].try_into().unwrap());
            if quest == 0 || next.claimed_quests.len() >= MAX_ENTRIES {
                return Err("Claimed quest budget exceeded".into());
            }
            next.claimed_quests.insert(quest);
        }
        let receipt = Receipt {
            revision: self
                .revision
                .checked_add(1)
                .ok_or("Reward revisions exhausted")?,
            transaction,
        };
        #[cfg(feature = "service-auth")]
        if self.books.is_some() {
            return self.commit_book(
                receipt.transaction.actor,
                receipt.transaction.source,
                receipt,
                next,
            );
        }
        #[cfg(feature = "service-auth")]
        if let Some(archive) = &self.archive {
            if self.receipts.len() >= ACTIVE_RECEIPTS {
                self.root = archive.insert(self.root, &self.receipts)?;
                self.receipts.clear();
            }
        }
        self.characters.insert(receipt.transaction.actor, next);
        self.revision = receipt.revision;
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

fn validate_transaction(transaction: &Transaction) -> Result<(), String> {
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
    entries(&transaction.spent)
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
    fn lifetime_transaction_count_does_not_exhaust_the_ledger() {
        let mut ledger = Ledger::default();
        for n in 1..=4096 {
            let mut tx = transaction(1);
            tx.source[..8].copy_from_slice(&(n as u64).to_be_bytes());
            ledger.apply(tx).unwrap();
        }
        let mut next = transaction(2);
        next.source[..8].copy_from_slice(&4097u64.to_be_bytes());
        assert_eq!(ledger.apply(next).unwrap().revision, 4097);
        let original = ledger.receipts[0].clone();
        assert_eq!(
            ledger.apply(original.transaction.clone()).unwrap(),
            original
        );
    }
    #[cfg(feature = "service-auth")]
    #[test]
    fn archived_mixed_operations_retain_retries_and_bounded_checkpoints() {
        let dir = tempfile::tempdir().unwrap();
        let archive = history::History::open(&dir.path().join("history")).unwrap();
        let mut ledger = Ledger::default();
        ledger.attach(archive.clone()).unwrap();
        let quest = super::super::progression::Quest {
            dialogue: None,
            giver: Some(2),
            prerequisites: vec![],
            id: 1,
            name: "First quest".into(),
            objective: 3,
            goal: 1,
            experience: 1,
            items: vec![],
        };
        let acceptance = ledger.apply(quest.acceptance(4, 10, 0)).unwrap();
        let mut original = None;
        for n in 1u64..=5000 {
            let mut tx = transaction(1);
            tx.source[..8].copy_from_slice(&n.to_be_bytes());
            tx.experience = 1;
            tx.items = vec![Entry { id: 1, count: 1 }, Entry { id: 2, count: 1 }];
            match n % 5 {
                1 => {}
                2 => {
                    tx.items.clear();
                    tx.spent = vec![Entry { id: 1, count: 1 }];
                }
                3 => {
                    tx.items.clear();
                    tx.equipment = Some(super::super::equipment::Change {
                        slot: super::super::equipment::Slot::MainHand,
                        item: 2,
                    });
                }
                4 => {
                    tx.items.clear();
                    tx.outfit = Some(2);
                }
                _ => {
                    tx.items.clear();
                    tx.equipment = Some(super::super::equipment::Change {
                        slot: super::super::equipment::Slot::MainHand,
                        item: 0,
                    });
                }
            }
            let receipt = ledger.apply(tx).unwrap();
            original.get_or_insert(receipt);
            assert!(ledger.receipts.len() <= ACTIVE_RECEIPTS);
        }
        let claim = ledger.apply(quest.transaction(4, 10)).unwrap();
        let original = original.unwrap();
        let character = ledger.character(10).unwrap().clone();
        assert_eq!(character.experience, 5001);
        assert_eq!(character.quests[&3], 5000);
        assert_eq!(character.items.get(&1), None);
        assert_eq!(character.items[&2], 1000);
        assert_eq!(character.outfit, 2);
        assert_eq!(character.accepted_quests[&1], 0);
        assert!(character.claimed_quests.contains(&1));
        let bytes = serde_json::to_vec(&ledger.checkpoint().unwrap()).unwrap();
        assert!(bytes.len() < 64 * 1024);
        let mut recovered =
            Ledger::restore(serde_json::from_slice(&bytes).unwrap(), Some(archive)).unwrap();
        assert_eq!(recovered.character(10), Some(&character));
        assert_eq!(
            recovered.apply(acceptance.transaction.clone()).unwrap(),
            acceptance
        );
        assert_eq!(recovered.apply(claim.transaction.clone()).unwrap(), claim);
        assert_eq!(
            recovered.apply(original.transaction.clone()).unwrap(),
            original
        );
        let mut conflict = original.transaction;
        conflict.experience += 1;
        assert!(recovered.apply(conflict).is_err());
        assert_eq!(recovered.character(10), Some(&character));
        assert_eq!(recovered.revision(), 5002);
    }
    #[cfg(feature = "service-auth")]
    #[test]
    fn abandoned_archive_nodes_cannot_apply_a_failed_batch() {
        let dir = tempfile::tempdir().unwrap();
        let archive = history::History::open(&dir.path().join("history")).unwrap();
        let mut ledger = Ledger::default();
        ledger.attach(archive.clone()).unwrap();
        let mut maximum = transaction(1);
        maximum.actor = 11;
        maximum.experience = u64::MAX;
        ledger.apply(maximum).unwrap();
        for n in 1u64..ACTIVE_RECEIPTS as u64 {
            let mut tx = transaction(1);
            tx.source[..8].copy_from_slice(&n.to_be_bytes());
            ledger.apply(tx).unwrap();
        }
        let bytes = serde_json::to_vec(&ledger.checkpoint().unwrap()).unwrap();
        let first = transaction(2);
        let mut overflow = first.clone();
        overflow.actor = 11;
        assert!(ledger.batch(vec![first.clone(), overflow]).is_err());
        assert_eq!(
            serde_json::to_vec(&ledger.checkpoint().unwrap()).unwrap(),
            bytes
        );
        let mut recovered =
            Ledger::restore(serde_json::from_slice(&bytes).unwrap(), Some(archive)).unwrap();
        assert!(!recovered.contains(first.actor, first.source).unwrap());
        assert_eq!(
            recovered.apply(first).unwrap().revision,
            ACTIVE_RECEIPTS as u64 + 1
        );
    }
    #[cfg(feature = "service-auth")]
    #[test]
    fn failed_archive_write_preserves_balances_revisions_and_retry_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history");
        let mut ledger = Ledger::default();
        ledger
            .attach(history::History::open(&path).unwrap())
            .unwrap();
        for n in 1..=ACTIVE_RECEIPTS {
            let mut tx = transaction(1);
            tx.source[..8].copy_from_slice(&(n as u64).to_be_bytes());
            ledger.apply(tx).unwrap();
        }
        let before = serde_json::to_vec(&ledger.checkpoint().unwrap()).unwrap();
        std::fs::rename(&path, dir.path().join("original")).unwrap();
        std::fs::write(&path, b"injected storage failure").unwrap();
        let next = transaction(2);
        assert!(ledger.apply(next.clone()).is_err());
        assert_eq!(
            serde_json::to_vec(&ledger.checkpoint().unwrap()).unwrap(),
            before
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(dir.path().join("original"), path).unwrap();
        assert_eq!(
            ledger.apply(next).unwrap().revision,
            ACTIVE_RECEIPTS as u64 + 1
        );
    }
}
