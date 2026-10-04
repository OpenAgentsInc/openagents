//! Portable lifetime identities and bounded simulation time.
use serde::{Deserialize, Serialize};

/// Persistent actor identity within one world instance, fenced by life generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct LifeId {
    pub instance: u64,
    pub actor: u64,
    pub generation: u64,
}
impl LifeId {
    /// Advances a respawn without allowing generation wraparound.
    pub fn next(self) -> Result<Self, String> {
        Ok(Self {
            generation: self
                .generation
                .checked_add(1)
                .ok_or("Life generation exhausted")?,
            ..self
        })
    }
}

/// Generation-checked storage identity, separate from persistent actor identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityHandle {
    pub slot: usize,
    pub generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Slot<T> {
    generation: u64,
    value: Option<T>,
}
/// Stable-order storage. Exhausted generations retire their slots permanently.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entities<T> {
    slots: Vec<Slot<T>>,
}
impl<T> Default for Entities<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}
impl<T> Entities<T> {
    pub fn insert(&mut self, value: T) -> EntityHandle {
        let slot = self
            .slots
            .iter()
            .position(|s| s.value.is_none() && s.generation < u64::MAX)
            .unwrap_or(self.slots.len());
        if slot == self.slots.len() {
            self.slots.push(Slot {
                generation: 0,
                value: None,
            });
        }
        let entry = &mut self.slots[slot];
        entry.value = Some(value);
        EntityHandle {
            slot,
            generation: entry.generation,
        }
    }
    pub fn get(&self, handle: EntityHandle) -> Option<&T> {
        let slot = self.slots.get(handle.slot)?;
        (slot.generation == handle.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }
    pub fn remove(&mut self, handle: EntityHandle) -> Option<T> {
        let slot = self.slots.get_mut(handle.slot)?;
        if slot.generation != handle.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.saturating_add(1);
        Some(value)
    }
}

/// A fixed schedule with explicit dropped time and a bounded catch-up budget.
#[derive(Clone, Debug, Serialize)]
pub struct FixedSchedule {
    step: f64,
    max_steps: u32,
    remainder: f64,
    pub tick: u64,
    pub dropped_seconds: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepBatch {
    pub steps: u32,
    pub seconds: f32,
    pub interpolation: f32,
    pub dropped_seconds: f64,
}
impl FixedSchedule {
    pub fn new(hz: u32, max_steps: u32) -> Result<Self, String> {
        if hz == 0 || hz > 1000 || max_steps == 0 || max_steps > 1000 {
            return Err("Invalid simulation schedule".into());
        }
        Ok(Self {
            step: 1.0 / f64::from(hz),
            max_steps,
            remainder: 0.,
            tick: 0,
            dropped_seconds: 0.,
        })
    }
    pub fn advance(&mut self, elapsed: f64) -> Result<StepBatch, String> {
        if !elapsed.is_finite() || elapsed < 0. {
            return Err("Invalid elapsed simulation time".into());
        }
        let admitted = elapsed.min(self.step * f64::from(self.max_steps));
        let dropped = elapsed - admitted;
        let remainder = self.remainder + admitted;
        let steps = ((remainder / self.step + 1e-9).floor() as u32).min(self.max_steps);
        let tick = self
            .tick
            .checked_add(u64::from(steps))
            .ok_or("Simulation tick exhausted")?;
        let total_dropped = self.dropped_seconds + dropped;
        if !total_dropped.is_finite() {
            return Err("Dropped simulation time exhausted".into());
        }
        self.remainder = (remainder - f64::from(steps) * self.step).max(0.);
        self.tick = tick;
        self.dropped_seconds = total_dropped;
        Ok(StepBatch {
            steps,
            seconds: self.step as f32,
            interpolation: (self.remainder / self.step) as f32,
            dropped_seconds: dropped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reused_slots_and_respawns_refuse_old_identities() {
        let mut entities = Entities::default();
        let old = entities.insert("first");
        assert_eq!(entities.remove(old), Some("first"));
        let new = entities.insert("second");
        assert_eq!(old.slot, new.slot);
        assert_ne!(old.generation, new.generation);
        assert_eq!(entities.get(old), None);
        assert_eq!(entities.remove(old), None);
        assert_eq!(entities.get(new), Some(&"second"));
        let life = LifeId {
            instance: 7,
            actor: 2,
            generation: 0,
        };
        assert_ne!(life, life.next().unwrap());
        assert!(
            LifeId {
                generation: u64::MAX,
                ..life
            }
            .next()
            .is_err()
        );
        let saved = serde_json::to_vec(&entities).unwrap();
        let restored: Entities<String> = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.get(old), None);
        assert_eq!(restored.get(new).map(String::as_str), Some("second"));
    }
    #[test]
    fn render_rates_produce_the_same_ticks() {
        for hz in [30, 60, 144] {
            let mut schedule = FixedSchedule::new(30, 3).unwrap();
            for _ in 0..hz * 10 {
                schedule.advance(1. / f64::from(hz)).unwrap();
            }
            assert_eq!(schedule.tick, 300);
            assert_eq!(schedule.dropped_seconds, 0.);
        }
    }
    #[test]
    fn pauses_are_bounded_and_invalid_time_does_not_mutate_clock() {
        let mut schedule = FixedSchedule::new(30, 3).unwrap();
        let batch = schedule.advance(10.).unwrap();
        assert_eq!(batch.steps, 3);
        assert!((batch.dropped_seconds - 9.9).abs() < 1e-9);
        assert!(schedule.advance(f64::NAN).is_err());
        assert!(schedule.advance(-1.).is_err());
        assert_eq!(schedule.tick, 3);
    }
}
