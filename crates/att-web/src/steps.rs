//! The round's steps, their states, and the timeline that plays them.
//!
//! The live flow reports each step as the real work finishes, often far
//! faster than a person can follow. The [`Player`] queues every report and
//! applies them in order, holding each step's animation on screen for at
//! least its [`hold`] before the next report is applied, so every step is
//! seen. The milliseconds shown are always the real ones.

use std::collections::VecDeque;

/// One step of a round, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Fetch,
    Chain,
    Measure,
    Bind,
    Encrypt,
    Relay,
    Decrypt,
    Answer,
    Receipt,
}

impl Step {
    pub const ALL: [Self; 9] = [
        Self::Fetch,
        Self::Chain,
        Self::Measure,
        Self::Bind,
        Self::Encrypt,
        Self::Relay,
        Self::Decrypt,
        Self::Answer,
        Self::Receipt,
    ];

    /// Its place in [`Step::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The fixed id the page uses (`data-step`, element ids).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Chain => "chain",
            Self::Measure => "measure",
            Self::Bind => "bind",
            Self::Encrypt => "encrypt",
            Self::Relay => "relay",
            Self::Decrypt => "decrypt",
            Self::Answer => "answer",
            Self::Receipt => "receipt",
        }
    }
}

/// Where a step stands.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Pending,
    Running,
    Ok,
    Refused(String),
    Skipped,
}

impl State {
    /// The word the page's `data-state` and CSS use.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Ok => "ok",
            Self::Refused(_) => "refused",
            Self::Skipped => "skipped",
        }
    }

    #[must_use]
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Ok | Self::Refused(_) | Self::Skipped)
    }
}

/// What the visitor can break on purpose, to watch the check refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tamper {
    #[default]
    None,
    /// The fingerprint the provider reports is changed.
    Measurement,
    /// The key offered is not the one the hardware evidence vouches for.
    UnboundKey,
    /// The GPU's confidential-computing mode reads as off.
    GpuOff,
}

/// Which machine answers: the sealed GPU, the sealed CPU, or an open
/// Pylon (sealed in transit only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lane {
    Gpu,
    #[default]
    Cpu,
    Open,
}

impl Lane {
    /// From the form's radio value.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "gpu" => Self::Gpu,
            "open" => Self::Open,
            _ => Self::Cpu,
        }
    }

    /// The gateway's `?lane=` word.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
            Self::Open => "open",
        }
    }

    /// Whether a tamper choice applies on this lane.
    #[must_use]
    pub fn allows(self, tamper: Tamper) -> bool {
        match tamper {
            Tamper::None | Tamper::Measurement => true,
            Tamper::UnboundKey => self != Self::Open,
            Tamper::GpuOff => self == Self::Gpu,
        }
    }
}

impl Tamper {
    /// From the form's radio value.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "measurement" => Self::Measurement,
            "unbound-key" => Self::UnboundKey,
            "gpu-off" => Self::GpuOff,
            _ => Self::None,
        }
    }
}

/// What the visitor asked for when they pressed Run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOptions {
    pub lane: Lane,
    pub tamper: Tamper,
    pub prompt: String,
}

/// The longest prompt the form takes, in characters.
pub const PROMPT_MAX: usize = 200;

/// Trims the prompt and cuts it to [`PROMPT_MAX`] characters.
#[must_use]
pub fn clean_prompt(text: &str) -> String {
    text.trim().chars().take(PROMPT_MAX).collect()
}

/// The least time, in seconds, a step's animation stays before the next
/// report is applied. Travelling steps get time for the packet to arrive.
#[must_use]
pub fn hold(step: Step, reduced_motion: bool) -> f64 {
    if reduced_motion {
        return 0.35;
    }
    match step {
        Step::Fetch => 0.9,
        Step::Relay | Step::Answer => 1.5,
        Step::Measure => 1.0,
        _ => 0.65,
    }
}

/// One step's animation: when it started, and how it ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anim {
    pub state: State,
    /// Seconds (page clock) the animation started; `None` while pending.
    pub started: Option<f64>,
    /// Seconds it settled (Ok, Refused or Skipped).
    pub settled: Option<f64>,
    /// The real milliseconds the flow reported.
    pub ms: Option<f64>,
}

