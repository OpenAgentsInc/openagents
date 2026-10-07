//! The town's districts as data (`docs/verse/everglade.md`, the district
//! table): which district each of [`city::BUILDINGS`] and each generated
//! instance ([`city::instances`]) stands in, including the Civic Hall and
//! the owner's house. The world tree groups buildings under these
//! (`docs/verse/generative-agents.md`, item 3).
//!
//! A building or instance added without a district fails the tests below.

use super::city::{self, Building, STAND_INS};

/// A district of Everglade's town.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum District {
    Commons,
    MainStreet,
    FountainPlaza,
    CreativeDistrict,
    Foundry,
    KnowledgeDistrict,
    StoopLane,
    LanternQuarter,
    BrownstoneRow,
    WaldenWoods,
    Fernhollow,
    Gardens,
    Farm,
    /// The wild ground and trails past the town's last streets.
    Wilds,
}

impl District {
    pub const ALL: [Self; 14] = [
        Self::Commons,
        Self::MainStreet,
        Self::FountainPlaza,
        Self::CreativeDistrict,
        Self::Foundry,
        Self::KnowledgeDistrict,
        Self::StoopLane,
        Self::LanternQuarter,
        Self::BrownstoneRow,
        Self::WaldenWoods,
        Self::Fernhollow,
        Self::Gardens,
        Self::Farm,
        Self::Wilds,
    ];

    /// The district's display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Commons => "The Commons",
            Self::MainStreet => "Main Street",
            Self::FountainPlaza => "Fountain Plaza",
            Self::CreativeDistrict => "Creative District",
            Self::Foundry => "The Foundry",
            Self::KnowledgeDistrict => "Knowledge District",
            Self::StoopLane => "Stoop Lane",
            Self::LanternQuarter => "Lantern Quarter",
            Self::BrownstoneRow => "Brownstone Row",
            Self::WaldenWoods => "Walden Woods",
            Self::Fernhollow => "Fernhollow",
            Self::Gardens => "Gardens and orchards",
            Self::Farm => "The farm",
            Self::Wilds => "The wilds",
        }
    }
}

/// The district of the city building named `name`, one of
/// [`city::BUILDINGS`] or the beekeeper's hut.
#[must_use]
pub fn of_building(name: &str) -> Option<District> {
    use District::*;
    Some(match name {
        "corner shop" | "bakehouse" | "tailor" | "tea house" | "print shop" | "cheesemonger"
        | "music shop" | "hardware store" => MainStreet,
        n if n.starts_with("market row ") => MainStreet,
        "market hall" | "plaza cafe west" | "plaza cafe east" => FountainPlaza,
        n if n.starts_with("townhouse ") => StoopLane,
        "meeting hall" | "the lantern" | "music hall" | "the fiddle" | "guild hall"
        | "the hearth" | "choir house" | "the lamplighter" | "the snug" | "well house west"
        | "well house south" => LanternQuarter,
        n if n.starts_with("brownstone ") || n.starts_with("row house ") => BrownstoneRow,
        "prototype shed" | "quiet cabin" | "woodcutter's cottage" => WaldenWoods,
        "college hall" | "lecture hall" | "archive" | "seminar house" | "map room"
        | "scriptorium" | "observatory" | "sketch cabin" | "clock tower" => KnowledgeDistrict,
        "workshop" | "fab hall" | "server barn annex" | "forge" => Foundry,
        "studio" | "atelier" | "pottery" | "atelier hall" => CreativeDistrict,
        "lookout hut" | "fern cabin" => Fernhollow,
        "windmill" | "farmhouse" | "barn" => Farm,
        "beekeeper's hut" => Gardens,
        _ => return None,
    })
}

/// The district of the generated instance named `name`, one of
/// [`city::instances`]: a stand-in takes its building's district.
#[must_use]
pub fn of_instance(name: &str) -> Option<District> {
    if let Some(stand_in) = STAND_INS.iter().find(|s| s.name == name) {
        return of_building(stand_in.building);
    }
    Some(match name {
        "fountain" => District::FountainPlaza,
        "boathouse" | "gazebo" => District::Commons,
        // In the community garden behind Brownstone Row.
        "glasshouse" => District::BrownstoneRow,
        // Beside the north trail.
        "chapel" => District::Wilds,
        "owner's house" => District::KnowledgeDistrict,
        "civic hall" => District::MainStreet,
        // Over the Lantern Quarter, at Hearth Road's end.
        "belvedere" => District::LanternQuarter,
        // North of the market hall, facing the plaza.
        "agora" => District::FountainPlaza,
        _ => return None,
    })
}

impl Building {
    /// The district this building stands in; `None` only for a building
    /// that the district table is missing, which a test refuses.
    #[must_use]
    pub fn district(&self) -> Option<District> {
        of_building(self.name)
    }
}

/// Every building and generated instance of `district`, by name.
#[must_use]
pub fn members(district: District) -> Vec<&'static str> {
    let buildings = city::BUILDINGS
        .iter()
        .map(|b| b.name)
        .filter(|n| of_building(n) == Some(district));
    let instances = city::instances()
        .into_iter()
        .map(|i| i.name)
        .filter(|n| of_instance(n) == Some(district));
    let mut names: Vec<_> = buildings.chain(instances).collect();
    names.sort_unstable();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_building_has_a_district() {
        let missing: Vec<_> = city::BUILDINGS
            .iter()
            .filter(|b| b.district().is_none())
            .map(|b| b.name)
            .collect();
        assert!(missing.is_empty(), "no district for {missing:?}");
    }

    #[test]
    fn every_generated_instance_has_a_district() {
        let missing: Vec<_> = city::instances()
            .into_iter()
            .filter(|i| of_instance(i.name).is_none())
            .map(|i| i.name)
            .collect();
        assert!(missing.is_empty(), "no district for {missing:?}");
        assert_eq!(of_instance("civic hall"), Some(District::MainStreet));
        assert_eq!(
            of_instance("owner's house"),
            Some(District::KnowledgeDistrict)
        );
    }

    #[test]
    fn districts_have_names_and_most_have_buildings() {
        let mut names: Vec<_> = District::ALL.iter().map(|d| d.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), District::ALL.len());
        // The Commons, the Gardens, and the wilds hold open ground and a
        // few landmarks; every other district has city buildings.
        for d in District::ALL {
            let members = members(d);
            assert!(!members.is_empty() || d == District::Gardens, "{d:?}");
        }
        assert!(members(District::LanternQuarter).contains(&"the lantern"));
    }
}
