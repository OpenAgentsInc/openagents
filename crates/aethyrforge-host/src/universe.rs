//! Curated universe entry and destinations. These are not live network counts.
use crate::{Checkpoint, Error};
use serde::{Deserialize, Serialize};

pub struct World {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}
pub const WORLDS: [World; 6] = [
    World {
        id: "openagents",
        name: "OpenAgents",
        category: "Company world · Locked",
        description: "Our home in the Verse. The Gym and Pylons will open here for their communities.",
    },
    World {
        id: "blue-rush",
        name: "Blue Rush",
        category: "Creator studio",
        description: "Stories, games, and worlds with a little wonder. Meet the studio behind the Haunted House.",
    },
    World {
        id: "haunted-house",
        name: "Haunted House",
        category: "A world by Blue Rush",
        description: "Step inside, look around, and make something your own.",
    },
    World {
        id: "creator-worlds",
        name: "Creator worlds",
        category: "Locked",
        description: "More worlds are still taking shape.",
    },
    World {
        id: "new-horizons",
        name: "New horizons",
        category: "Locked",
        description: "A little room for what comes next.",
    },
    World {
        id: "far-field",
        name: "The far field",
        category: "Locked",
        description: "An open horizon. For now, enjoy the view.",
    },
];
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryChoice {
    Guest,
    Member,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UniverseVisit {
    pub choice: Option<EntryChoice>,
    pub exploring: bool,
    pub selected: String,
    pub checkpoint: Option<Checkpoint>,
}
impl Default for UniverseVisit {
    fn default() -> Self {
        Self {
            choice: None,
            exploring: false,
            selected: "openagents".into(),
            checkpoint: None,
        }
    }
}
impl UniverseVisit {
    pub fn validate(&self) -> Result<(), Error> {
        if !WORLDS.iter().any(|w| w.id == self.selected)
            || (self.exploring && self.choice.is_none())
        {
            return Err(Error::Invalid("universe visit"));
        }
        if let Some(c) = &self.checkpoint {
            c.validate()?;
        }
        Ok(())
    }
    pub fn enter(&mut self, choice: EntryChoice) {
        self.choice = Some(choice);
    }
    pub fn explore(&mut self) -> Result<(), Error> {
        if self.choice.is_none() {
            return Err(Error::Phase);
        }
        self.exploring = true;
        Ok(())
    }
    pub fn select(&mut self, id: &str) -> Result<(), Error> {
        if !self.exploring {
            return Err(Error::Phase);
        }
        if !WORLDS.iter().any(|w| w.id == id) {
            return Err(Error::Invalid("destination"));
        }
        self.selected = id.into();
        Ok(())
    }
}