/// A report from the flow, waiting for its turn on screen.
#[derive(Clone, Debug, PartialEq)]
pub enum Event<P> {
    Step(Step, State, Option<f64>),
    /// Anything else that should wait its turn (a panel, the verdict).
    Other(P),
}

/// Plays reports in order, holding each step long enough to be seen.
#[derive(Clone, Debug)]
pub struct Player<P> {
    queue: VecDeque<Event<P>>,
    blocked_until: f64,
    pub anims: [Anim; 9],
    pub reduced_motion: bool,
}

impl<P> Default for Player<P> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            blocked_until: 0.0,
            anims: Default::default(),
            reduced_motion: false,
        }
    }
}

impl<P> Player<P> {
    /// Forgets everything: every step pending, nothing queued.
    pub fn reset(&mut self) {
        self.queue.clear();
        self.blocked_until = 0.0;
        self.anims = Default::default();
    }

    pub fn push(&mut self, event: Event<P>) {
        self.queue.push_back(event);
    }

    /// True while reports are still waiting or a step is still being held.
    #[must_use]
    pub fn busy(&self, now: f64) -> bool {
        !self.queue.is_empty() || now < self.blocked_until
    }

    #[must_use]
    pub fn anim(&self, step: Step) -> &Anim {
        &self.anims[step.index()]
    }

    /// Applies every report whose turn has come, and returns them in order
    /// for the page to show.
    pub fn pump(&mut self, now: f64) -> Vec<Event<P>> {
        let mut applied = Vec::new();
        while now >= self.blocked_until {
            let Some(event) = self.queue.pop_front() else {
                break;
            };
            match event {
                Event::Step(step, state, ms) => {
                    // A skipped step is shown at once, without its animation.
                    let skipped_next = matches!(
                        self.queue.front(),
                        Some(Event::Step(next, State::Skipped, _)) if *next == step
                    );
                    if state == State::Running && skipped_next {
                        continue;
                    }
                    let anim = &mut self.anims[step.index()];
                    if state == State::Skipped {
                        anim.state = State::Skipped;
                        anim.started = Some(now);
                        anim.settled = Some(now);
                        anim.ms = ms;
                        self.blocked_until = now + 0.12;
                        applied.push(Event::Step(step, state, ms));
                        continue;
                    }
                    if state.is_final() && anim.started.is_none() {
                        // Reported done without a start: play it first.
                        anim.state = State::Running;
                        anim.started = Some(now);
                        self.blocked_until = now + hold(step, self.reduced_motion);
                        applied.push(Event::Step(step, State::Running, None));
                        self.queue.push_front(Event::Step(step, state, ms));
                        continue;
                    }
                    match state {
                        State::Running => {
                            anim.state = State::Running;
                            anim.started = Some(now);
                            anim.settled = None;
                            self.blocked_until = now + hold(step, self.reduced_motion);
                        }
                        State::Pending => *anim = Anim::default(),
                        _ => {
                            anim.state = state.clone();
                            anim.settled = Some(now);
                            anim.ms = ms;
                        }
                    }
                    applied.push(Event::Step(step, state, ms));
                }
                other => applied.push(other),
            }
        }
        applied
    }
}

/// A long value cut in the middle, `abcdef…uvwxyz`, or `None` when it is
/// short enough to show whole.
#[must_use]
pub fn middle_cut(value: &str, max: usize) -> Option<String> {
    let count = value.chars().count();
    if count <= max || max < 5 {
        return None;
    }
    let tail = (max - 1) / 2;
    let head = max - 1 - tail;
    let start: String = value.chars().take(head).collect();
    let end: String = value.chars().skip(count - tail).collect();
    Some(format!("{start}\u{2026}{end}"))
}

/// A panel value as first shown: keys, hashes and other unbroken strings
/// longer than `max` are cut in the middle; sentences only when very long.
#[must_use]
pub fn cut_value(value: &str, max: usize) -> Option<String> {
    if value.contains(char::is_whitespace) {
        middle_cut(value, max * 4)
    } else {
        middle_cut(value, max)
    }
}

