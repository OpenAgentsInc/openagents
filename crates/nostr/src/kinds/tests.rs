use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::{REGISTRY, claim_of};

fn nips() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nips")
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Every OpenAgents specification, as (file stem, text).
fn specifications() -> Vec<(String, String)> {
    let dir = nips().join("openagents");
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("nips/openagents")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?.to_owned();
            (stem.starts_with("NIP-") || stem == "contracts").then(|| (stem, read(path)))
        })
        .collect();
    files.sort();
    files
}

/// The numbers written as `` `N` `` in `text`.
fn backticked(text: &str) -> Vec<u16> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|span| span.parse().ok())
        .collect()
}

/// The kind a table row declares in its first cell, as `` | `N` | ``.
fn row_kind(line: &str) -> Option<u16> {
    let cell = line.strip_prefix('|')?.split('|').next()?.trim();
    cell.strip_prefix('`')?.strip_suffix('`')?.parse().ok()
}

/// The kind a prose line or heading declares: "Kind `N` is" or
/// "— kind `N`".
fn prose_kind(line: &str) -> Option<u16> {
    let rest = line
        .split_once("Kind `")
        .filter(|(_, rest)| rest.contains("` is"))
        .or_else(|| line.strip_prefix('#').and_then(|l| l.split_once("kind `")))?
        .1;
    rest.split('`').next()?.parse().ok()
}

/// The official NIP registry's single kinds and inclusive ranges.
fn official_kinds() -> Vec<(u32, u32)> {
    let readme = read(nips().join("official/README.md"));
    let table = readme
        .split("## Event Kinds")
        .nth(1)
        .expect("official Event Kinds section");
    let mut kinds = Vec::new();
    for line in table.lines().take_while(|l| !l.starts_with("## ")) {
        let Some(cell) = line.strip_prefix('|').and_then(|l| l.split('|').next()) else {
            continue;
        };
        let cell = cell.trim().trim_matches('`');
        let (low, high) = cell.split_once('-').unwrap_or((cell, cell));
        if let (Ok(low), Ok(high)) = (low.trim().parse(), high.trim().parse()) {
            kinds.push((low, high));
        }
    }
    assert!(kinds.len() > 100, "parsed the official kinds table");
    kinds
}

#[test]
fn no_kind_is_claimed_twice() {
    let mut seen = BTreeMap::new();
    for claim in REGISTRY {
        if let Some(owner) = seen.insert(claim.kind, claim.owner) {
            panic!(
                "kind {} is claimed by both {owner} and {}",
                claim.kind, claim.owner
            );
        }
    }
    assert_eq!(claim_of(3_195).map(|c| c.owner), Some("NIP-EVAL"));
    assert_eq!(claim_of(3_197).map(|c| c.owner), Some("NIP-XP"));
}

#[test]
fn the_readme_registry_lists_exactly_the_registry() {
    let readme = read(nips().join("openagents/README.md"));
    let section = readme
        .split("## Kind registry")
        .nth(1)
        .expect("a Kind registry section in nips/openagents/README.md");
    let mut listed = Vec::new();
    for line in section.lines().take_while(|l| !l.starts_with("## ")) {
        let Some(kind) = row_kind(line) else { continue };
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let owner = cells[2]
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or_default();
        listed.push((kind, owner.to_owned(), cells[3].to_owned()));
    }
    let registry: Vec<(u16, String, String)> = REGISTRY
        .iter()
        .map(|c| (c.kind, c.owner.to_owned(), c.meaning.to_owned()))
        .collect();
    assert_eq!(
        listed, registry,
        "the README Kind registry must list REGISTRY in order"
    );
}

#[test]
fn every_kind_a_nip_declares_is_registered_to_it_or_names_its_owner() {
    for (stem, text) in specifications() {
        for (number, line) in text.lines().enumerate() {
            let declared = row_kind(line)
                .map(|k| (k, true))
                .or_else(|| prose_kind(line).map(|k| (k, false)));
            let Some((kind, row)) = declared else {
                continue;
            };
            let at = format!("{stem}.md:{}", number + 1);
            match claim_of(kind) {
                Some(claim) if claim.owner == stem => {}
                // A row may restate a kind another specification owns if
                // it names that owner, as NIP-KB does for NIP-EVAL `3189`.
                Some(claim) if row && names(line, claim.owner) => {}
                Some(claim) => panic!(
                    "{at} declares kind {kind}, which {} owns: rename it or name the owner",
                    claim.owner
                ),
                // An official kind used in a row, such as NIP-32 `1985`.
                None if row && line.contains("(NIP-") => {}
                None => panic!(
                    "{at} declares kind {kind}, which isn't in the kind registry \
                     (crates/nostr/src/kinds.rs and nips/openagents/README.md)"
                ),
            }
        }
    }
}

fn names(line: &str, owner: &str) -> bool {
    line.contains(owner) || (owner == "contracts" && line.contains("private artifact envelope"))
}

#[test]
fn every_registered_kind_is_written_in_its_owners_specification() {
    let specifications: BTreeMap<String, String> = specifications().into_iter().collect();
    for claim in REGISTRY {
        let text = specifications
            .get(claim.owner)
            .unwrap_or_else(|| panic!("no specification {}", claim.owner));
        assert!(
            backticked(text).contains(&claim.kind),
            "{} doesn't mention its kind {}",
            claim.owner,
            claim.kind
        );
    }
}

#[test]
fn no_claim_takes_an_official_or_block_kind() {
    let official = official_kinds();
    let block_readme = read(nips().join("block/README.md"));
    let block: BTreeSet<u16> = block_readme
        .lines()
        .filter(|line| line.starts_with("| [NIP-"))
        .filter_map(|line| line.split('|').nth(3))
        .flat_map(backticked)
        .collect();
    assert!(block.contains(&30_174), "parsed the Block kinds column");
    for claim in REGISTRY {
        let kind = u32::from(claim.kind);
        assert!(
            !official
                .iter()
                .any(|&(low, high)| (low..=high).contains(&kind)),
            "{} claims kind {kind}, which the official NIP list assigns",
            claim.owner
        );
        assert!(
            !block.contains(&claim.kind),
            "{} claims kind {kind}, which a Block NIP assigns",
            claim.owner
        );
    }
}

#[test]
fn the_duplicate_detector_catches_a_second_claim() {
    // The shape of issue #9900: NIP-XP restating NIP-EVAL's kind as its own.
    let line = "| `3195` | Regular | A tester's content-free playtest report. |";
    let kind = row_kind(line).expect("row");
    let claim = claim_of(kind).expect("registered");
    assert_ne!(claim.owner, "NIP-XP");
    assert!(!names(line, claim.owner));
    assert_eq!(prose_kind("Kind `3195` is a regular record."), Some(3_195));
    assert_eq!(prose_kind("## Zone command — kind `23302`"), Some(23_302));
}
