//! Everglade's placed characters (NPCs): characters the world places, not
//! bodies a player can choose. Today that is Alice, our original
//! explorer-druid (`docs/verse/female-character.md`), who is the workshop
//! agent's body (`docs/verse/workshop-agent.md`): she stands at her
//! standing desk in the owner's house as a resident studio seat, and walks
//! to the console while a command runs and to the lectern while a proposal
//! waits. She never sits.
//!
//! So she no longer stands by the approach as a still creature: the studio
//! draws her where her work puts her, as the pack's form [`ALICE_FORM`],
//! for any seat whose look names her ([`form_of`]). The pack carries her in
//! each of [`ALICE_OUTFITS`]; the owner picks one with `verse
//! --alice-outfit NAME` or `VERSE_ALICE_OUTFIT` ([`set_alice_outfit`]), and
//! the fitted coat is the default.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::zones::everglade_pack::compile::{ALICE_FORM, ALICE_OUTFIT_FORMS};

/// Alice's outfits: each one's name and the pack form that wears it, the
/// default first.
pub const ALICE_OUTFITS: [(&str, &str); 3] = [
    ("coat", ALICE_FORM),
    ("light", ALICE_OUTFIT_FORMS[0]),
    ("summer", ALICE_OUTFIT_FORMS[1]),
];

/// The outfit she wears, an index into [`ALICE_OUTFITS`].
static ALICE_OUTFIT: AtomicUsize = AtomicUsize::new(0);

/// Dresses Alice in the outfit `name`, before Everglade loads.
///
/// # Errors
///
/// Returns a message naming the outfits when `name` is none of them.
pub fn set_alice_outfit(name: &str) -> Result<(), String> {
    let index = ALICE_OUTFITS
        .iter()
        .position(|(outfit, _)| outfit.eq_ignore_ascii_case(name.trim()))
        .ok_or_else(|| {
            let names: Vec<&str> = ALICE_OUTFITS.iter().map(|(n, _)| *n).collect();
            format!("Alice has no outfit `{name}`; use {}", names.join(", "))
        })?;
    ALICE_OUTFIT.store(index, Ordering::Relaxed);
    Ok(())
}

/// The pack form of the outfit she wears.
#[must_use]
pub fn alice_form() -> &'static str {
    ALICE_OUTFITS[ALICE_OUTFIT
        .load(Ordering::Relaxed)
        .min(ALICE_OUTFITS.len() - 1)]
    .1
}

/// Where Alice stood before she took her desk: west of the approach, a few
/// strides north of the spawn. Nothing stands there now.
pub const ALICE_AT: [f32; 2] = [-4.6, -21.6];
/// The look a studio seat names to be drawn as Alice.
pub const ALICE_LOOK: &str = "alice";

/// The pack form a seat's `look` draws it as, when it names a placed
/// character rather than an outfit color.
#[must_use]
pub fn form_of(look: &str) -> Option<&'static str> {
    look.eq_ignore_ascii_case(ALICE_LOOK).then(alice_form)
}

/// The placed characters that stand still in the town, as creatures:
/// none, now that Alice works at her desk.
#[must_use]
pub fn creatures() -> Vec<super::wildlife::Creature> {
    Vec::new()
}

/// Each placed character's block in the solids: none, for the same
/// reason; a studio seat walks and blocks nothing.
#[must_use]
pub fn blocks() -> Vec<(crate::controller::Footprint, f32)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_outfit_has_its_own_form_and_the_coat_is_the_default() {
        assert_eq!(ALICE_OUTFITS[0], ("coat", ALICE_FORM));
        let forms: std::collections::BTreeSet<_> = ALICE_OUTFITS.iter().map(|(_, f)| *f).collect();
        assert_eq!(forms.len(), ALICE_OUTFITS.len());
        assert!(forms.iter().all(|f| f.starts_with("npc/alice")));
    }

    #[test]
    fn alice_is_the_workshop_agents_body_not_a_creature_by_the_approach() {
        assert!(creatures().is_empty());
        assert!(blocks().is_empty());
        assert_eq!(form_of("alice"), Some("npc/alice"));
        assert_eq!(form_of("Alice"), Some("npc/alice"));
        assert_eq!(form_of("teal"), None);
        assert!(set_alice_outfit("cape").is_err());
        assert_eq!(
            super::super::studio::WORKSHOP_AGENT,
            ALICE_LOOK,
            "the workshop agent is Alice"
        );
    }
}
