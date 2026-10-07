//! The quest steps a rumor or a fixed line may point at: the Apprentice's
//! Road (`docs/verse/first-agent-quests.md`), one step a quest.
//!
//! A rumor must name one of these, so it always points at a real step and
//! never at a made-up objective. A fixed line keyed by a step is said at
//! that step, or at every step of an act when it names the act, such as
//! `apprentice-road/2` ([`matches`]).

/// One quest step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// `apprentice-road/ACT/QUEST`.
    pub id: &'static str,
    /// The quest's title.
    pub title: &'static str,
    /// Where it happens, in words a villager would use.
    pub place: &'static str,
}

const fn step(id: &'static str, title: &'static str, place: &'static str) -> Step {
    Step { id, title, place }
}

/// Every step of the Apprentice's Road, in order.
pub const STEPS: &[Step] = &[
    step(
        "apprentice-road/1/clearing",
        "A Clearing in the Woods",
        "the glade, where Bram waits",
    ),
    step(
        "apprentice-road/1/team-at-work",
        "The Team at Work",
        "the workshop hall",
    ),
    step(
        "apprentice-road/1/reading-the-wall",
        "Reading the Wall",
        "the notice board in the workshop",
    ),
    step(
        "apprentice-road/1/name-in-the-ledger",
        "A Name in the Ledger",
        "the workshop",
    ),
    step(
        "apprentice-road/2/light-the-hearth",
        "Light the Hearth",
        "the Server Barn",
    ),
    step(
        "apprentice-road/2/two-keys",
        "Two Keys, One House",
        "the Server Barn",
    ),
    step(
        "apprentice-road/2/tools-of-the-trade",
        "Tools of the Trade",
        "the Server Barn",
    ),
    step(
        "apprentice-road/2/barn-lights-up",
        "The Barn Lights Up",
        "the Server Barn",
    ),
    step(
        "apprentice-road/3/apprentices-name",
        "An Apprentice's Name",
        "the empty desk in the workshop hall",
    ),
    step(
        "apprentice-road/3/small-errand",
        "A Small Errand",
        "the notice board",
    ),
    step(
        "apprentice-road/3/question-at-the-podium",
        "A Question at the Podium",
        "the Podium",
    ),
    step(
        "apprentice-road/3/strongroom",
        "The Strongroom",
        "the Merge station",
    ),
    step(
        "apprentice-road/3/ask-for-better",
        "Ask for Better",
        "the Merge station",
    ),
    step(
        "apprentice-road/4/raise-the-workshop",
        "Raise the Workshop",
        "Makers' Hall",
    ),
    step("apprentice-road/4/many-hands", "Many Hands", "Makers' Hall"),
    step(
        "apprentice-road/4/when-hands-collide",
        "When Hands Collide",
        "Makers' Hall",
    ),
    step("apprentice-road/4/lounge", "The Lounge", "the Lounge"),
    step("apprentice-road/5/from-afar", "From Afar", "anywhere"),
    step(
        "apprentice-road/5/standing-orders",
        "Standing Orders",
        "the workshop",
    ),
    step(
        "apprentice-road/5/old-college",
        "The Old College",
        "the Old College",
    ),
    step("apprentice-road/5/open-doors", "Open Doors", "the workshop"),
];

/// The step `id`.
#[must_use]
pub fn step_of(id: &str) -> Option<&'static Step> {
    STEPS.iter().find(|s| s.id == id)
}

/// Whether a line keyed by `key` is said at step `current`: the same step,
/// or `key` names the act or quest line `current` is in.
#[must_use]
pub fn matches(key: &str, current: &str) -> bool {
    current == key
        || current
            .strip_prefix(key)
            .is_some_and(|rest| rest.starts_with('/'))
}
