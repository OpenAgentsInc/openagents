//! Replay the escape package without subtracting its external anchor fees.

use super::model::{Event, Leaf};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Step {
    pub level: u32,
    pub confirmed: bool,
    pub fee: u64,
}
#[derive(Clone, Debug)]
pub struct Paid {
    pub key: String,
    pub amount: u64,
}
#[derive(Clone, Debug, Default)]
pub struct Escape {
    pub down: bool,
    pub obsolete: bool,
    pub old_output: Option<String>,
    pub mature_at: Option<u32>,
    pub height: u32,
    pub steps: BTreeMap<String, Step>,
    pub paid: Vec<Paid>,
    pub total: u64,
    pub expected_total: u64,
    pub anchor_total: u64,
    coin_height: u32,
    remaining: BTreeMap<String, Vec<u64>>,
}
impl Escape {
    pub fn apply(
        &mut self,
        event: &Event,
        leaves: &BTreeMap<u64, Leaf>,
        total: u64,
        delay: u32,
    ) -> Result<(), String> {
        self.height = self.height.max(event.height.unwrap_or(0));
        match event.kind.as_str() {
            "tree_open" => self.coin_height = event.height.unwrap_or(0),
            "transition" | "simulation" => {
                self.old_output = event
                    .data
                    .get("prev")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                if self.down {
                    self.obsolete = true;
                    self.down = false;
                }
                self.coin_height = event.height.unwrap_or(self.height);
            }
            "enclave_down" => {
                if self.down {
                    return Err("The enclave is already down.".into());
                }
                self.down = true;
                self.obsolete = false;
                self.mature_at = Some(
                    self.coin_height
                        .checked_add(delay)
                        .ok_or("The unroll deadline is too large.")?,
                );
                self.steps.clear();
                self.paid.clear();
                self.total = 0;
                self.anchor_total = 0;
                self.remaining.clear();
                self.expected_total = total;
                for leaf in leaves.values() {
                    self.remaining
                        .entry(leaf.exit_key.clone())
                        .or_default()
                        .push(leaf.amount);
                }
            }
            "unroll_broadcast" => {
                if !self.down
                    || event.height.unwrap_or(0).saturating_add(1)
                        < self.mature_at.unwrap_or(u32::MAX)
                {
                    return Err("The unroll was broadcast before it could be mined.".into());
                }
                let level = event.field("level")?;
                if level > 2 {
                    return Err("The viewer supports up to three unroll levels.".into());
                }
                let txid = event.field("txid")?;
                let fee = event.field("cpfp_fee")?;
                self.anchor_total = self
                    .anchor_total
                    .checked_add(fee)
                    .ok_or("The anchor fee total is too large.")?;
                if self
                    .steps
                    .insert(
                        txid,
                        Step {
                            level,
                            confirmed: false,
                            fee,
                        },
                    )
                    .is_some()
                {
                    return Err("The unroll broadcasts the same transaction twice.".into());
                }
            }
            "unroll_confirmed" => {
                if !self.down || event.height.unwrap_or(0) < self.mature_at.unwrap_or(u32::MAX) {
                    return Err("The unroll confirmed before its deadline.".into());
                }
                let txid: String = event.field("txid")?;
                let step = self
                    .steps
                    .get_mut(&txid)
                    .ok_or("An unroll transaction confirmed before its broadcast event.")?;
                if step.level != event.field::<u32>("level")? {
                    return Err("The unroll confirmation has the wrong level.".into());
                }
                step.confirmed = true;
            }
            "payout" if self.down => {
                if !self.steps.values().any(|s| s.confirmed) {
                    return Err("A leaf was paid before any unroll transaction confirmed.".into());
                }
                let key: String = event.field("exit_key")?;
                let amount: u64 = event.field("amount")?;
                let amounts = self
                    .remaining
                    .get_mut(&key)
                    .ok_or("The payout uses an unknown exit key.")?;
                let at = amounts
                    .iter()
                    .position(|n| *n == amount)
                    .ok_or("The payout does not equal an unpaid leaf's balance.")?;
                amounts.swap_remove(at);
                self.total = self
                    .total
                    .checked_add(amount)
                    .ok_or("The payout total is too large.")?;
                self.paid.push(Paid { key, amount });
            }
            _ => {}
        }
        Ok(())
    }
    pub fn remaining_blocks(&self) -> u32 {
        self.mature_at
            .unwrap_or(self.height)
            .saturating_sub(self.height)
    }
}
#[cfg(test)]
mod tests {
    use crate::layertree::model::Run;
    use std::path::Path;
    #[test]
    fn deadline_uses_the_output_height_and_progress_spends_the_old_package() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../layertree-tee-lab/fixtures/events/regtest.jsonl");
        let run = Run::load(&path).unwrap();
        let down = run.snapshot(18).unwrap();
        assert_eq!(down.escape.mature_at, Some(404));
        assert_eq!(down.escape.remaining_blocks(), 144);
        let mut early = run.events[19].clone();
        early.height = Some(402);
        assert!(down.clone().apply(&early).is_err());
        let mut early = run.events[20].clone();
        early.height = Some(403);
        assert!(run.snapshot(19).unwrap().apply(&early).is_err());
        let mut progress = run.events[15].clone();
        progress.seq = 19;
        progress.height = Some(261);
        progress
            .data
            .insert("prev".into(), down.outpoint.clone().into());
        let mut state = down;
        state.apply(&progress).unwrap();
        assert!(state.escape.obsolete);
        assert!(!state.escape.down);
        assert_eq!(
            state.escape.old_output.as_deref(),
            progress.data.get("prev").and_then(|v| v.as_str())
        );
    }
    #[test]
    fn unroll_pays_each_exact_leaf_and_refuses_a_changed_or_repeated_payout() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../layertree-tee-lab/fixtures/events");
        for name in ["regtest.jsonl", "regtest-two-level.jsonl", "unroll.jsonl"] {
            let run = Run::load(&root.join(name)).unwrap();
            let end = run.snapshot(run.events.len() as u64 - 1).unwrap();
            assert_eq!(end.escape.total, end.escape.expected_total, "{name}");
            assert_eq!(end.escape.paid.len(), end.leaves.len(), "{name}");
            let first = run
                .events
                .iter()
                .find(|e| {
                    e.kind == "payout"
                        && e.seq
                            > run
                                .events
                                .iter()
                                .find(|e| e.kind == "enclave_down")
                                .unwrap()
                                .seq
                })
                .unwrap();
            let mut state = run.snapshot(first.seq - 1).unwrap();
            let mut wrong = first.clone();
            wrong.data.insert(
                "amount".into(),
                (first.field::<u64>("amount").unwrap() + 1).into(),
            );
            assert!(state.apply(&wrong).is_err());
            let mut state = run.snapshot(first.seq).unwrap();
            assert!(state.apply(first).is_err());
        }
    }
}
