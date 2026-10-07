//! `verse.private`: the owner's private Verse placements for a paired phone
//! (`docs/verse/private-assets.md`).
//!
//! The phone holds `observe` and names its Verse world key. The host notes
//! the key in Verse's home (`private-phones.json`), says once in its log how
//! the owner grants it, and answers with the owner's `private-assets.json`.
//! The answer grants nothing: the broker signs a pack's URL only for a key
//! the asset's manifest lists, which `verse-private grant` adds and
//! `verse-private revoke` removes.

use std::path::Path;

use coder_access::Code;
use verse_private::{phones, placements};

/// Answers `device`'s ask with `world_key` at `now` from Verse's `home`:
/// the placements file, or `None` when the owner has none.
///
/// # Errors
///
/// Returns [`Code::Unavailable`] when the placements file exists but can't
/// be read or fails its checks.
pub fn answer(
    home: &Path,
    device: &str,
    world_key: &str,
    now: u64,
) -> Result<Option<String>, Code> {
    match phones::note(home, device, world_key, now) {
        Ok(true) => eprintln!(
            "coder host: a paired phone asked for your private Verse assets; to let it \
             load one, run `verse-private grant NAME {world_key}` (`verse-private phones` \
             lists every phone that asked)"
        ),
        Ok(false) => {}
        // Noting is a convenience for the owner; the answer doesn't need it.
        Err(error) => eprintln!("coder host: a phone's Verse key was not noted: {error}"),
    }
    let Some(file) = placements::load(home).map_err(|_| Code::Unavailable)? else {
        return Ok(None);
    };
    let bytes = file.to_bytes().map_err(|_| Code::Unavailable)?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| Code::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use verse_private::placements::{Placement, Placements};

    #[test]
    fn a_phone_gets_the_owners_placements_and_is_noted() {
        let home = tempfile::tempdir().unwrap();
        let (device, world) = ("aa".repeat(32), "bb".repeat(32));
        // No placements: nothing to offer, but the phone is still noted.
        assert_eq!(answer(home.path(), &device, &world, 1_000), Ok(None));
        assert_eq!(phones::load(home.path()).phones[0].world_key, world);

        let mut file = Placements::new("https://broker.example", "default");
        file.place(Placement {
            asset: "sample-guest".into(),
            sha256: "cd".repeat(32),
            bytes: 1234,
            zone: "everglade".into(),
            at: [1.0, 2.0],
            yaw: 0.0,
            scale: 1.0,
            seat: None,
        });
        placements::save(home.path(), &file).unwrap();
        let answered = answer(home.path(), &device, &world, 1_100)
            .unwrap()
            .unwrap();
        assert_eq!(Placements::parse(answered.as_bytes()).unwrap(), file);

        // A broken file is a refusal, not an empty answer, so a phone keeps
        // what it has.
        std::fs::write(placements::path(home.path()), b"{").unwrap();
        assert_eq!(
            answer(home.path(), &device, &world, 1_200),
            Err(Code::Unavailable)
        );
    }
}