/// The real time a step took, as the step list shows it.
#[must_use]
pub fn ms_label(ms: f64) -> String {
    if ms < 10.0 {
        format!("{ms:.1} ms")
    } else if ms < 1000.0 {
        format!("{ms:.0} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_have_their_fixed_ids_in_order() {
        let ids: Vec<_> = Step::ALL.iter().map(|s| s.id()).collect();
        assert_eq!(
            ids,
            [
                "fetch", "chain", "measure", "bind", "encrypt", "relay", "decrypt", "answer",
                "receipt"
            ]
        );
        for (i, step) in Step::ALL.iter().enumerate() {
            assert_eq!(step.index(), i);
        }
    }

    #[test]
    fn a_fast_flow_is_held_so_each_step_is_seen() {
        let mut player: Player<&str> = Player::default();
        player.push(Event::Step(Step::Fetch, State::Running, None));
        player.push(Event::Step(Step::Fetch, State::Ok, Some(3.0)));
        player.push(Event::Other("panel"));
        player.push(Event::Step(Step::Chain, State::Ok, Some(1.0)));
        let first = player.pump(0.0);
        assert_eq!(first.len(), 1);
        assert_eq!(player.anim(Step::Fetch).state, State::Running);
        assert!(player.pump(0.5).is_empty());
        let next = player.pump(0.95);
        // Fetch settles, the panel shows, and Chain starts (it never
        // reported Running) and is held.
        assert_eq!(next.len(), 3);
        assert_eq!(player.anim(Step::Fetch).state, State::Ok);
        assert_eq!(player.anim(Step::Fetch).ms, Some(3.0));
        assert_eq!(player.anim(Step::Chain).state, State::Running);
        assert!(player.busy(1.0));
        let last = player.pump(2.0);
        assert_eq!(last, vec![Event::Step(Step::Chain, State::Ok, Some(1.0))]);
        assert!(!player.busy(2.0));
    }

    #[test]
    fn a_slow_step_settles_when_it_is_reported() {
        let mut player: Player<()> = Player::default();
        player.push(Event::Step(Step::Relay, State::Running, None));
        player.pump(0.0);
        assert!(player.pump(5.0).is_empty());
        player.push(Event::Step(
            Step::Relay,
            State::Refused("no answer".into()),
            Some(5000.0),
        ));
        player.pump(5.0);
        assert_eq!(player.anim(Step::Relay).settled, Some(5.0));
        player.reset();
        assert_eq!(player.anim(Step::Relay), &Anim::default());
    }

    #[test]
    fn skipped_steps_do_not_play() {
        let mut player: Player<()> = Player::default();
        player.push(Event::Step(Step::Relay, State::Running, None));
        player.push(Event::Step(Step::Relay, State::Skipped, None));
        player.push(Event::Step(Step::Answer, State::Skipped, None));
        let first = player.pump(0.0);
        assert_eq!(first, vec![Event::Step(Step::Relay, State::Skipped, None)]);
        assert_eq!(player.pump(0.2).len(), 1);
        assert!(!player.busy(0.5));
    }

    #[test]
    fn values_are_cut_in_the_middle() {
        assert_eq!(middle_cut("short", 10), None);
        let cut = middle_cut("0123456789abcdefghij", 9).unwrap();
        assert_eq!(cut, "0123\u{2026}ghij");
        assert_eq!(cut.chars().count(), 9);
        assert_eq!(
            cut_value("The fingerprint does not match any logged build.", 20),
            None
        );
        assert!(cut_value(&"ab".repeat(40), 20).is_some());
        assert_eq!(ms_label(4.25), "4.2 ms");
        assert_eq!(ms_label(250.4), "250 ms");
        assert_eq!(ms_label(1830.0), "1.83 s");
        assert_eq!(clean_prompt(&format!("  {}", "x".repeat(300))).len(), 200);
        assert_eq!(Tamper::parse("unbound-key"), Tamper::UnboundKey);
        assert_eq!(Tamper::parse("anything"), Tamper::None);
        assert_eq!(Tamper::parse("gpu-off"), Tamper::GpuOff);
        assert_eq!(Lane::parse("open"), Lane::Open);
        assert!(!Lane::Open.allows(Tamper::UnboundKey));
        assert!(!Lane::Cpu.allows(Tamper::GpuOff));
        assert!(Lane::Gpu.allows(Tamper::GpuOff));
    }
}
