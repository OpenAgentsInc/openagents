//! A terminal turn that works a GitHub issue, on Microluna.
//!
//! The flow is `coder_delegate::issue`, re-exported here; this module runs
//! it with Coder One's terminal turn ([`crate::terminal::answer`]), whose
//! issue-flow turns run the Microluna loop, and with an evaluation run's
//! seal. Coder's terminal runs the same flow on Microcoder.

use std::rc::Rc;

pub use coder_delegate::issue::*;

use crate::record::Recorder;
use crate::terminal::{Answer, Micro, Progress, Request};

pub mod confined;

/// A checkout set up to work an issue, with a Microluna request.
pub type Prepared = coder_delegate::issue::Prepared<Micro>;

/// Coder One's terminal turn, as the issue flow's worker.
struct Terminal;

impl Worker<Micro> for Terminal {
    async fn answer(&self, request: &Request, on: Rc<dyn Fn(Progress)>) -> Answer {
        Box::pin(crate::terminal::answer(request, on)).await
    }

    fn seal(&self, request: &Request) -> Option<coder_delegate::seal::Seal> {
        request.extra.seal.clone()
    }
}

/// Runs the issue flow for `reference` on Coder One's terminal turn and
/// returns the turn's answer.
pub async fn run(
    request: &Request,
    reference: Reference,
    on: Rc<dyn Fn(Progress)>,
    recorder: &Recorder,
) -> Answer {
    coder_delegate::issue::run(&Terminal, request, reference, on, recorder).await
}

/// Works a prepared issue on Coder One's terminal turn; see
/// [`coder_delegate::issue::work`].
pub async fn work(
    prepared: Prepared,
    reference: Reference,
    on: Rc<dyn Fn(Progress)>,
    recorder: &Recorder,
    publish: bool,
) -> (Answer, Option<Confinement>) {
    coder_delegate::issue::work(&Terminal, prepared, reference, on, recorder, publish).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn parts_cut_a_requirement_into_its_clauses() {
        let got = crate::micro::parts(
            "- R1 (deliverable): A short explanation, in the docs and in the Gym's view, of what they are, how long they take, what they cost, and how they relate to TB4: a fast screen.",
        );
        assert!(got.contains(&"how long they take".to_string()), "{got:?}");
        assert!(got.contains(&"what they cost".to_string()), "{got:?}");
        assert!(got.contains(&"in the Gym's view".to_string()), "{got:?}");
    }
}
