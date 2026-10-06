//! Immutable checkpoints and one atomic sealed manifest select all placements.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
};
const MANIFEST_BYTES: usize = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sealed {
    manifest: Manifest,
    digest: [u8; 32],
}
pub(super) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn regular(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err("Realm input must be a regular file".into()),
    }
}
pub(super) fn secure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir(path).map_err(|_| "Cannot create realm directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| "Cannot secure realm directory")?;
        }
        if let Some(parent) = path.parent() {
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|_| "Cannot sync realm directory parent")?;
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| "Cannot inspect realm directory")?;
    if !metadata.is_dir() {
        return Err("Realm root must be a directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Realm directory requires owner-only permissions".into());
        }
    }
    Ok(())
}
pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    regular(path)?;
    let file = File::open(path).map_err(|_| "Cannot open realm checkpoint")?;
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect realm checkpoint")?
        .is_file()
    {
        return Err("Realm checkpoint is not a regular file".into());
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read realm checkpoint")?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err("Realm checkpoint exceeds byte budget".into());
    }
    Ok(bytes)
}
pub(super) fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}
fn random() -> Result<[u8; 32], String> {
    let mut id = [0; 32];
    getrandom::fill(&mut id).map_err(|_| "Cannot create realm identity")?;
    Ok(id)
}
pub(super) fn open(root: &Path) -> Result<Realm, String> {
    secure_dir(root)?;
    secure_dir(&root.join("snapshots"))?;
    secure_dir(&root.join("transfers"))?;
    secure_dir(&root.join("registry"))?;
    regular(&root.join("writer.lock"))?;
    let lock = options()
        .create(true)
        .truncate(false)
        .open(root.join("writer.lock"))
        .map_err(|_| "Cannot open realm writer lock")?;
    lock.try_lock()
        .map_err(|_| "Realm already has an authority coordinator")?;
    let history = History::open(&root.join("reward-history"))?;
    let path = root.join("realm.json");
    regular(&path)?;
    let manifest = if path.exists() {
        let sealed: Sealed = serde_json::from_slice(&read(&path, MANIFEST_BYTES)?)
            .map_err(|_| "Invalid realm manifest")?;
        if digest(
            &serde_json::to_vec(&sealed.manifest).map_err(|_| "Cannot encode realm manifest")?,
        ) != sealed.digest
        {
            return Err("Realm manifest digest mismatch".into());
        }
        sealed.manifest
    } else {
        Manifest {
            version: 2,
            id: random()?,
            revision: 0,
            last_now_ms: 0,
            next_character: 1,
            next_account: 1,
            registry_root: None,
            transfer_root: None,
            transfers: 0,
            instances: BTreeMap::new(),
            characters: BTreeMap::new(),
        }
    };
    if !matches!(manifest.version, 1 | 2)
        || manifest.version == 2 && manifest.next_account == 0
        || manifest.id == [0; 32]
        || manifest.instances.len() > INSTANCES
        || manifest.characters.len() > CHARACTERS
        || manifest.next_character == 0
    {
        return Err("Realm manifest version or budgets are incompatible".into());
    }
    let mut realm = Realm {
        root: root.to_path_buf(),
        _lock: lock,
        run: random()?,
        manifest,
        games: BTreeMap::new(),
        history,
        poisoned: false,
        dirty: std::collections::BTreeSet::new(),
        transfer_commit: false,
        lifecycle_commit: false,
        services_commit: false,
    };
    let instances: Vec<_> = realm.manifest.instances.keys().copied().collect();
    for id in &instances {
        let game = recover_game(&realm, *id)?;
        let slot = realm.manifest.instances.get_mut(id).unwrap();
        if *id == 0
            || !(1..=64).contains(&slot.capacity)
            || slot.endpoint.port() == 0
            || game.chamber.owners.len() > slot.capacity as usize
        {
            return Err("Realm instance identity or capacity is incompatible".into());
        }
        slot.epoch = slot
            .epoch
            .checked_add(1)
            .ok_or("Realm authority epochs exhausted")?;
        slot.owner = None;
        slot.expires_ms = 0;
        realm.games.insert(*id, game);
    }
    let mut owners = std::collections::BTreeSet::new();
    let mut bindings = std::collections::BTreeSet::new();
    for (id, c) in &realm.manifest.characters {
        if *id == 0
            || *id >= realm.manifest.next_character
            || !owners.insert(c.principal)
            || !bindings.insert((c.instance, c.actor))
            || realm.games.get(&c.instance).is_none_or(|g| {
                g.chamber.owners.get(&super::super::Principal(c.principal)) != Some(&c.actor)
                    || g.chamber.rewards.realm_character(c.actor) != Some(*id)
            })
        {
            return Err("Realm character placement is missing or duplicated".into());
        }
    }
    super::registry::upgrade(&mut realm)?;
    super::transfer::validate(&realm)?;
    let actual: usize = realm.games.values().map(|g| g.chamber.owners.len()).sum();
    if actual != bindings.len() {
        return Err("Realm character ownership is not completely registered".into());
    }
    publish(&mut realm, &instances)?;
    Ok(realm)
}
pub(super) fn recover_game(realm: &Realm, id: u64) -> Result<Gateway, String> {
    let slot = realm
        .manifest
        .instances
        .get(&id)
        .ok_or("Realm instance is missing")?;
    let bytes = read(
        &realm
            .root
            .join("snapshots")
            .join(format!("{}.json", hex(&slot.snapshot))),
        super::super::save::MAX_BYTES,
    )?;
    if digest(&bytes) != slot.snapshot {
        return Err("Realm instance checkpoint digest mismatch".into());
    }
    super::super::save::decode_with_history(&bytes, slot.content, id, Some(realm.history.clone()))
}
fn boundary(realm: &Realm, name: &str) {
    #[cfg(test)]
    if std::env::var("VERSE_REALM_CRASH_AT").is_ok_and(|stage| {
        stage == name
            || realm.transfer_commit && stage == format!("transfer_{name}")
            || realm.lifecycle_commit && stage == format!("lifecycle_{name}")
            || realm.services_commit && stage == format!("services_{name}")
    }) {
        std::process::exit(86);
    }
    #[cfg(not(test))]
    let _ = (realm, name);
}
pub(super) fn store_snapshot(realm: &Realm, bytes: &[u8]) -> Result<[u8; 32], String> {
    if bytes.is_empty() || bytes.len() > super::super::save::MAX_BYTES {
        return Err("Realm snapshot exceeds byte budget".into());
    }
    let hash = digest(bytes);
    let path = realm
        .root
        .join("snapshots")
        .join(format!("{}.json", hex(&hash)));
    regular(&path)?;
    if path.exists() {
        if read(&path, super::super::save::MAX_BYTES)? != bytes {
            return Err("Realm immutable checkpoint collision".into());
        }
    } else {
        let pending = realm.root.join("snapshots").join("snapshot.next");
        regular(&pending)?;
        let mut file = options()
            .create(true)
            .truncate(true)
            .open(&pending)
            .map_err(|_| "Cannot create realm instance checkpoint")?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Cannot sync realm instance checkpoint")?;
        fs::rename(&pending, &path).map_err(|_| "Cannot publish immutable realm checkpoint")?;
    }
    File::open(realm.root.join("snapshots"))
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync realm snapshot directory")?;
    Ok(hash)
}
pub(super) fn publish(realm: &mut Realm, instances: &[u64]) -> Result<(), String> {
    boundary(realm, "before_snapshots");
    for id in instances {
        let bytes = realm
            .games
            .get(id)
            .ok_or("Realm instance is missing")?
            .checkpoint()?;
        let hash = store_snapshot(realm, &bytes)?;
        realm.manifest.instances.get_mut(id).unwrap().snapshot = hash;
    }
    realm.history.synchronize()?;
    File::open(realm.root.join("snapshots"))
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync realm checkpoint directory")?;
    boundary(realm, "after_snapshots");
    realm.manifest.revision = realm
        .manifest
        .revision
        .checked_add(1)
        .ok_or("Realm revisions exhausted")?;
    let encoded =
        serde_json::to_vec(&realm.manifest).map_err(|_| "Cannot encode realm manifest")?;
    let sealed = Sealed {
        manifest: realm.manifest.clone(),
        digest: digest(&encoded),
    };
    let bytes = serde_json::to_vec(&sealed).map_err(|_| "Cannot seal realm manifest")?;
    if bytes.len() > MANIFEST_BYTES {
        return Err("Realm manifest exceeds byte budget".into());
    }
    let pending = realm.root.join("realm.next");
    regular(&pending)?;
    let mut file = options()
        .create(true)
        .truncate(true)
        .open(&pending)
        .map_err(|_| "Cannot stage realm manifest")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot sync staged realm manifest")?;
    boundary(realm, "before_seal");
    fs::rename(&pending, realm.root.join("realm.json"))
        .map_err(|_| "Cannot seal realm manifest")?;
    boundary(realm, "after_seal");
    File::open(&realm.root)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync realm manifest directory")?;
    boundary(realm, "after_directory_sync");
    for id in instances {
        realm.dirty.remove(id);
    }
    Ok(())
}
