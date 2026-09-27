//! Bounded local demo preferences. Portable door behavior owns their schema.

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::Doors;

const CAP: usize = 2048;

fn path(dir: &Path, profile: &str) -> Result<PathBuf, String> {
    if profile.is_empty()
        || profile.len() > 32
        || !profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Door profile must be 1 to 32 letters, digits, - or _".into());
    }
    Ok(dir.join(format!("{profile}-verse-plaza-doors.json")))
}

pub fn load(dir: &Path, profile: &str) -> Result<Option<String>, String> {
    let path = path(dir, profile)?;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Saved door choices could not be opened".into()),
    };
    let metadata = file
        .metadata()
        .map_err(|_| "Saved door choices could not be inspected")?;
    if !metadata.is_file() || metadata.len() > CAP as u64 {
        return Err("Saved door choices exceed their storage bounds".into());
    }
    let mut bytes = Vec::new();
    file.take((CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Saved door choices could not be read")?;
    if bytes.len() > CAP {
        return Err("Saved door choices exceed their storage bounds".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "Saved door choices are not valid text")?;
    let mut doors = Doors::default();
    doors.restore(&text)?;
    Ok(Some(doors.document()))
}

pub fn save(dir: &Path, profile: &str, document: &str) -> Result<(), String> {
    let destination = path(dir, profile)?;
    if document.len() > CAP {
        return Err("Door choices exceed their storage bounds".into());
    }
    let mut doors = Doors::default();
    doors.restore(document)?;
    let document = doors.document();
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|_| "Door choice storage could not be created")?;
    let temporary = dir.join(format!(
        ".{profile}-doors-{}.tmp",
        crate::identity::random_hex(12)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(document.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, destination)?;
        std::fs::File::open(dir)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|_| "Door choice not saved".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn preferences_are_private_atomic_and_bounded_to_the_profile() {
        let dir = std::env::temp_dir().join(format!(
            "verse-door-store-{}",
            crate::identity::random_hex(12)
        ));
        let mut doors = Doors::default();
        doors.hold(super::super::DemoItem::Bolt);
        doors.tap(super::super::DoorId::Spark);
        doors.hold(super::super::DemoItem::Ring);
        doors.tap(super::super::DoorId::Halo);
        let document = doors.document();
        assert_eq!(load(&dir, "demo").unwrap(), None);
        save(&dir, "demo", &document).unwrap();
        assert_eq!(load(&dir, "demo").unwrap(), Some(document.clone()));
        let file = path(&dir, "demo").unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(save(&dir, "../escape", &document).is_err());
        assert!(save(&dir, "demo", "{} ").is_err());
        assert_eq!(load(&dir, "demo").unwrap(), Some(document.clone()));
        let link = path(&dir, "linked").unwrap();
        symlink(&file, &link).unwrap();
        assert!(load(&dir, "linked").is_err());
        std::fs::write(&file, vec![b'x'; CAP + 1]).unwrap();
        assert!(load(&dir, "demo").is_err());
        save(&dir, "demo", &document).unwrap();
        assert_eq!(load(&dir, "demo").unwrap(), Some(document));
        doors.reset(super::super::DoorId::Spark);
        save(&dir, "demo", &doors.document()).unwrap();
        let mut restored = Doors::default();
        restored
            .restore(&load(&dir, "demo").unwrap().unwrap())
            .unwrap();
        assert_eq!(restored.state(super::super::DoorId::Spark).last, None);
        assert_eq!(
            restored.state(super::super::DoorId::Halo).last,
            Some(super::super::DemoItem::Ring)
        );
        assert_eq!(restored.held(), super::super::DemoItem::Ring);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
