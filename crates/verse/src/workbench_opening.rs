//! Everglade entry points into the shared admitted studio context.
use crate::terminal::opening::Opening;
use crate::zones::everglade::studio::PanelKind;
use coder_access::{Right, review::TaskReview, studio::Snapshot};
use terminal_studio::opening::{self, Target};

pub fn context(
    snapshot: &Snapshot,
    rights: &[Right],
    target: &PanelKind,
    review: Option<&TaskReview>,
) -> Result<Opening, String> {
    let target = match target {
        PanelKind::Seat(name) => Target::Seat(name.clone()),
        PanelKind::Desk(desk) => Target::Desk(*desk),
        PanelKind::Task(id) => Target::Task(id.clone()),
        PanelKind::Review => Target::Review,
        _ => Target::Workshop,
    };
    opening::context(snapshot, rights, &target, review)
}
