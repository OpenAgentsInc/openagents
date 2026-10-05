//! Stable character keys preserve original receipts across instance placement.
use super::*;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Indexed {
    character: u64,
    source: [u8; 32],
    receipt: Receipt,
}
impl History {
    pub(in crate::service) fn character_receipt(
        &self,
        mut root: Root,
        character: u64,
        source: [u8; 32],
    ) -> Result<Option<Receipt>, String> {
        let hash = key(character, source);
        for depth in 0..=64 {
            let Some(digest) = root else { return Ok(None) };
            match self.read(digest)? {
                Node::Character(records) => {
                    return Ok(records
                        .into_iter()
                        .find(|r| r.character == character && r.source == source)
                        .map(|r| r.receipt));
                }
                Node::Branch(children) if depth < 64 => root = children[nibble(hash, depth)],
                Node::Branch(_) => return Err("Character receipt index depth exceeded".into()),
                Node::Leaf(_) => {
                    return Err("Character receipt root refers to a legacy index".into());
                }
            }
        }
        Err("Character receipt index depth exceeded".into())
    }
    pub(in crate::service) fn insert_character(
        &self,
        root: Root,
        character: u64,
        source: [u8; 32],
        receipt: Receipt,
    ) -> Result<Root, String> {
        if character == 0 || source == [0; 32] {
            return Err("Character receipt identity is empty".into());
        }
        super::super::validate_transaction(&receipt.transaction)?;
        let next = self.insert_character_at(
            root,
            vec![Indexed {
                character,
                source,
                receipt,
            }],
            0,
        )?;
        if !self.0.deferred.load(Ordering::Acquire) {
            File::open(&self.0.path)
                .and_then(|f| f.sync_all())
                .map_err(|_| "Cannot sync character receipt directory")?;
        }
        Ok(Some(next))
    }
    fn insert_character_at(
        &self,
        root: Root,
        mut records: Vec<Indexed>,
        depth: usize,
    ) -> Result<[u8; 32], String> {
        let node = root
            .map(|r| self.read(r))
            .transpose()?
            .unwrap_or(Node::Character(vec![]));
        let children = match node {
            Node::Character(existing) => {
                records.extend(existing);
                records.sort_by_key(|r| (r.character, r.source));
                if records
                    .windows(2)
                    .any(|r| (r[0].character, r[0].source) == (r[1].character, r[1].source))
                {
                    return Err("Character receipt source is already indexed".into());
                }
                if records.len() <= LEAF_RECEIPTS {
                    return self.write(&Node::Character(records));
                }
                [None; 16]
            }
            Node::Branch(children) => children,
            Node::Leaf(_) => {
                return Err("Cannot insert a character receipt into a legacy index".into());
            }
        };
        if depth >= 64 {
            return Err("Character receipt index depth exceeded".into());
        }
        let mut groups: [Vec<Indexed>; 16] = std::array::from_fn(|_| Vec::new());
        for record in records {
            groups[nibble(key(record.character, record.source), depth)].push(record);
        }
        let mut children = children;
        for (index, records) in groups.into_iter().enumerate() {
            if !records.is_empty() {
                children[index] =
                    Some(self.insert_character_at(children[index], records, depth + 1)?);
            }
        }
        self.write(&Node::Branch(children))
    }
    pub(in crate::service) fn validate_character(
        &self,
        root: Root,
        character: u64,
        base_revision: u64,
        revision: u64,
    ) -> Result<(), String> {
        let expected = revision
            .checked_sub(base_revision)
            .ok_or("Character receipt revisions regressed")?;
        let count =
            self.validate_character_at(root, character, base_revision, revision, &mut vec![])?;
        if count != expected {
            return Err("Character receipt count is incompatible".into());
        }
        Ok(())
    }
    fn validate_character_at(
        &self,
        root: Root,
        character: u64,
        base: u64,
        revision: u64,
        prefix: &mut Vec<usize>,
    ) -> Result<u64, String> {
        let Some(digest) = root else { return Ok(0) };
        match self.read(digest)? {
            Node::Character(records) => {
                let mut previous = None;
                for record in &records {
                    super::super::validate_transaction(&record.receipt.transaction)?;
                    let identity = (record.character, record.source);
                    let hash = key(identity.0, identity.1);
                    if record.character != character
                        || record.source == [0; 32]
                        || record.receipt.revision <= base
                        || record.receipt.revision > revision
                        || previous.is_some_and(|p| p >= identity)
                        || prefix
                            .iter()
                            .enumerate()
                            .any(|(depth, index)| nibble(hash, depth) != *index)
                    {
                        return Err("Character receipt or stable index is invalid".into());
                    }
                    previous = Some(identity);
                }
                Ok(records.len() as u64)
            }
            Node::Branch(children) if prefix.len() < 64 => {
                let mut count = 0u64;
                for (index, child) in children.into_iter().enumerate() {
                    prefix.push(index);
                    count = count
                        .checked_add(
                            self.validate_character_at(child, character, base, revision, prefix)?,
                        )
                        .ok_or("Character receipt count exceeded")?;
                    prefix.pop();
                }
                Ok(count)
            }
            Node::Branch(_) => Err("Character receipt index depth exceeded".into()),
            Node::Leaf(_) => Err("Character receipt root refers to a legacy index".into()),
        }
    }
}
