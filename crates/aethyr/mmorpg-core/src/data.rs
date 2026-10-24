use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    #[default]
    Paladin,
    Sorceror,
    Zombie,
    Warrok,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stats {
    pub health: f32,
    pub max_health: f32,
    pub strength: u32,
    pub wisdomness: u32,
    pub benchpress: u32,
    pub curl: u32,
    pub experience: u64,
    pub level: u32,
}
impl Class {
    pub fn stats(self) -> Stats {
        let (health, max_health, strength, wisdomness, benchpress, curl) = match self {
            Self::Paladin => (200., 200., 50, 5, 20, 100),
            Self::Sorceror => (100., 100., 10, 200, 3, 17),
            Self::Zombie => (20., 50., 25, 4, 3, 20),
            Self::Warrok => (1000., 1000., 200, 4, 3, 200),
        };
        Stats {
            health,
            max_health,
            strength,
            wisdomness,
            benchpress,
            curl,
            experience: 0,
            level: 1,
        }
    }
    pub fn attack_hit(self) -> u16 {
        match self {
            Self::Paladin => 21,
            Self::Sorceror | Self::Zombie => 60,
            Self::Warrok => 90,
        }
    }
    pub fn attack_cooldown(self) -> u16 {
        match self {
            Self::Paladin => 60,
            Self::Sorceror => 90,
            Self::Zombie => 180,
            Self::Warrok => 156,
        }
    }
    pub fn attack_duration(self) -> u16 {
        // Measured from the source GLB animation input accessors, in 60 Hz ticks.
        match self {
            Self::Paladin => 92,
            Self::Sorceror => 198,
            Self::Zombie => 232,
            Self::Warrok => 162,
        }
    }
    pub fn dance_duration(self) -> u16 {
        match self {
            Self::Paladin => 288,
            Self::Sorceror => 496,
            _ => 1,
        }
    }
    pub fn range(self) -> f32 {
        match self {
            Self::Sorceror => 40.,
            Self::Warrok => 15.,
            _ => 10.,
        }
    }
    pub fn height(self) -> f32 {
        match self {
            Self::Zombie => 1.2,
            Self::Warrok => 2.4,
            _ => 1.8,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Paladin => "Paladin",
            Self::Sorceror => "Sorceror",
            Self::Zombie => "Zombie",
            Self::Warrok => "Monster Guy",
        }
    }
    pub fn is_player(self) -> bool {
        matches!(self, Self::Paladin | Self::Sorceror)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    #[serde(rename = "weapon.sword1")]
    Sword,
    #[serde(rename = "weapon.axe1")]
    Axe,
    #[serde(rename = "weapon.hammer1")]
    Hammer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    pub version: u64,
    pub slots: [Option<Item>; 32],
}
impl Inventory {
    pub fn new(class: Class) -> Self {
        let mut value = Self {
            version: 0,
            slots: [None; 32],
        };
        if class == Class::Paladin {
            value.slots[0] = Some(Item::Axe);
            value.slots[1] = Some(Item::Hammer);
            value.slots[24] = Some(Item::Sword);
        }
        value
    }
    pub fn swap(&mut self, version: u64, from: u8, to: u8) -> Result<(), &'static str> {
        if version != self.version {
            return Err("The inventory changed. Try that move again.");
        }
        if from >= 32 || to >= 32 || from == to {
            return Err("Choose two different inventory slots.");
        }
        self.slots.swap(from as usize, to as usize);
        self.version += 1;
        Ok(())
    }
}
impl Stats {
    pub fn damage(&self, class: Class, inventory: &Inventory) -> f32 {
        if class == Class::Sorceror {
            self.wisdomness as f32 / 10.0
        } else {
            self.strength as f32 / 5.0
                * if inventory.slots[24].is_some() {
                    30.0
                } else {
                    1.0
                }
        }
    }
    /// Source progression hook. No source gameplay awards XP automatically.
    pub fn add_experience(&mut self, amount: u32) -> bool {
        self.experience = self.experience.saturating_add(amount as u64);
        let threshold = 100u64.saturating_mul(
            1u64.checked_shl(self.level.saturating_sub(1))
                .unwrap_or(u64::MAX),
        );
        if self.experience < threshold {
            return false;
        }
        self.level = self.level.saturating_add(1);
        self.strength = self.strength.saturating_add(1);
        self.wisdomness = self.wisdomness.saturating_add(1);
        self.benchpress = self.benchpress.saturating_add(1);
        self.curl = self.curl.saturating_add(2);
        true
    }
}
