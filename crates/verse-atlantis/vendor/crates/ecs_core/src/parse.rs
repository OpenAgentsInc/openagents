//! Parsers for string -> ECS enums for data-driven configs.

use crate::components::{Condition, DamageType};

/// Case-insensitive damage type parser.
pub fn parse_damage_type(s: &str) -> Option<DamageType> {
    Some(match s.to_ascii_lowercase().as_str() {
        "acid" => DamageType::Acid,
        "bludgeoning" => DamageType::Bludgeoning,
        "cold" => DamageType::Cold,
        "fire" => DamageType::Fire,
        "force" => DamageType::Force,
        "lightning" => DamageType::Lightning,
        "necrotic" => DamageType::Necrotic,
        "piercing" => DamageType::Piercing,
        "poison" => DamageType::Poison,
        "psychic" => DamageType::Psychic,
        "radiant" => DamageType::Radiant,
        "slashing" => DamageType::Slashing,
        "thunder" => DamageType::Thunder,
        _ => return None,
    })
}

/// Case-insensitive condition parser with common aliases.
pub fn parse_condition(s: &str) -> Option<Condition> {
    Some(match s.to_ascii_lowercase().as_str() {
        // canonical
        "blinded" => Condition::Blinded,
        "charmed" => Condition::Charmed,
        "deafened" => Condition::Deafened,
        "frightened" => Condition::Frightened,
        "grappled" => Condition::Grappled,
        "incapacitated" => Condition::Incapacitated,
        "invisible" => Condition::Invisible,
        "paralyzed" => Condition::Paralyzed,
        "petrified" => Condition::Petrified,
        "poisoned" => Condition::Poisoned,
        "prone" => Condition::Prone,
        "restrained" => Condition::Restrained,
        "stunned" => Condition::Stunned,
        "unconscious" => Condition::Unconscious,
        // aliases
        "fear" => Condition::Frightened,
        "charm" => Condition::Charmed,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn damage_parses() {
        assert!(parse_damage_type("necrotic").is_some());
        assert!(parse_damage_type("FoRcE").is_some());
        assert!(parse_damage_type("unknown").is_none());
    }
    #[test]
    fn condition_parses_with_alias() {
        assert!(parse_condition("frightened").is_some());
        assert!(parse_condition("fear").is_some());
        assert!(parse_condition("charm").is_some());
        assert!(parse_condition("none").is_none());
    }
}
