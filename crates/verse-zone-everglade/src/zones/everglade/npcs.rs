//! Everglade's placed characters (NPCs): characters the world places, not
//! bodies a player can choose. Today that is Alice, our original
//! explorer-druid (`docs/verse/female-character.md`), who is the workshop
//! agent's body (`docs/verse/workshop-agent.md`): she sits at the last desk
//! in the workshop hall as a resident studio seat, and walks to the
//! Workbench while a command runs and to the Podium while a proposal waits.
//!
//! So she no longer stands by the approach as a still creature: the studio
//! draws her where her work puts her, as the pack's form [`ALICE_FORM`],
//! for any seat whose look names her ([`form_of`]).

use crate::zones::everglade_pack::compile::ALICE_FORM;

/// Where Alice stood before she took her desk: west of the approach, a few
/// strides north of the spawn. Nothing stands there now.
pub const ALICE_AT: [f32; 2] = [-4.6, -21.6];
/// The look a studio seat names to be drawn as Alice.
pub const ALICE_LOOK: &str = "alice";

/// The pack form a seat's `look` draws it as, when it names a placed
/// character rather than an outfit color.
#[must_use]
pub fn form_of(look: &str) -> Option<&'static str> {
    look.eq_ignore_ascii_case(ALICE_LOOK).then_some(ALICE_FORM)
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
    fn alice_is_the_workshop_agents_body_not_a_creature_by_the_approach() {
        assert!(creatures().is_empty());
        assert!(blocks().is_empty());
        assert_eq!(form_of("alice"), Some("npc/alice"));
        assert_eq!(form_of("Alice"), Some("npc/alice"));
        assert_eq!(form_of("teal"), None);
        assert_eq!(
            super::super::studio::WORKSHOP_AGENT,
            ALICE_LOOK,
            "the workshop agent is Alice"
        );
    }
}
