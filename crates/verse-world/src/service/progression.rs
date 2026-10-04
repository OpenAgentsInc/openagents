//! Versioned campaign quests and experience levels owned by the chamber host.
use super::rewards::{Character, Entry, Ledger, Transaction};
use serde::{Deserialize, Serialize};
const CLAIM_DOMAIN: &[u8; 8] = b"VQUEST01";
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quest {
    pub id: u64,
    pub name: String,
    pub objective: u64,
    pub goal: u32,
    pub experience: u64,
    pub items: Vec<Entry>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u16,
    pub levels: Vec<u64>,
    pub quests: Vec<Quest>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            levels: vec![0],
            quests: vec![],
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub level: u16,
    pub start: u64,
    pub next: Option<u64>,
}
impl Level {
    pub fn validate(&self, experience: u64) -> Result<(), String> {
        if !(1..=100).contains(&self.level)
            || (self.level == 1) != (self.start == 0)
            || experience < self.start
            || self
                .next
                .is_some_and(|next| next <= experience || next <= self.start)
        {
            return Err("Invalid character experience level".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub id: u64,
    pub name: String,
    pub progress: u32,
    pub goal: u32,
    pub claimed: bool,
    pub experience: u64,
    pub items: Vec<Entry>,
}
fn name(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 80 && text.bytes().all(|c| (32..=126).contains(&c))
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.levels.is_empty()
            || self.levels.len() > 100
            || self.levels[0] != 0
            || self.levels.windows(2).any(|w| w[0] >= w[1])
            || self.quests.len() > 64
        {
            return Err("Invalid campaign progression version or budget".into());
        }
        let mut previous = 0;
        for quest in &self.quests {
            if quest.id <= previous
                || !name(&quest.name)
                || quest.objective == 0
                || !(1..=1_000_000).contains(&quest.goal)
                || (quest.experience == 0 && quest.items.is_empty())
            {
                return Err("Invalid campaign quest definition".into());
            }
            super::rewards::entries(&quest.items)?;
            previous = quest.id;
        }
        Ok(())
    }
    pub fn level(&self, experience: u64) -> Result<Level, String> {
        self.validate()?;
        let index = self
            .levels
            .partition_point(|threshold| *threshold <= experience)
            .saturating_sub(1);
        Ok(Level {
            level: index as u16 + 1,
            start: self.levels[index],
            next: self.levels.get(index + 1).copied(),
        })
    }
    pub(super) fn progress(&self, actor: u64, instance: u64, ledger: &Ledger) -> Vec<Progress> {
        self.quests
            .iter()
            .map(|quest| Progress {
                id: quest.id,
                name: quest.name.clone(),
                progress: quest.count(ledger.character(actor)).min(quest.goal),
                goal: quest.goal,
                claimed: ledger.contains(actor, quest.transaction(instance, actor).source),
                experience: quest.experience,
                items: quest.items.clone(),
            })
            .collect()
    }
    pub(super) fn validate_claim(
        &self,
        transaction: &Transaction,
        ledger: &Ledger,
    ) -> Result<(), String> {
        let id = u64::from_be_bytes(transaction.source[16..24].try_into().unwrap());
        let quest = self
            .quests
            .iter()
            .find(|quest| quest.id == id)
            .ok_or("Claimed campaign quest is not defined")?;
        if transaction != &quest.transaction(transaction.instance, transaction.actor)
            || quest.count(ledger.character(transaction.actor)) < quest.goal
        {
            return Err(
                "Campaign claim does not match its definition or completed objective".into(),
            );
        }
        Ok(())
    }
}
impl Quest {
    pub(super) fn count(&self, character: Option<&Character>) -> u32 {
        character
            .and_then(|c| c.quests.get(&self.objective))
            .copied()
            .unwrap_or(0)
    }
    pub(super) fn transaction(&self, instance: u64, actor: u64) -> Transaction {
        let mut source = [0; 32];
        source[..8].copy_from_slice(CLAIM_DOMAIN);
        source[8..16].copy_from_slice(&instance.to_be_bytes());
        source[16..24].copy_from_slice(&self.id.to_be_bytes());
        Transaction {
            outfit: None,
            equipment: None,
            spent: vec![],
            instance,
            actor,
            source,
            experience: self.experience,
            items: self.items.clone(),
            quests: vec![],
        }
    }
}
pub(super) fn reserved(source: &[u8; 32]) -> bool {
    source[..8] == CLAIM_DOMAIN[..]
}
pub fn validate_progress(values: &[Progress]) -> Result<(), String> {
    if values.len() > 64 {
        return Err("Campaign quest presentation budget exceeded".into());
    }
    let mut previous = 0;
    for quest in values {
        if quest.id <= previous
            || !name(&quest.name)
            || !(1..=1_000_000).contains(&quest.goal)
            || quest.progress > quest.goal
            || (quest.claimed && quest.progress != quest.goal)
            || (quest.experience == 0 && quest.items.is_empty())
        {
            return Err("Invalid campaign quest presentation".into());
        }
        super::rewards::entries(&quest.items)?;
        previous = quest.id;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_levels_and_quest_definitions_enforce_boundaries() {
        let config = Config {
            version: 1,
            levels: vec![0, 100, 300],
            quests: vec![Quest {
                id: 1,
                name: "Disrupt the summoning".into(),
                objective: 1,
                goal: 3,
                experience: 75,
                items: vec![],
            }],
        };
        config.validate().unwrap();
        for (xp, level, start, next) in [
            (0, 1, 0, Some(100)),
            (99, 1, 0, Some(100)),
            (100, 2, 100, Some(300)),
            (299, 2, 100, Some(300)),
            (300, 3, 300, None),
            (u64::MAX, 3, 300, None),
        ] {
            let result = config.level(xp).unwrap();
            assert_eq!(result, Level { level, start, next });
            result.validate(xp).unwrap();
        }
        for case in 0..7 {
            let mut bad = config.clone();
            match case {
                0 => bad.version = 2,
                1 => bad.levels = vec![],
                2 => bad.levels = vec![1],
                3 => bad.levels = vec![0, 100, 100],
                4 => bad.quests[0].goal = 0,
                5 => bad.quests[0].name = "\n".into(),
                _ => bad.quests.push(bad.quests[0].clone()),
            }
            assert!(bad.validate().is_err());
        }
    }
}
