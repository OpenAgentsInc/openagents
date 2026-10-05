//! Bounded structural changes over a committed checkpoint, with a digest chain.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
};

pub(super) const INTERVAL: usize = 256;
pub(super) const LOG_BYTES: u64 = 64 * 1024 * 1024;
const RECORD_BYTES: usize = super::FILE_BYTES;
const CHANGES: usize = 65_536;
const DEPTH: usize = 64;

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Change {
    Put { path: Vec<String>, value: Value },
    Remove { path: Vec<String> },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    revision: u64,
    parent: [u8; 32],
    state: [u8; 32],
    changes: Vec<Change>,
    digest: [u8; 32],
}
fn seal(record: &Record) -> Result<[u8; 32], String> {
    let mut h = Sha256::new();
    h.update(b"verse.chamber.journal.v1\0");
    h.update(record.version.to_be_bytes());
    h.update(record.revision.to_be_bytes());
    h.update(record.parent);
    h.update(record.state);
    h.update(
        serde_json::to_vec(&record.changes).map_err(|_| "Cannot encode chamber journal changes")?,
    );
    Ok(h.finalize().into())
}
pub(super) fn expand(bytes: &[u8]) -> Result<Value, String> {
    let mut value: Value =
        serde_json::from_slice(bytes).map_err(|_| "Invalid chamber checkpoint")?;
    let world = value
        .get_mut("world")
        .ok_or("Chamber checkpoint has no world")?;
    let bytes = world
        .as_str()
        .ok_or("Chamber checkpoint world is invalid")?
        .as_bytes();
    *world = serde_json::from_slice(bytes).map_err(|_| "Invalid chamber checkpoint world")?;
    Ok(value)
}
pub(super) fn contract(value: &Value) -> Result<Vec<u8>, String> {
    let mut value = value.clone();
    let world = value
        .get_mut("world")
        .ok_or("Chamber checkpoint has no world")?;
    *world =
        Value::String(serde_json::to_string(world).map_err(|_| "Cannot encode checkpoint world")?);
    let bytes = serde_json::to_vec(&value).map_err(|_| "Cannot encode chamber checkpoint")?;
    if bytes.len() > super::MAX_BYTES {
        return Err("Saved chamber byte budget exceeded".into());
    }
    Ok(bytes)
}
pub(super) fn hash(value: &Value) -> Result<[u8; 32], String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "Cannot hash chamber checkpoint")?;
    if bytes.len() > super::MAX_BYTES {
        return Err("Chamber journal state byte budget exceeded".into());
    }
    Ok(Sha256::digest(bytes).into())
}
fn diff(
    old: &Value,
    new: &Value,
    path: &mut Vec<String>,
    changes: &mut Vec<Change>,
) -> Result<(), String> {
    if path.len() > DEPTH || changes.len() >= CHANGES {
        return Err("Chamber journal change budget exceeded".into());
    }
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => {
            for (name, value) in new {
                path.push(name.clone());
                if let Some(prior) = old.get(name) {
                    diff(prior, value, path, changes)?;
                } else {
                    changes.push(Change::Put {
                        path: path.clone(),
                        value: value.clone(),
                    });
                }
                path.pop();
            }
            for name in old.keys().filter(|name| !new.contains_key(*name)) {
                path.push(name.clone());
                changes.push(Change::Remove { path: path.clone() });
                path.pop();
            }
        }
        (Value::Array(old), Value::Array(new)) if old.len() == new.len() => {
            for (index, (old, new)) in old.iter().zip(new).enumerate() {
                path.push(index.to_string());
                diff(old, new, path, changes)?;
                path.pop();
            }
        }
        // Signed zero compares equal, but its persisted representation and digest differ.
        (Value::Number(old), Value::Number(new)) if old.to_string() == new.to_string() => {}
        _ if old == new && !old.is_number() => {}
        _ => changes.push(Change::Put {
            path: path.clone(),
            value: new.clone(),
        }),
    }
    if changes.len() > CHANGES {
        return Err("Chamber journal change budget exceeded".into());
    }
    Ok(())
}
fn apply(value: &mut Value, change: Change) -> Result<(), String> {
    let (path, replacement) = match change {
        Change::Put { path, value } => (path, Some(value)),
        Change::Remove { path } => (path, None),
    };
    if path.len() > DEPTH {
        return Err("Chamber journal path budget exceeded".into());
    }
    if path.is_empty() {
        *value = replacement.ok_or("Cannot remove the chamber checkpoint root")?;
        return Ok(());
    }
    let (name, ancestors) = path.split_last().unwrap();
    let mut parent = value;
    for name in ancestors {
        parent = match parent {
            Value::Object(map) => map.get_mut(name),
            Value::Array(values) => name.parse::<usize>().ok().and_then(|i| values.get_mut(i)),
            _ => None,
        }
        .ok_or("Chamber journal path is absent")?;
    }
    match parent {
        Value::Object(map) => {
            if let Some(value) = replacement {
                map.insert(name.clone(), value);
            } else if map.remove(name).is_none() {
                return Err("Chamber journal removed an absent field".into());
            }
        }
        Value::Array(values) => {
            let index = name
                .parse::<usize>()
                .map_err(|_| "Invalid chamber journal array index")?;
            let target = values
                .get_mut(index)
                .ok_or("Chamber journal array index is absent")?;
            *target = replacement.ok_or("Chamber journal cannot remove an array element")?;
        }
        _ => return Err("Chamber journal parent is not a container".into()),
    }
    Ok(())
}
pub(super) fn append(
    file: &mut File,
    revision: u64,
    old: &Value,
    new: &Value,
    parent_hash: [u8; 32],
    state_hash: [u8; 32],
) -> Result<usize, String> {
    let mut changes = Vec::new();
    diff(old, new, &mut Vec::new(), &mut changes)?;
    let mut record = Record {
        version: 1,
        revision,
        parent: parent_hash,
        state: state_hash,
        changes,
        digest: [0; 32],
    };
    record.digest = seal(&record)?;
    let mut bytes =
        serde_json::to_vec(&record).map_err(|_| "Cannot encode chamber journal record")?;
    bytes.push(b'\n');
    if bytes.len() > RECORD_BYTES {
        return Err("Chamber journal record byte budget exceeded".into());
    }
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot append and sync chamber journal record")?;
    Ok(bytes.len())
}
/// Replay complete records; only an unterminated final append can be discarded.
pub(super) fn replay(
    file: &mut File,
    mut revision: u64,
    value: &mut Value,
) -> Result<usize, String> {
    if file
        .metadata()
        .map_err(|_| "Cannot inspect chamber journal")?
        .len()
        > LOG_BYTES + RECORD_BYTES as u64
    {
        return Err("Chamber journal byte budget exceeded".into());
    }
    let snapshot_revision = revision;
    let mut reader = BufReader::new(
        file.try_clone()
            .map_err(|_| "Cannot open chamber journal reader")?,
    );
    let mut offset = 0u64;
    let mut count = 0usize;
    let mut applied = 0usize;
    let mut head = hash(value)?;
    loop {
        let mut bytes = Vec::new();
        let size = Read::by_ref(&mut reader)
            .take(RECORD_BYTES as u64 + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| "Cannot read chamber journal")?;
        if size == 0 {
            break;
        }
        if bytes.len() > RECORD_BYTES {
            return Err("Chamber journal record byte budget exceeded".into());
        }
        if bytes.last() != Some(&b'\n') {
            file.set_len(offset)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Cannot discard interrupted chamber journal append")?;
            break;
        }
        count += 1;
        if count > INTERVAL + 1 {
            return Err("Chamber journal record budget exceeded".into());
        }
        let record: Record = serde_json::from_slice(&bytes)
            .map_err(|_| "Invalid committed chamber journal record")?;
        if record.version != 1
            || record.revision == 0
            || record.changes.len() > CHANGES
            || seal(&record)? != record.digest
        {
            return Err("Chamber journal record version or digest is invalid".into());
        }
        if record.revision > snapshot_revision {
            if record.revision
                != revision
                    .checked_add(1)
                    .ok_or("Chamber journal revisions exhausted")?
                || record.parent != head
            {
                return Err("Chamber journal revision chain is incompatible".into());
            }
            for change in record.changes {
                apply(value, change)?;
            }
            head = hash(value)?;
            if head != record.state {
                return Err("Chamber journal state digest is incompatible".into());
            }
            revision = record.revision;
            applied += 1;
        }
        offset += size as u64;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn journal_preserves_signed_zero_inside_otherwise_equal_containers() {
        let old = serde_json::json!({"positions":[{"x":-0.0}]});
        let new = serde_json::json!({"positions":[{"x":0.0}]});
        assert_eq!(old, new);
        assert_ne!(hash(&old).unwrap(), hash(&new).unwrap());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal");
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)
            .unwrap();
        append(
            &mut file,
            2,
            &old,
            &new,
            hash(&old).unwrap(),
            hash(&new).unwrap(),
        )
        .unwrap();
        let mut file = File::open(path).unwrap();
        let mut recovered = old;
        assert_eq!(replay(&mut file, 1, &mut recovered).unwrap(), 1);
        assert_eq!(hash(&recovered).unwrap(), hash(&new).unwrap());
    }
}
