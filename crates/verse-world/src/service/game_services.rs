//! Stable character membership and consent-based gear exchange in a realm.
use serde::{Deserialize, Serialize};
pub type Id = [u8; 32];
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Party,
    Guild,
}
impl Kind {
    pub fn capacity(self) -> usize {
        match self {
            Self::Party => 8,
            Self::Guild => 64,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: Id,
    pub kind: Kind,
    pub name: String,
    pub leader: u64,
    pub members: Vec<u64>,
    pub invites: Vec<u64>,
    pub revision: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    pub id: Id,
    pub version: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: Id,
    pub owner: u64,
    pub definition: u64,
    pub catalog: Id,
    pub version: u64,
    pub locked: Option<Id>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Offered,
    Accepted,
    Cancelled,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub id: Id,
    pub from: u64,
    pub to: u64,
    pub give: Vec<Token>,
    pub want: Vec<Token>,
    pub expires_ms: u64,
    pub status: Status,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Create {
        kind: Kind,
        name: String,
    },
    Invite {
        group: Id,
        target: u64,
    },
    Join {
        group: Id,
    },
    Decline {
        group: Id,
    },
    Leave {
        group: Id,
    },
    Remove {
        group: Id,
        target: u64,
    },
    Materialize {
        definition: u64,
    },
    Offer {
        to: u64,
        give: Vec<Token>,
        want: Vec<Token>,
        expires_ms: u64,
    },
    Accept {
        offer: Id,
    },
    Cancel {
        offer: Id,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Group { group: Group },
    Item { item: Item },
    Trade { offer: Offer },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub realm: Id,
    pub character: u64,
    pub operation: [u8; 16],
    pub digest: Id,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub realm: Id,
    pub character: u64,
    pub groups: Vec<Group>,
    pub invitations: Vec<Group>,
    pub items: Vec<Item>,
    pub offers: Vec<Offer>,
}
/// Amounts are supplied only by the local authoritative host.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loot {
    pub event: Id,
    pub party: Id,
    pub experience: u64,
    pub items: Vec<super::rewards::Entry>,
    pub quests: Vec<super::rewards::Entry>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LootReceipt {
    pub grant: Loot,
    pub recipients: Vec<u64>,
    pub receipts: Vec<super::rewards::Receipt>,
}

pub(super) fn trade_source(source: &[u8; 32]) -> bool {
    source[..8] == *b"VTRADE01"
}
pub(super) fn validate_trade(tx: &super::rewards::Transaction) -> Result<(), String> {
    if !trade_source(&tx.source)
        || tx.source[8..] == [0; 24]
        || tx.experience != 0
        || !tx.quests.is_empty()
        || tx.acceptance.is_some()
        || tx.outfit.is_some()
        || tx.equipment.is_some()
        || tx.items.is_empty() && tx.spent.is_empty()
    {
        return Err("Invalid saved realm trade transaction".into());
    }
    super::rewards::entries(&tx.items)?;
    super::rewards::entries(&tx.spent)
}
pub fn action_digest(action: &Action) -> Result<Id, String> {
    use sha2::Digest;
    Ok(sha2::Sha256::digest(
        serde_json::to_vec(action).map_err(|_| "Cannot encode service action")?,
    )
    .into())
}
impl Group {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == [0; 32]
            || self.revision == 0
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || !self.name.bytes().all(|b| (32..=126).contains(&b))
            || self.members.len() > self.kind.capacity()
            || self.invites.len() > 64
            || self.members.windows(2).any(|w| w[0] >= w[1])
            || self.invites.windows(2).any(|w| w[0] >= w[1])
            || self.members.contains(&0)
            || self.invites.contains(&0)
            || self.members.iter().any(|id| self.invites.contains(id))
            || (self.members.is_empty() && (self.leader != 0 || !self.invites.is_empty()))
            || (!self.members.is_empty() && !self.members.contains(&self.leader))
        {
            return Err("Invalid realm group projection".into());
        }
        Ok(())
    }
}
impl Item {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == [0; 32]
            || self.owner == 0
            || self.definition == 0
            || self.catalog == [0; 32]
            || self.version == 0
            || self.locked == Some([0; 32])
        {
            return Err("Invalid item instance projection".into());
        }
        Ok(())
    }
}
impl Offer {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == [0; 32]
            || self.from == 0
            || self.to == 0
            || self.from == self.to
            || self.expires_ms == 0
            || self.give.is_empty() && self.want.is_empty()
            || [&self.give, &self.want].iter().any(|tokens| {
                tokens.len() > 8
                    || tokens.windows(2).any(|w| w[0].id >= w[1].id)
                    || tokens.iter().any(|t| t.id == [0; 32] || t.version == 0)
            })
            || self
                .give
                .iter()
                .any(|t| self.want.iter().any(|other| t.id == other.id))
        {
            return Err("Invalid trade offer projection".into());
        }
        Ok(())
    }
}
impl View {
    pub fn validate(&self) -> Result<(), String> {
        if self.realm == [0; 32]
            || self.character == 0
            || self.groups.len() > 2
            || self.invitations.len() > 16
            || self.items.len() > 64
            || self.offers.len() > 16
            || self.groups.len() == 2 && self.groups[0].kind == self.groups[1].kind
            || self
                .groups
                .iter()
                .any(|g| !g.members.contains(&self.character))
            || self
                .invitations
                .iter()
                .any(|g| !g.invites.contains(&self.character))
            || self.items.windows(2).any(|w| w[0].id >= w[1].id)
            || self.offers.windows(2).any(|w| w[0].id >= w[1].id)
            || self.items.iter().any(|i| i.owner != self.character)
            || self.offers.iter().any(|o| {
                o.status != Status::Offered || o.from != self.character && o.to != self.character
            })
        {
            return Err("Invalid private realm service projection".into());
        }
        for g in self.groups.iter().chain(&self.invitations) {
            g.validate()?;
        }
        for i in &self.items {
            i.validate()?;
        }
        for o in &self.offers {
            o.validate()?;
        }
        Ok(())
    }
}
impl Receipt {
    pub fn validate(&self, action: &Action) -> Result<(), String> {
        if self.realm == [0; 32]
            || self.character == 0
            || self.operation == [0; 16]
            || self.digest != action_digest(action)?
        {
            return Err("Invalid service receipt identity".into());
        }
        match &self.outcome {
            Outcome::Group { group } => group.validate()?,
            Outcome::Item { item } => item.validate()?,
            Outcome::Trade { offer } => offer.validate()?,
        }
        let matches = match (action, &self.outcome) {
            (Action::Create { kind, name }, Outcome::Group { group }) => {
                group.kind == *kind
                    && group.name == *name
                    && group.leader == self.character
                    && group.members == [self.character]
            }
            (Action::Invite { group, target }, Outcome::Group { group: result }) => {
                result.id == *group
                    && result.leader == self.character
                    && result.invites.contains(target)
            }
            (Action::Join { group }, Outcome::Group { group: result }) => {
                result.id == *group && result.members.contains(&self.character)
            }
            (Action::Decline { group }, Outcome::Group { group: result }) => {
                result.id == *group && !result.invites.contains(&self.character)
            }
            (Action::Leave { group }, Outcome::Group { group: result }) => {
                result.id == *group && !result.members.contains(&self.character)
            }
            (Action::Remove { group, target }, Outcome::Group { group: result }) => {
                result.id == *group && !result.members.contains(target)
            }
            (Action::Materialize { definition }, Outcome::Item { item }) => {
                item.definition == *definition
                    && item.owner == self.character
                    && item.version == 1
                    && item.locked.is_none()
            }
            (
                Action::Offer {
                    to,
                    give,
                    want,
                    expires_ms,
                },
                Outcome::Trade { offer },
            ) => {
                offer.from == self.character
                    && offer.to == *to
                    && offer.give == *give
                    && offer.want == *want
                    && offer.expires_ms == *expires_ms
                    && offer.status == Status::Offered
            }
            (Action::Accept { offer }, Outcome::Trade { offer: result }) => {
                result.id == *offer
                    && result.to == self.character
                    && result.status == Status::Accepted
            }
            (Action::Cancel { offer }, Outcome::Trade { offer: result }) => {
                result.id == *offer
                    && (result.from == self.character || result.to == self.character)
                    && result.status == Status::Cancelled
            }
            _ => false,
        };
        if !matches {
            return Err("Service receipt outcome does not match its action".into());
        }
        Ok(())
    }
}
