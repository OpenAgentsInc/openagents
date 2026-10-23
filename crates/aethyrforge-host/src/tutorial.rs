//! Account-scoped tutorial progress and the bounded first personal planet.
use crate::{Error, MAX_PROJECT_BYTES, MAX_SAFE_INTEGER, MAX_SAVED_VERSIONS, project_hash};
use serde::{Deserialize, Serialize};
pub const TUTORIAL_VERSION: u16 = 1;
pub const PERSONAL_PROJECT_KEY: &str = "verse-personal-v1";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TutorialPublication {
    pub world: String,
    pub release: String,
    pub project: String,
    pub saved_version: u64,
}
impl TutorialPublication {
    pub fn validate(&self) -> Result<(), Error> {
        let hash = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        };
        if !hash(&self.world)
            || !hash(&self.release)
            || !crate::protocol::id(&self.project)
            || self.saved_version == 0
            || self.saved_version > MAX_SAFE_INTEGER
        {
            return Err(Error::Invalid("tutorial publication"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TutorialChoice {
    Play,
    Share,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanetCapacity {
    pub side_metres: u16,
    pub max_entities: u16,
    pub max_project_bytes: u32,
    pub max_saved_versions: u16,
}
impl Default for PlanetCapacity {
    fn default() -> Self {
        Self {
            side_metres: 64,
            max_entities: 128,
            max_project_bytes: MAX_PROJECT_BYTES as u32,
            max_saved_versions: MAX_SAVED_VERSIONS as u16,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PersonalPlanet {
    pub id: String,
    pub owner: String,
    pub project: String,
    pub name: String,
    pub created_unix: u64,
    pub capacity: PlanetCapacity,
}
impl PersonalPlanet {
    pub fn id_for(owner: &str) -> String {
        format!(
            "planet_{}",
            project_hash(format!("verse-personal-v1\0{owner}").as_bytes())
        )
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.owner.is_empty()
            || self.owner.len() > 256
            || self.owner.chars().any(char::is_control)
            || self.id != Self::id_for(&self.owner)
            || !crate::protocol::id(&self.project)
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
            || self.created_unix > MAX_SAFE_INTEGER
            || self.capacity != PlanetCapacity::default()
        {
            return Err(Error::Invalid("personal planet"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TutorialProgress {
    pub version: u16,
    pub publication: Option<TutorialPublication>,
    pub choice: Option<TutorialChoice>,
    pub returned_unix: Option<u64>,
    pub planet: Option<PersonalPlanet>,
}
impl Default for TutorialProgress {
    fn default() -> Self {
        Self {
            version: TUTORIAL_VERSION,
            publication: None,
            choice: None,
            returned_unix: None,
            planet: None,
        }
    }
}
impl TutorialProgress {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != TUTORIAL_VERSION
            || self.choice.is_some() && self.publication.is_none()
            || self.returned_unix.is_some() && self.choice.is_none()
            || self.returned_unix.is_some_and(|n| n > MAX_SAFE_INTEGER)
            || self.planet.is_some() && self.returned_unix.is_none()
        {
            return Err(Error::Invalid("tutorial progress"));
        }
        if let Some(p) = &self.publication {
            p.validate()?;
        }
        if let Some(p) = &self.planet {
            p.validate()?;
        }
        Ok(())
    }
    pub fn published(&mut self, p: TutorialPublication) -> Result<(), Error> {
        p.validate()?;
        if self.planet.is_none() && self.publication.as_ref() != Some(&p) {
            self.publication = Some(p);
            self.choice = None;
            self.returned_unix = None;
        }
        self.validate()
    }
    pub fn choose(&mut self, p: TutorialPublication, choice: TutorialChoice) -> Result<(), Error> {
        p.validate()?;
        if self.planet.is_none() {
            if self.publication.as_ref() != Some(&p) {
                self.returned_unix = None;
            }
            self.publication = Some(p);
            self.choice = Some(choice);
        }
        self.validate()
    }
    pub fn returned(&mut self, now: u64) -> Result<(), Error> {
        if self.choice.is_none() || now > MAX_SAFE_INTEGER {
            return Err(Error::Phase);
        }
        self.returned_unix.get_or_insert(now);
        self.validate()
    }
    pub fn allocate(&mut self, planet: PersonalPlanet) -> Result<(), Error> {
        planet.validate()?;
        if self.returned_unix.is_none() {
            return Err(Error::Phase);
        }
        if self.planet.as_ref().is_some_and(|old| old != &planet) {
            return Err(Error::Invalid("planet already allocated"));
        }
        self.planet = Some(planet);
        self.validate()
    }
}
