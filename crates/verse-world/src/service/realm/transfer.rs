//! One sealed realm head moves placement, both game states, and the retry root.
use super::*;
use std::io::Write;
const NODE_BYTES: usize = 256 * 1024;
type Root = Option<[u8; 32]>;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    pub operation: [u8; 16],
    pub character: u64,
    pub source: LifeId,
    pub destination: LifeId,
    pub spawn: [f32; 3],
}
#[derive(Serialize, Deserialize)]
enum Node {
    Leaf(Vec<Transfer>),
    Branch([Root; 16]),
}
fn nibble(key: [u8; 16], depth: usize) -> usize {
    if depth % 2 == 0 {
        (key[depth / 2] >> 4) as usize
    } else {
        (key[depth / 2] & 15) as usize
    }
}
fn read(realm: &Realm, root: [u8; 32]) -> Result<Node, String> {
    let bytes = disk::read(
        &realm.root.join("transfers").join(disk::hex(&root)),
        NODE_BYTES,
    )?;
    if disk::digest(&bytes) != root {
        return Err("Realm transfer history digest mismatch".into());
    }
    let node: Node =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid realm transfer history")?;
    if matches!(&node, Node::Leaf(v) if v.is_empty() || v.len() > 16) {
        return Err("Realm transfer history leaf exceeds budget".into());
    }
    Ok(node)
}
fn write(realm: &Realm, node: &Node) -> Result<[u8; 32], String> {
    let bytes = serde_json::to_vec(node).map_err(|_| "Cannot encode realm transfer history")?;
    if bytes.len() > NODE_BYTES {
        return Err("Realm transfer history byte budget exceeded".into());
    }
    let hash = disk::digest(&bytes);
    let directory = realm.root.join("transfers");
    let target = directory.join(disk::hex(&hash));
    disk::regular(&target)?;
    if target.exists() {
        read(realm, hash)?;
        return Ok(hash);
    }
    let pending = directory.join("transfer.next");
    disk::regular(&pending)?;
    let mut file = disk::options()
        .create(true)
        .truncate(true)
        .open(&pending)
        .map_err(|_| "Cannot create realm transfer history")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot sync realm transfer history")?;
    std::fs::rename(pending, target).map_err(|_| "Cannot publish realm transfer history")?;
    Ok(hash)
}
fn lookup(realm: &Realm, operation: [u8; 16]) -> Result<Option<Transfer>, String> {
    let mut root = realm.manifest.transfer_root;
    for depth in 0..=32 {
        let Some(hash) = root else {
            return Ok(None);
        };
        match read(realm, hash)? {
            Node::Leaf(records) => {
                return Ok(records.into_iter().find(|r| r.operation == operation));
            }
            Node::Branch(children) if depth < 32 => root = children[nibble(operation, depth)],
            Node::Branch(_) => return Err("Realm transfer history depth exceeded".into()),
        }
    }
    Err("Realm transfer history depth exceeded".into())
}
fn insert(
    realm: &Realm,
    root: Root,
    mut records: Vec<Transfer>,
    depth: usize,
) -> Result<[u8; 32], String> {
    let children = match root
        .map(|h| read(realm, h))
        .transpose()?
        .unwrap_or(Node::Leaf(vec![]))
    {
        Node::Leaf(existing) => {
            records.extend(existing);
            records.sort_by_key(|r| r.operation);
            if records.windows(2).any(|r| r[0].operation == r[1].operation) {
                return Err("Realm transfer operation is duplicated".into());
            }
            if records.len() <= 16 {
                return write(realm, &Node::Leaf(records));
            }
            [None; 16]
        }
        Node::Branch(children) => children,
    };
    if depth >= 32 {
        return Err("Realm transfer history depth exceeded".into());
    }
    let mut groups: [Vec<Transfer>; 16] = std::array::from_fn(|_| vec![]);
    for record in records {
        groups[nibble(record.operation, depth)].push(record);
    }
    let mut children = children;
    for (index, records) in groups.into_iter().enumerate() {
        if !records.is_empty() {
            children[index] = Some(insert(realm, children[index], records, depth + 1)?);
        }
    }
    write(realm, &Node::Branch(children))
}
pub(super) fn validate(realm: &Realm) -> Result<(), String> {
    let mut stack = vec![(realm.manifest.transfer_root, Vec::<usize>::new())];
    let mut count = 0u64;
    while let Some((root, prefix)) = stack.pop() {
        let Some(hash) = root else {
            continue;
        };
        match read(realm, hash)? {
            Node::Leaf(records) => {
                let mut previous = None;
                for record in records {
                    if record.operation == [0; 16]
                        || previous.is_some_and(|p| p >= record.operation)
                        || prefix
                            .iter()
                            .enumerate()
                            .any(|(depth, n)| nibble(record.operation, depth) != *n)
                        || !realm.manifest.characters.contains_key(&record.character)
                        || record.source.instance == record.destination.instance
                        || record.source.actor == 0
                        || record.destination.actor == 0
                        || !realm
                            .manifest
                            .instances
                            .contains_key(&record.source.instance)
                        || !realm
                            .manifest
                            .instances
                            .contains_key(&record.destination.instance)
                        || record
                            .spawn
                            .iter()
                            .any(|v| !v.is_finite() || v.abs() > 10_000.)
                    {
                        return Err("Realm transfer history identity is incompatible".into());
                    }
                    previous = Some(record.operation);
                    count = count
                        .checked_add(1)
                        .ok_or("Realm transfer count exhausted")?;
                }
            }
            Node::Branch(children) => {
                if prefix.len() >= 32 || children.iter().all(Option::is_none) {
                    return Err("Realm transfer history branch is invalid".into());
                }
                for (index, child) in children.into_iter().enumerate() {
                    if child.is_some() {
                        let mut path = prefix.clone();
                        path.push(index);
                        stack.push((child, path));
                    }
                }
            }
        }
    }
    if count != realm.manifest.transfers {
        return Err("Realm transfer history count is incompatible".into());
    }
    Ok(())
}
impl Realm {
    /// A trusted host chooses the destination and collision-checked spawn.
    pub fn transfer(
        &mut self,
        source: &Lease,
        destination: &Lease,
        character: u64,
        operation: [u8; 16],
        spawn: [f32; 3],
        now: u64,
    ) -> Result<Transfer, String> {
        self.check(source, now)?;
        self.check(destination, now)?;
        if source.instance == destination.instance
            || operation == [0; 16]
            || spawn.iter().any(|v| !v.is_finite() || v.abs() > 10_000.)
        {
            return Err("Realm transfer request is incompatible".into());
        }
        if let Some(previous) = lookup(self, operation)? {
            if previous.character != character
                || previous.source.instance != source.instance
                || previous.destination.instance != destination.instance
                || previous.spawn.map(f32::to_bits) != spawn.map(f32::to_bits)
            {
                return Err("Realm transfer retry differs from its committed request".into());
            }
            return Ok(previous);
        }
        let placement = self
            .manifest
            .characters
            .get(&character)
            .ok_or("Realm character is missing")?
            .clone();
        if placement.instance != source.instance
            || self.manifest.instances[&destination.instance].phase != Phase::Open
            || self.games[&destination.instance].chamber.owners.len()
                >= self.manifest.instances[&destination.instance].capacity as usize
        {
            return Err("Realm transfer placement or destination capacity is incompatible".into());
        }
        let original = &self.games[&source.instance].chamber;
        let target = &self.games[&destination.instance].chamber;
        let catalogs = |c: &super::super::Chamber| {
            serde_json::to_vec(&(&c.items, &c.outfits, &c.equipment, &c.progression))
                .map_err(|_| "Cannot compare realm character catalogs")
        };
        if catalogs(original)? != catalogs(target)? {
            return Err("Realm transfer character catalogs are incompatible".into());
        }
        // Recovery copies park every connection; no live gateway mutates before validation.
        let copy = |id| {
            let g = &self.games[&id];
            super::super::save::decode_with_history(
                &g.checkpoint()?,
                g.content().unwrap(),
                id,
                Some(self.history.clone()),
            )
        };
        let mut from = copy(source.instance)?;
        let mut to = copy(destination.instance)?;
        let old_life = from
            .game()
            .player_admission(placement.actor)
            .ok_or("Source character life is missing")?
            .actor();
        let portable = from.chamber.game.take_transfer_player(placement.actor)?;
        let (book, state) = from.chamber.rewards.take_book(placement.actor)?;
        if book.character != character {
            return Err("Realm transfer receipt identity differs from placement".into());
        }
        let principal = super::super::Principal(placement.principal);
        from.chamber.owners.remove(&principal);
        from.chamber.grants.remove(&principal);
        let new_life = to.enroll_player(placement.principal, glam::Vec3::from(spawn))?;
        to.chamber.game.put_transfer_player(new_life, portable)?;
        to.chamber.rewards.put_book(new_life.actor, book, state)?;
        let receipt = Transfer {
            operation,
            character,
            source: old_life,
            destination: new_life,
            spawn,
        };
        // Validate both encoded copies before selecting their immutable checkpoints.
        for (id, g) in [(source.instance, &from), (destination.instance, &to)] {
            super::super::save::decode_with_history(
                &g.checkpoint()?,
                g.content().unwrap(),
                id,
                Some(self.history.clone()),
            )?;
        }
        let root = insert(self, self.manifest.transfer_root, vec![receipt.clone()], 0)?;
        File::open(self.root.join("transfers"))
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync realm transfer directory")?;
        let count = self
            .manifest
            .transfers
            .checked_add(1)
            .ok_or("Realm transfer count exhausted")?;
        self.games.insert(source.instance, from);
        self.games.insert(destination.instance, to);
        self.manifest.characters.insert(
            character,
            Placement {
                instance: destination.instance,
                actor: new_life.actor,
                ..placement
            },
        );
        self.manifest.transfer_root = Some(root);
        self.manifest.transfers = count;
        self.transfer_commit = true;
        self.publish(&[source.instance, destination.instance])?;
        self.transfer_commit = false;
        Ok(receipt)
    }
}
