//! What the graph shows at a moment: the data and sats moving along links,
//! which links are lit, each part's line, and the wallet's balance. It
//! takes the script's cues as the chat plays them. Pure, so it is tested
//! without a browser.

use crate::scene::{self, End, LINKS, Node};
use crate::script::{BALANCE, Cue};

/// How long one piece of data takes to cross a link.
pub const TRAVEL_MS: u64 = 700;
/// How long a link stays lit after the data has crossed it.
const AFTERGLOW_MS: u64 = 260;
/// How long the wallet's balance takes to count down.
const COUNT_MS: u64 = 900;

/// One piece of data on its way.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse {
    pub link: usize,
    /// Moving from the link's second end to its first.
    pub back: bool,
    pub start: u64,
    /// Sats, when it carries a payment.
    pub sats: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub pulses: Vec<Pulse>,
    /// By [`Node::ALL`]: the line and whether it is working.
    pub lines: Vec<(&'static str, bool)>,
    /// When each part last changed, for its glow.
    pub touched: Vec<Option<u64>>,
    balance_from: u64,
    balance_to: u64,
    balance_at: u64,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    pub fn new() -> Self {
        Self {
            pulses: Vec::new(),
            lines: Node::ALL.iter().map(|n| (n.idle(), false)).collect(),
            touched: vec![None; Node::ALL.len()],
            balance_from: BALANCE,
            balance_to: BALANCE,
            balance_at: 0,
        }
    }

    /// The graph after the whole reply, still.
    pub fn settled() -> Self {
        let mut graph = Self::new();
        let (lines, balance) = crate::script::settled();
        for (node, line) in lines {
            graph.lines[node.index()] = (line, false);
        }
        graph.balance_from = balance;
        graph.balance_to = balance;
        graph
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn cue(&mut self, cue: &Cue, now: u64) {
        match *cue {
            Cue::Data { from, to } => {
                if let Some((link, back)) = scene::link(from, to) {
                    self.pulses.push(Pulse {
                        link,
                        back,
                        start: now,
                        sats: None,
                    });
                }
            }
            Cue::Sats { to, amount } => {
                if let Some((link, back)) = scene::link(End::Node(Node::Wallet), End::Node(to)) {
                    self.pulses.push(Pulse {
                        link,
                        back,
                        start: now,
                        sats: Some(amount),
                    });
                }
                self.balance_from = self.balance(now);
                self.balance_to = self.balance_to.saturating_sub(amount);
                self.balance_at = now;
            }
            Cue::Line { node, line, busy } => {
                self.lines[node.index()] = (line, busy);
                self.touched[node.index()] = Some(now);
            }
        }
    }

    /// The balance as it counts down after a payment.
    pub fn balance(&self, now: u64) -> u64 {
        let p = (now.saturating_sub(self.balance_at) as f32 / COUNT_MS as f32).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - p).powi(3);
        let span = self.balance_from as f32 - self.balance_to as f32;
        (self.balance_from as f32 - span * eased).round() as u64
    }

    /// Drops what has finished. Returns whether anything still moves.
    pub fn sweep(&mut self, now: u64) -> bool {
        self.pulses
            .retain(|p| now < p.start + TRAVEL_MS + AFTERGLOW_MS);
        !self.pulses.is_empty() || now < self.balance_at + COUNT_MS
    }

    /// How lit each link is, from 0 to 1: fully while data crosses it,
    /// fading just after.
    pub fn lit(&self, now: u64) -> Vec<f32> {
        let mut lit = vec![0.0_f32; LINKS.len()];
        for p in &self.pulses {
            let age = now.saturating_sub(p.start);
            let level = if now < p.start {
                0.0
            } else if age <= TRAVEL_MS {
                1.0
            } else {
                1.0 - (age - TRAVEL_MS) as f32 / AFTERGLOW_MS as f32
            };
            lit[p.link] = lit[p.link].max(level.clamp(0.0, 1.0));
        }
        lit
    }

    /// Each moving piece: its link, how far along from the link's first end,
    /// and its sats.
    pub fn moving(&self, now: u64) -> Vec<(usize, f32, Option<u64>)> {
        self.pulses
            .iter()
            .filter(|p| now >= p.start && now < p.start + TRAVEL_MS)
            .map(|p| {
                let t = (now - p.start) as f32 / TRAVEL_MS as f32;
                let t = t * t * (3.0 - 2.0 * t);
                (p.link, if p.back { 1.0 - t } else { t }, p.sats)
            })
            .collect()
    }

    /// Whether a part is lit: working, or data on one of its links.
    pub fn active(&self, node: Node, now: u64) -> bool {
        if self.lines[node.index()].1 {
            return true;
        }
        let lit = self.lit(now);
        LINKS
            .iter()
            .enumerate()
            .any(|(i, &(a, b))| lit[i] > 0.0 && (a == End::Node(node) || b == End::Node(node)))
    }
}

/// `50,000`.
pub fn sats(amount: u64) -> String {
    let digits = amount.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_light_only_while_data_moves() {
        let mut g = Graph::new();
        assert!(g.lit(0).iter().all(|l| *l == 0.0));
        g.cue(
            &Cue::Data {
                from: End::Chat,
                to: End::Node(Node::GitHub),
            },
            1_000,
        );
        assert_eq!(g.lit(1_100)[0], 1.0);
        assert_eq!(g.moving(1_100).len(), 1);
        assert!(g.active(Node::GitHub, 1_100));
        assert!(!g.active(Node::Gmail, 1_100));
        assert!(g.sweep(1_100));
        assert!(!g.sweep(1_000 + TRAVEL_MS + AFTERGLOW_MS));
        assert!(g.lit(5_000).iter().all(|l| *l == 0.0));
    }

    #[test]
    fn data_coming_back_runs_the_link_backwards() {
        let mut g = Graph::new();
        g.cue(
            &Cue::Data {
                from: End::Node(Node::GitHub),
                to: End::Chat,
            },
            0,
        );
        let (_, t, _) = g.moving(10)[0];
        assert!(t > 0.95);
    }

    #[test]
    fn the_balance_counts_down_once() {
        let mut g = Graph::new();
        g.cue(
            &Cue::Sats {
                to: Node::Pylon,
                amount: 21,
            },
            100,
        );
        assert_eq!(g.balance(100), BALANCE);
        assert_eq!(g.balance(100 + COUNT_MS), BALANCE - 21);
        assert_eq!(g.moving(200)[0].2, Some(21));
        assert_eq!(Graph::settled().balance(0), BALANCE - 21);
        assert_eq!(sats(49_979), "49,979");
        assert_eq!(sats(21), "21");
    }
}
