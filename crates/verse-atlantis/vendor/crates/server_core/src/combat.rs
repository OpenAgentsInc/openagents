//! Faction rules and hostility helpers.

use crate::actor::Faction;

#[inline]
pub fn are_hostile(a: Faction, b: Faction) -> bool {
    use Faction::*;
    matches!(
        (a, b),
        (Pc, Undead) | (Undead, Pc) | (Wizards, Undead) | (Undead, Wizards)
    )
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FactionState {
    pub pc_vs_wizards_hostile: bool,
}

impl FactionState {
    pub fn effective_hostile(&self, a: Faction, b: Faction) -> bool {
        use Faction::*;
        match (a, b) {
            (Pc, Wizards) | (Wizards, Pc) => self.pc_vs_wizards_hostile,
            _ => are_hostile(a, b),
        }
    }
}
