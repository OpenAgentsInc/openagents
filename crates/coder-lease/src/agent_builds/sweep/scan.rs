//! Bounded metadata scans. Cached sizes inform cap checks; only complete,
//! freshly measured units become removal candidates.

use super::{Budget, Kind, Layout, Measured, Target, Unit, Watch, allocated, unit_name};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

struct Descriptor {
    paths: Vec<PathBuf>,
    profile: Option<PathBuf>,
    shown: PathBuf,
    shallow: bool,
}

impl Descriptor {
    fn loose(path: PathBuf, shallow: bool) -> Self {
        Self {
            shown: path.clone(),
            paths: vec![path],
            profile: None,
            shallow,
        }
    }

    fn unit(profile: &Path, paths: Vec<PathBuf>, deps: bool) -> Self {
        let mut shown = paths[0].clone();
        if deps {
            let stem = name(&shown).split('.').next().unwrap_or("").to_owned();
            shown = shown.with_file_name(format!("{stem}.*"));
        }
        Self {
            paths,
            profile: Some(profile.to_owned()),
            shown,
            shallow: false,
        }
    }

    fn key(&self) -> String {
        self.paths[0].display().to_string()
    }
}

pub(super) fn scan(
    path: PathBuf,
    kind: Kind,
    holder: Option<String>,
    layout: &Layout,
    budget: &Budget,
    watch: &mut Watch,
) -> Target {
    let mut target = Target {
        path,
        kind,
        bytes: 0,
        used: SystemTime::UNIX_EPOCH,
        units: Vec::new(),
        holder,
        gone: false,
        complete: false,
    };
    let Some(mut descriptors) = discover(&target.path, budget) else {
        target.bytes = watch
            .sizes
            .iter()
            .filter(|(path, _)| Path::new(path).starts_with(&target.path))
            .fold(0u64, |sum, (_, bytes)| sum.saturating_add(*bytes));
        return target;
    };
    descriptors.sort_by_cached_key(Descriptor::key);
    let known: BTreeSet<_> = descriptors.iter().map(Descriptor::key).collect();
    watch
        .sizes
        .retain(|key, _| !Path::new(key).starts_with(&target.path) || known.contains(key));
    watch
        .used
        .retain(|key, _| !Path::new(key).starts_with(&target.path) || known.contains(key));
    let target_key = target.path.display().to_string();
    let cursor = watch.cursors.get(&target_key).cloned().unwrap_or_default();
    let start = descriptors.partition_point(|descriptor| descriptor.key() <= cursor);
    target.bytes = descriptors
        .iter()
        .map(|descriptor| watch.sizes.get(&descriptor.key()).copied().unwrap_or(0))
        .sum();
    let mut fresh = 0;
    for descriptor in descriptors[start..].iter().chain(&descriptors[..start]) {
        let key = descriptor.key();
        // An oversized unit gets another turn after the other units, rather
        // than monopolizing every pass. Incomplete measurements never prune.
        watch.cursors.insert(target_key.clone(), key.clone());
        let mut measured = empty();
        let mut complete = true;
        for path in &descriptor.paths {
            let value = if descriptor.shallow {
                shallow(path, budget)
            } else {
                measure(path, budget)
            };
            let Some(value) = value else {
                complete = false;
                break;
            };
            measured.bytes = measured.bytes.saturating_add(value.bytes);
            measured.used = measured.used.max(value.used);
            measured.arm.extend(value.arm);
        }
        if !complete {
            if budget.expired() {
                break;
            }
            continue;
        }
        fresh += 1;
        let previous = watch.sizes.insert(key, measured.bytes).unwrap_or(0);
        target.bytes = target
            .bytes
            .saturating_sub(previous)
            .saturating_add(measured.bytes);
        target.used = target.used.max(measured.used);
        if let Some(profile) = &descriptor.profile
            && !descriptor.paths.iter().any(|path| layout.protected(path))
        {
            let mut unit = Unit::new(profile);
            unit.paths.clone_from(&descriptor.paths);
            unit.shown.clone_from(&descriptor.shown);
            unit.bytes = measured.bytes;
            unit.used = measured.used;
            unit.arm = measured.arm;
            target.units.push(unit);
        }
    }
    target.complete = fresh == descriptors.len();
    if target.complete && target.used == SystemTime::UNIX_EPOCH {
        if let Some(Some(meta)) = metadata(&target.path, budget)
            && let Ok(modified) = meta.modified()
        {
            target.used = modified;
        } else {
            target.complete = false;
        }
    }
    if target.complete {
        watch.cursors.remove(&target_key);
    }
    target
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn empty() -> Measured {
    Measured {
        bytes: 0,
        used: SystemTime::UNIX_EPOCH,
        arm: Vec::new(),
    }
}

/// `None` means the scan cannot safely finish; a missing path is harmless.
fn metadata(path: &Path, budget: &Budget) -> Option<Option<std::fs::Metadata>> {
    if budget.expired() {
        return None;
    }
    let result = std::fs::symlink_metadata(path);
    if budget.expired() {
        return None;
    }
    match result {
        Ok(meta) => Some(Some(meta)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(None),
        Err(_) => None,
    }
}

fn entries(path: &Path, budget: &Budget) -> Option<Vec<PathBuf>> {
    if !metadata(path, budget)?.is_some_and(|meta| meta.is_dir()) {
        return Some(Vec::new());
    }
    if budget.expired() {
        return None;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    let mut found = Vec::new();
    for entry in entries {
        if budget.expired() {
            return None;
        }
        found.push(entry.ok()?.path());
    }
    if budget.expired() {
        return None;
    }
    found.sort();
    Some(found)
}

fn is_dir(path: &Path, budget: &Budget) -> Option<bool> {
    Some(metadata(path, budget)?.is_some_and(|meta| meta.is_dir()))
}

fn dirs(path: &Path, budget: &Budget) -> Option<Vec<PathBuf>> {
    let mut found = Vec::new();
    for child in entries(path, budget)? {
        if is_dir(&child, budget)? {
            found.push(child);
        }
    }
    Some(found)
}

fn profiles(path: &Path, budget: &Budget) -> Option<Vec<PathBuf>> {
    let mut found = Vec::new();
    for child in dirs(path, budget)? {
        if is_dir(&child.join("deps"), budget)? || is_dir(&child.join(".fingerprint"), budget)? {
            found.push(child);
        } else {
            for nested in dirs(&child, budget)? {
                if is_dir(&nested.join("deps"), budget)?
                    || is_dir(&nested.join(".fingerprint"), budget)?
                {
                    found.push(nested);
                }
            }
        }
    }
    for bucket in dirs(&path.join("worktrees"), budget)? {
        for workspace in dirs(&bucket, budget)? {
            found.extend(profiles(&workspace, budget)?);
        }
    }
    found.sort();
    found.dedup();
    Some(found)
}

fn discover(path: &Path, budget: &Budget) -> Option<Vec<Descriptor>> {
    let profiles = profiles(path, budget)?;
    let profile_set: BTreeSet<_> = profiles.iter().cloned().collect();
    let mut descriptors = Vec::new();
    let mut stack = vec![path.to_owned()];
    while let Some(dir) = stack.pop() {
        descriptors.push(Descriptor::loose(dir.clone(), true));
        for child in entries(&dir, budget)? {
            if child
                .file_name()
                .is_some_and(|name| name == ".metadata_never_index")
            {
                descriptors.push(Descriptor::loose(child, true));
                continue;
            }
            if profile_set.contains(&child) {
                continue;
            }
            if is_dir(&child, budget)? && profiles.iter().any(|profile| profile.starts_with(&child))
            {
                stack.push(child);
            } else {
                descriptors.push(Descriptor::loose(child, false));
            }
        }
    }
    for profile in profiles {
        discover_profile(&profile, budget, &mut descriptors)?;
    }
    Some(descriptors)
}

fn discover_profile(
    profile: &Path,
    budget: &Budget,
    descriptors: &mut Vec<Descriptor>,
) -> Option<()> {
    descriptors.push(Descriptor::loose(profile.to_owned(), true));
    let fingerprint_root = profile.join(".fingerprint");
    let fingerprint_dir = is_dir(&fingerprint_root, budget)?;
    let mut fingerprints = BTreeMap::new();
    if fingerprint_dir {
        descriptors.push(Descriptor::loose(fingerprint_root.clone(), true));
        for path in entries(&fingerprint_root, budget)? {
            fingerprints.insert(name(&path), path);
        }
    }
    let mut deps: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut units = Vec::new();
    for child in entries(profile, budget)? {
        let name = name(&child);
        let directory = is_dir(&child, budget)?;
        if name == ".fingerprint" && fingerprint_dir {
            continue;
        } else if name == "deps" && directory {
            descriptors.push(Descriptor::loose(child.clone(), true));
            for file in entries(&child, budget)? {
                deps.entry(unit_name(&self::name(&file)))
                    .or_default()
                    .push(file);
            }
        } else if name == "build" && directory {
            descriptors.push(Descriptor::loose(child.clone(), true));
            for path in entries(&child, budget)? {
                units.push((Some(self::name(&path)), vec![path], false));
            }
        } else if name.starts_with("incremental") && directory {
            descriptors.push(Descriptor::loose(child.clone(), true));
            for session in entries(&child, budget)? {
                let nested = !self::name(&session).contains('-') && is_dir(&session, budget)?;
                if nested {
                    descriptors.push(Descriptor::loose(session.clone(), true));
                    for path in entries(&session, budget)? {
                        units.push((None, vec![path], false));
                    }
                } else {
                    units.push((None, vec![session], false));
                }
            }
        } else {
            descriptors.push(Descriptor::loose(child, false));
        }
    }
    units.extend(
        deps.into_iter()
            .map(|(name, paths)| (Some(name), paths, true)),
    );
    for (name, mut paths, deps) in units {
        if let Some(path) = name.and_then(|name| fingerprints.remove(&name)) {
            paths.push(path);
        }
        descriptors.push(Descriptor::unit(profile, paths, deps));
    }
    for (_, path) in fingerprints {
        descriptors.push(Descriptor::unit(profile, vec![path], false));
    }
    Some(())
}

fn shallow(path: &Path, budget: &Budget) -> Option<Measured> {
    let mut found = empty();
    if let Some(meta) = metadata(path, budget)? {
        found.bytes = allocated(&meta);
    }
    Some(found)
}

/// A partial tree contributes no age, size, or removal paths.
pub(super) fn measure(path: &Path, budget: &Budget) -> Option<Measured> {
    let mut found = empty();
    let mut stack = vec![path.to_owned()];
    let mut own = None;
    while let Some(path) = stack.pop() {
        let Some(meta) = metadata(&path, budget)? else {
            continue;
        };
        if own.is_none() {
            own = meta.modified().ok();
        }
        found.bytes = found.bytes.saturating_add(allocated(&meta));
        if meta.is_dir() {
            stack.extend(entries(&path, budget)?);
            continue;
        }
        let modified = meta.modified().ok()?;
        let accessed = meta.accessed().ok()?;
        found.used = found.used.max(modified).max(accessed);
        if accessed >= modified {
            found.arm.push(path);
        }
    }
    if found.used == SystemTime::UNIX_EPOCH {
        found.used = own.unwrap_or(SystemTime::UNIX_EPOCH);
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7; 4_096]).unwrap();
    }

    #[test]
    fn complete_scan_counts_nested_state_and_keeps_final_outputs_out_of_units() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::at(home.path(), &home.path().join("leases"));
        let target = layout.work.join("openagents-target-1");
        for suffix in [
            "debug/deps/libfoo-aa.rlib",
            "debug/deps/foo-aa.d",
            "debug/.fingerprint/foo-aa/lib-foo",
            "debug/build/build-bb/output",
            "debug/incremental/v1/session-cc/object.o",
            "debug/final-output",
            "worktrees/ab/cdef/.openagents-workspace",
            "worktrees/ab/cdef/debug/deps/libstate-dd.rlib",
        ] {
            write(&target.join(suffix));
        }
        let observed = scan(
            target.clone(),
            Kind::Shared,
            None,
            &layout,
            &Budget::unlimited(),
            &mut Watch::default(),
        );
        assert!(observed.complete);
        assert_eq!(observed.bytes, super::super::measure(&target).bytes);
        assert_eq!(observed.units.len(), 4);
        assert!(observed.units.iter().any(|unit| {
            unit.paths
                .contains(&target.join("debug/deps/libfoo-aa.rlib"))
                && unit
                    .paths
                    .contains(&target.join("debug/.fingerprint/foo-aa"))
        }));
        assert!(observed.units.iter().all(|unit| {
            !unit.paths.contains(&target.join("debug/final-output"))
                && !unit
                    .paths
                    .contains(&target.join("worktrees/ab/cdef/.openagents-workspace"))
        }));
    }

    #[test]
    fn partial_scans_keep_size_estimates_and_rotate_fresh_candidates() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::at(home.path(), &home.path().join("leases"));
        let target = layout.work.join("openagents-target-1");
        for suffix in ["a-aa.rlib", "b-bb.rlib", "c-cc.rlib"] {
            write(&target.join("debug/deps").join(suffix));
        }
        let mut watch = Watch::default();
        let full = scan(
            target.clone(),
            Kind::Shared,
            None,
            &layout,
            &Budget::unlimited(),
            &mut watch,
        );
        assert!(full.complete);
        // Count discovery work once, then allow only four metadata reads.
        // This stays deterministic without a wall-clock assertion or sleep.
        let inventory = Budget::with_steps(1_000_000);
        discover(&target, &inventory).unwrap();
        let discovery_steps = 1_000_000 - inventory.steps.get().unwrap();
        let mut seen = BTreeSet::new();
        let mut candidates_were_partial = false;
        for _ in 0..4 {
            let partial = scan(
                target.clone(),
                Kind::Shared,
                None,
                &layout,
                &Budget::with_steps(discovery_steps + 8),
                &mut watch,
            );
            assert!(!partial.complete);
            assert_eq!(partial.bytes, full.bytes);
            candidates_were_partial |= partial.units.len() < full.units.len();
            seen.extend(partial.units.iter().map(Unit::key));
            assert!(watch.cursors.contains_key(&target.display().to_string()));
        }
        assert!(candidates_were_partial);
        assert_eq!(seen, full.units.iter().map(Unit::key).collect());
        let completed = scan(
            target.clone(),
            Kind::Shared,
            None,
            &layout,
            &Budget::unlimited(),
            &mut watch,
        );
        assert!(completed.complete);
        assert!(!watch.cursors.contains_key(&target.display().to_string()));
    }

    #[test]
    fn expired_measurement_returns_no_partial_tree() {
        let home = tempfile::tempdir().unwrap();
        write(&home.path().join("nested/artifact"));
        assert!(measure(home.path(), &Budget::with_steps(2)).is_none());
        assert!(home.path().join("nested/artifact").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_fingerprint_symlink_never_becomes_a_removal_unit() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::at(home.path(), &home.path().join("leases"));
        let target = layout.work.join("openagents-target-1");
        write(&target.join("debug/deps/libfoo-aa.rlib"));
        let outside = home.path().join("outside");
        write(&outside.join("foo-aa/keep"));
        std::os::unix::fs::symlink(&outside, target.join("debug/.fingerprint")).unwrap();
        let observed = scan(
            target,
            Kind::Shared,
            None,
            &layout,
            &Budget::unlimited(),
            &mut Watch::default(),
        );
        assert!(observed.complete);
        assert_eq!(observed.units.len(), 1);
        assert_eq!(observed.units[0].paths.len(), 1);
        assert!(outside.join("foo-aa/keep").is_file());
    }
}
