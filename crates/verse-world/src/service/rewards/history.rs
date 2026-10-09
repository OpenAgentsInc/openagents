//! Immutable receipt index whose root is committed with character state.
use super::Receipt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

mod character;

const LEAF_RECEIPTS: usize = 16;
const NODE_BYTES: usize = 256 * 1024;
pub(in crate::service) const PENDING_BYTES: usize = 128 * 1024 * 1024;
pub(super) type Root = Option<[u8; 32]>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Node {
    Leaf(Vec<Receipt>),
    Character(Vec<character::Indexed>),
    Branch([Root; 16]),
}

struct Directory {
    path: PathBuf,
    temporary: bool,
    deferred: AtomicBool,
    pending: Mutex<Pending>,
}
#[derive(Default)]
struct Pending {
    nodes: BTreeMap<[u8; 32], Arc<Vec<u8>>>,
    bytes: usize,
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Clones share files, while each ledger retains its own immutable index root.
#[derive(Clone)]
pub(in crate::service) struct History(Arc<Directory>);
impl History {
    pub(in crate::service) fn open(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            std::fs::create_dir(path).map_err(|_| "Cannot create reward history directory")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| "Cannot secure reward history directory")?;
            }
            if let Some(parent) = path.parent() {
                File::open(parent)
                    .and_then(|f| f.sync_all())
                    .map_err(|_| "Cannot sync reward history directory creation")?;
            }
        }
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|_| "Cannot inspect reward history directory")?;
        if !metadata.is_dir() {
            return Err("Reward history must be a directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err("Reward history requires owner-only permissions".into());
            }
        }
        Ok(Self(Arc::new(Directory {
            path: path.to_path_buf(),
            temporary: false,
            deferred: AtomicBool::new(false),
            pending: Mutex::new(Pending::default()),
        })))
    }
    pub(in crate::service) fn temporary() -> Result<Self, String> {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(|_| "Cannot create reward history identity")?;
        let path = std::env::temp_dir().join(format!("verse-rewards-{}", hex(&nonce)));
        std::fs::create_dir(&path)
            .map_err(|_| "Cannot create temporary reward history directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "Cannot secure temporary reward history directory")?;
        }
        let mut history = Self::open(&path)?;
        Arc::get_mut(&mut history.0).unwrap().temporary = true;
        Ok(history)
    }
    pub(in crate::service) fn reachable(
        &self,
        root: Option<[u8; 32]>,
        nodes: &mut std::collections::BTreeSet<[u8; 32]>,
        limit: usize,
    ) -> Result<(), String> {
        fn visit(
            history: &History,
            root: Option<[u8; 32]>,
            nodes: &mut std::collections::BTreeSet<[u8; 32]>,
            limit: usize,
            depth: usize,
        ) -> Result<(), String> {
            let Some(root) = root else {
                return Ok(());
            };
            if depth > 64 {
                return Err("Backup reward index depth exceeded".into());
            }
            if nodes.contains(&root) {
                return Ok(());
            }
            if nodes.len() >= limit {
                return Err("Backup reachable history exceeds file budget".into());
            }
            nodes.insert(root);
            if let Node::Branch(children) = history.read_disk(root)? {
                for child in children {
                    visit(history, child, nodes, limit, depth + 1)?;
                }
            }
            Ok(())
        }
        visit(self, root, nodes, limit, 0)
    }
    pub(in crate::service) fn verify_node(&self, digest: [u8; 32]) -> Result<(), String> {
        self.read_disk(digest).map(|_| ())
    }
    pub(super) fn same_directory(&self, other: &Self) -> bool {
        self.0.path == other.0.path
    }
    pub(in crate::service) fn defer_writes(&self) {
        self.0.deferred.store(true, Ordering::Release);
    }
    pub(in crate::service) fn has_capacity(&self) -> Result<bool, String> {
        let pending = self
            .0
            .pending
            .lock()
            .map_err(|_| "Reward history staging is unavailable")?;
        Ok(pending.bytes < PENDING_BYTES / 2)
    }
    /// Publish staged immutable nodes before a commit can reference their roots.
    pub(in crate::service) fn synchronize(&self) -> Result<(), String> {
        let nodes: Vec<_> = self
            .0
            .pending
            .lock()
            .map_err(|_| "Reward history staging is unavailable")?
            .nodes
            .iter()
            .map(|(digest, bytes)| (*digest, bytes.clone()))
            .collect();
        self.publish_all(&nodes)?;
        if !nodes.is_empty() {
            File::open(&self.0.path)
                .and_then(|f| f.sync_all())
                .map_err(|_| "Cannot sync reward history directory")?;
            let mut pending = self
                .0
                .pending
                .lock()
                .map_err(|_| "Reward history staging is unavailable")?;
            for (digest, _) in nodes {
                if let Some(bytes) = pending.nodes.remove(&digest) {
                    pending.bytes -= bytes.len();
                }
            }
        }
        Ok(())
    }
    fn read(&self, digest: [u8; 32]) -> Result<Node, String> {
        let staged = self
            .0
            .pending
            .lock()
            .map_err(|_| "Reward history staging is unavailable")?
            .nodes
            .get(&digest)
            .cloned();
        if let Some(bytes) = staged {
            return serde_json::from_slice(&bytes)
                .map_err(|_| "Invalid staged reward history node".into());
        }
        self.read_disk(digest)
    }
    fn read_disk(&self, digest: [u8; 32]) -> Result<Node, String> {
        let path = self.0.path.join(hex(&digest));
        if !std::fs::symlink_metadata(&path)
            .map_err(|_| "Reward history node is missing")?
            .is_file()
        {
            return Err("Reward history node must be a regular file".into());
        }
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|f| f.take(NODE_BYTES as u64 + 1).read_to_end(&mut bytes))
            .map_err(|_| "Cannot read reward history node")?;
        if bytes.len() > NODE_BYTES || hash(&bytes) != digest {
            return Err("Reward history node size or digest is invalid".into());
        }
        let node: Node =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid reward history node")?;
        if matches!(&node, Node::Leaf(receipts) if receipts.len() > LEAF_RECEIPTS) {
            return Err("Reward history leaf budget exceeded".into());
        }
        if matches!(&node,Node::Character(records) if records.len()>LEAF_RECEIPTS) {
            return Err("Character receipt leaf budget exceeded".into());
        }
        Ok(node)
    }
    fn write(&self, node: &Node) -> Result<[u8; 32], String> {
        let bytes = serde_json::to_vec(node).map_err(|_| "Cannot encode reward history node")?;
        if bytes.len() > NODE_BYTES {
            return Err("Reward history node byte budget exceeded".into());
        }
        let digest = hash(&bytes);
        if self.0.deferred.load(Ordering::Acquire) {
            let mut pending = self
                .0
                .pending
                .lock()
                .map_err(|_| "Reward history staging is unavailable")?;
            if !pending.nodes.contains_key(&digest) {
                if pending.bytes + bytes.len() > PENDING_BYTES {
                    return Err("Reward history staging byte budget exceeded".into());
                }
                pending.bytes += bytes.len();
                pending.nodes.insert(digest, Arc::new(bytes));
            }
            return Ok(digest);
        }
        self.publish(digest, &bytes)?;
        Ok(digest)
    }
    /// Publishes staged nodes in three passes: write every new node, sync each,
    /// then rename. One node at a time paid a full journal commit per sync,
    /// which stalled a battle's commit for 0.6-0.7 s on a busy disk (#10559).
    /// A node still becomes visible only after its own bytes are durable.
    fn publish_all(&self, nodes: &[([u8; 32], Arc<Vec<u8>>)]) -> Result<(), String> {
        let mut written: Vec<(PathBuf, PathBuf, File)> = Vec::new();
        let result = (|| {
            for (digest, bytes) in nodes {
                let path = self.0.path.join(hex(digest));
                if path.exists() {
                    self.read_disk(*digest)?;
                    continue;
                }
                let mut nonce = [0; 16];
                getrandom::fill(&mut nonce)
                    .map_err(|_| "Cannot create reward history write identity")?;
                let pending = self.0.path.join(format!("next-{}", hex(&nonce)));
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options
                    .open(&pending)
                    .map_err(|_| "Cannot create reward history node")?;
                let wrote = file.write_all(bytes);
                written.push((pending, path, file));
                wrote.map_err(|_| "Cannot write and sync reward history node")?;
            }
            for (_, _, file) in &written {
                file.sync_all()
                    .map_err(|_| "Cannot write and sync reward history node")?;
            }
            while let Some((pending, path, _)) = written.first() {
                std::fs::rename(pending, path).map_err(|_| "Cannot publish reward history node")?;
                written.remove(0);
            }
            Ok(())
        })();
        for (pending, _, _) in written {
            let _ = std::fs::remove_file(pending);
        }
        result
    }
    fn publish(&self, digest: [u8; 32], bytes: &[u8]) -> Result<(), String> {
        let path = self.0.path.join(hex(&digest));
        if path.exists() {
            self.read_disk(digest)?;
            return Ok(());
        }
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(|_| "Cannot create reward history write identity")?;
        let pending = self.0.path.join(format!("next-{}", hex(&nonce)));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&pending)
                .map_err(|_| "Cannot create reward history node")?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Cannot write and sync reward history node")?;
            std::fs::rename(&pending, path).map_err(|_| "Cannot publish reward history node")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(pending);
        }
        result
    }
    pub(super) fn get(
        &self,
        mut root: Root,
        actor: u64,
        source: [u8; 32],
    ) -> Result<Option<Receipt>, String> {
        let key = key(actor, source);
        for depth in 0..=64 {
            let Some(digest) = root else { return Ok(None) };
            match self.read(digest)? {
                Node::Leaf(receipts) => {
                    return Ok(receipts
                        .into_iter()
                        .find(|r| r.transaction.actor == actor && r.transaction.source == source));
                }
                Node::Branch(children) if depth < 64 => root = children[nibble(key, depth)],
                Node::Branch(_) => return Err("Reward history index depth exceeded".into()),
                Node::Character(_) => {
                    return Err("Legacy reward lookup cannot read a character index".into());
                }
            }
        }
        Err("Reward history index depth exceeded".into())
    }
    pub(super) fn insert(&self, root: Root, receipts: &[Receipt]) -> Result<Root, String> {
        if receipts.is_empty() {
            return Ok(root);
        }
        if receipts.len() > super::ACTIVE_RECEIPTS {
            return Err("Reward history batch budget exceeded".into());
        }
        let root = self.insert_at(root, receipts.to_vec(), 0)?;
        if !self.0.deferred.load(Ordering::Acquire) {
            File::open(&self.0.path)
                .and_then(|f| f.sync_all())
                .map_err(|_| "Cannot sync reward history directory")?;
        }
        Ok(Some(root))
    }
    pub(super) fn validate(&self, root: Root, revision: u64) -> Result<(), String> {
        let mut prefix = Vec::new();
        let count = self.validate_at(root, revision, &mut prefix)?;
        if count != revision {
            return Err("Reward history revision count is incompatible".into());
        }
        Ok(())
    }
    fn validate_at(
        &self,
        root: Root,
        revision: u64,
        prefix: &mut Vec<usize>,
    ) -> Result<u64, String> {
        let Some(digest) = root else { return Ok(0) };
        match self.read(digest)? {
            Node::Leaf(receipts) => {
                let mut previous = None;
                for receipt in &receipts {
                    super::validate_transaction(&receipt.transaction)?;
                    let identity = (receipt.transaction.actor, receipt.transaction.source);
                    let key = key(identity.0, identity.1);
                    if receipt.revision == 0
                        || receipt.revision > revision
                        || previous.is_some_and(|p| p >= identity)
                        || prefix
                            .iter()
                            .enumerate()
                            .any(|(depth, index)| nibble(key, depth) != *index)
                    {
                        return Err("Reward history receipt or index is invalid".into());
                    }
                    previous = Some(identity);
                }
                Ok(receipts.len() as u64)
            }
            Node::Branch(children) if prefix.len() < 64 => {
                let mut count = 0u64;
                for (index, child) in children.into_iter().enumerate() {
                    prefix.push(index);
                    count = count
                        .checked_add(self.validate_at(child, revision, prefix)?)
                        .ok_or("Reward history receipt count exceeded")?;
                    prefix.pop();
                }
                Ok(count)
            }
            Node::Branch(_) => Err("Reward history index depth exceeded".into()),
            Node::Character(_) => {
                Err("Legacy reward validation cannot read a character index".into())
            }
        }
    }
    fn insert_at(
        &self,
        root: Root,
        mut receipts: Vec<Receipt>,
        depth: usize,
    ) -> Result<[u8; 32], String> {
        let node = root
            .map(|r| self.read(r))
            .transpose()?
            .unwrap_or(Node::Leaf(vec![]));
        let children = match node {
            Node::Leaf(existing) => {
                receipts.extend(existing);
                receipts.sort_by_key(|r| (r.transaction.actor, r.transaction.source));
                if receipts.windows(2).any(|rs| {
                    rs[0].transaction.actor == rs[1].transaction.actor
                        && rs[0].transaction.source == rs[1].transaction.source
                }) {
                    return Err("Reward history source is already indexed".into());
                }
                if receipts.len() <= LEAF_RECEIPTS {
                    return self.write(&Node::Leaf(receipts));
                }
                [None; 16]
            }
            Node::Branch(children) => children,
            Node::Character(_) => {
                return Err("Cannot insert legacy rewards into a character index".into());
            }
        };
        if depth >= 64 {
            return Err("Reward history index depth exceeded".into());
        }
        let mut groups: [Vec<Receipt>; 16] = std::array::from_fn(|_| Vec::new());
        for receipt in receipts {
            groups[nibble(
                key(receipt.transaction.actor, receipt.transaction.source),
                depth,
            )]
            .push(receipt);
        }
        let mut children = children;
        for (index, receipts) in groups.into_iter().enumerate() {
            if !receipts.is_empty() {
                children[index] = Some(self.insert_at(children[index], receipts, depth + 1)?);
            }
        }
        self.write(&Node::Branch(children))
    }
}
fn key(actor: u64, source: [u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"verse.reward.source.v1\0");
    h.update(actor.to_be_bytes());
    h.update(source);
    h.finalize().into()
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"verse.reward.history.v1\0");
    h.update(bytes);
    h.finalize().into()
}
fn nibble(key: [u8; 32], depth: usize) -> usize {
    let byte = key[depth / 2];
    if depth % 2 == 0 {
        (byte >> 4) as usize
    } else {
        (byte & 15) as usize
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
