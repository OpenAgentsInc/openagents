//! Immutable account, credential, and character records selected by the realm head.
use super::*;
use std::io::Write;
const MAX_NODE: usize = 128 * 1024;
pub(super) type Root = Option<[u8; 32]>;
pub use super::super::accounts::Account;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: u64,
    pub account: u64,
    pub residence: Residence,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Residence {
    Resident {
        instance: u64,
        actor: u64,
    },
    Dormant {
        instance: u64,
        actor: u64,
        content: [u8; 32],
        snapshot: [u8; 32],
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Record {
    Account(Account),
    Character(Character),
    Credential {
        key: [u8; 32],
        account: u64,
        epoch: u64,
        current: bool,
    },
}
#[derive(Serialize, Deserialize)]
enum Node {
    Leaf(Vec<([u8; 32], Record)>),
    Branch([Root; 16]),
}
pub(super) fn account_key(id: u64) -> [u8; 32] {
    key(b"account", &id.to_be_bytes())
}
pub(super) fn character_key(id: u64) -> [u8; 32] {
    key(b"character", &id.to_be_bytes())
}
pub(super) fn credential_key(public: [u8; 32]) -> [u8; 32] {
    key(b"credential", &public)
}
fn key(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut bytes = b"verse.realm.registry.v1\0".to_vec();
    bytes.extend_from_slice(domain);
    bytes.push(0);
    bytes.extend_from_slice(value);
    disk::digest(&bytes)
}
fn nibble(key: [u8; 32], depth: usize) -> usize {
    if depth % 2 == 0 {
        (key[depth / 2] >> 4) as usize
    } else {
        (key[depth / 2] & 15) as usize
    }
}
fn read(realm: &Realm, root: [u8; 32]) -> Result<Node, String> {
    let bytes = disk::read(
        &realm.root.join("registry").join(disk::hex(&root)),
        MAX_NODE,
    )?;
    if disk::digest(&bytes) != root {
        return Err("Realm registry digest mismatch".into());
    }
    let node: Node = serde_json::from_slice(&bytes).map_err(|_| "Invalid realm registry node")?;
    match &node {
        Node::Leaf(records)
            if records.is_empty()
                || records.len() > 8
                || records.windows(2).any(|r| r[0].0 >= r[1].0) =>
        {
            return Err("Invalid realm registry leaf".into());
        }
        Node::Branch(children) if children.iter().all(Option::is_none) => {
            return Err("Empty realm registry branch".into());
        }
        _ => {}
    }
    Ok(node)
}
fn write(realm: &Realm, node: &Node) -> Result<[u8; 32], String> {
    let bytes = serde_json::to_vec(node).map_err(|_| "Cannot encode realm registry node")?;
    if bytes.len() > MAX_NODE {
        return Err("Realm registry node exceeds byte budget".into());
    }
    let digest = disk::digest(&bytes);
    let directory = realm.root.join("registry");
    let target = directory.join(disk::hex(&digest));
    disk::regular(&target)?;
    if target.exists() {
        read(realm, digest)?;
        return Ok(digest);
    }
    let pending = directory.join("registry.next");
    disk::regular(&pending)?;
    let mut file = disk::options()
        .create(true)
        .truncate(true)
        .open(&pending)
        .map_err(|_| "Cannot create realm registry node")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot sync realm registry node")?;
    std::fs::rename(pending, target).map_err(|_| "Cannot publish realm registry node")?;
    Ok(digest)
}
pub(super) fn get(realm: &Realm, root: Root, key: [u8; 32]) -> Result<Option<Record>, String> {
    let mut root = root;
    for depth in 0..=64 {
        let Some(hash) = root else {
            return Ok(None);
        };
        match read(realm, hash)? {
            Node::Leaf(records) => {
                return Ok(records.into_iter().find(|r| r.0 == key).map(|r| r.1));
            }
            Node::Branch(children) if depth < 64 => root = children[nibble(key, depth)],
            _ => return Err("Realm registry depth exceeded".into()),
        }
    }
    Err("Realm registry depth exceeded".into())
}
fn insert(
    realm: &Realm,
    root: Root,
    mut records: Vec<([u8; 32], Record)>,
    depth: usize,
) -> Result<[u8; 32], String> {
    let children = match root
        .map(|h| read(realm, h))
        .transpose()?
        .unwrap_or(Node::Leaf(vec![]))
    {
        Node::Leaf(existing) => {
            for record in existing {
                if !records.iter().any(|r| r.0 == record.0) {
                    records.push(record);
                }
            }
            records.sort_by_key(|r| r.0);
            if records.len() <= 8 {
                return write(realm, &Node::Leaf(records));
            }
            [None; 16]
        }
        Node::Branch(children) => children,
    };
    if depth >= 64 {
        return Err("Realm registry depth exceeded".into());
    }
    let mut groups: [Vec<([u8; 32], Record)>; 16] = std::array::from_fn(|_| vec![]);
    for record in records {
        groups[nibble(record.0, depth)].push(record);
    }
    let mut children = children;
    for (index, group) in groups.into_iter().enumerate() {
        if !group.is_empty() {
            children[index] = Some(insert(realm, children[index], group, depth + 1)?);
        }
    }
    write(realm, &Node::Branch(children))
}
pub(super) fn put(realm: &Realm, mut root: Root, records: Vec<Record>) -> Result<Root, String> {
    if records.is_empty() {
        return Ok(root);
    }
    let mut keyed = Vec::with_capacity(records.len());
    for record in records {
        let key = match &record {
            Record::Account(a) => account_key(a.id),
            Record::Character(c) => character_key(c.id),
            Record::Credential { key, .. } => credential_key(*key),
        };
        keyed.push((key, record));
    }
    keyed.sort_by_key(|r| r.0);
    if keyed.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("Registry transaction duplicates a record".into());
    }
    root = Some(insert(realm, root, keyed, 0)?);
    File::open(realm.root.join("registry"))
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync realm registry directory")?;
    Ok(root)
}
pub(super) fn initial(account: u64, character: u64, placement: &Placement) -> Vec<Record> {
    vec![
        Record::Account(Account {
            id: account,
            epoch: 1,
            key: placement.principal,
            characters: vec![character],
        }),
        Record::Credential {
            key: placement.principal,
            account,
            epoch: 1,
            current: true,
        },
        Record::Character(Character {
            id: character,
            account,
            residence: Residence::Resident {
                instance: placement.instance,
                actor: placement.actor,
            },
        }),
    ]
}
impl Realm {
    pub fn account(&self, id: u64) -> Result<Account, String> {
        if self.poisoned {
            return Err("Realm requires recovery after an uncertain commit".into());
        }
        match get(self, self.manifest.registry_root, account_key(id))? {
            Some(Record::Account(a))
                if a.id == id
                    && id > 0
                    && id < self.manifest.next_account
                    && a.epoch > 0
                    && !a.characters.is_empty()
                    && a.characters.len() <= 8
                    && a.characters.windows(2).all(|w| w[0] < w[1])
                    && a.characters
                        .iter()
                        .all(|id| *id > 0 && *id < self.manifest.next_character)
                    && super::super::auth::valid_principal(a.key).is_ok() =>
            {
                Ok(a)
            }
            _ => Err("Realm account is missing or incompatible".into()),
        }
    }
    pub fn account_for_key(&self, key: [u8; 32]) -> Result<Option<Account>, String> {
        if self.poisoned {
            return Err("Realm requires recovery after an uncertain commit".into());
        }
        match get(self, self.manifest.registry_root, credential_key(key))? {
            None => Ok(None),
            Some(Record::Credential {
                key: saved,
                account,
                epoch,
                current: true,
            }) if saved == key => {
                let a = self.account(account)?;
                if a.key != key || a.epoch != epoch {
                    return Err("Realm credential binding is incompatible".into());
                }
                Ok(Some(a))
            }
            _ => Err("Realm credential is retired or incompatible".into()),
        }
    }
    pub fn character(&self, id: u64) -> Result<Character, String> {
        if self.poisoned {
            return Err("Realm requires recovery after an uncertain commit".into());
        }
        match get(self, self.manifest.registry_root, character_key(id))? {
            Some(Record::Character(c))
                if c.id == id
                    && id > 0
                    && id < self.manifest.next_character
                    && self.account(c.account)?.characters.contains(&id) =>
            {
                Ok(c)
            }
            _ => Err("Realm character is missing or incompatible".into()),
        }
    }
}
pub(super) fn upgrade(realm: &mut Realm) -> Result<(), String> {
    if realm.manifest.version == 1 {
        let mut root = None;
        let mut next = 1u64;
        for (id, placement) in &realm.manifest.characters {
            root = put(realm, root, initial(next, *id, placement))?;
            next = next
                .checked_add(1)
                .ok_or("Realm account identities exhausted")?;
        }
        realm.manifest.registry_root = root;
        realm.manifest.next_account = next;
        realm.manifest.version = 2;
    }
    for (id, placement) in &realm.manifest.characters {
        let character = realm.character(*id)?;
        let account = realm
            .account_for_key(placement.principal)?
            .ok_or("Resident account is missing")?;
        if account.id != character.account {
            return Err("Resident account identity differs".into());
        }
        if account.key != placement.principal
            || !matches!(character.residence,
            Residence::Resident { instance, actor } if instance == placement.instance && actor == placement.actor)
        {
            return Err("Realm resident registry disagrees with placement".into());
        }
    }
    Ok(())
}
