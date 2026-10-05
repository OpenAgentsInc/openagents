//! Bounded client movement intervals, separate from the shared world clock.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use verse_engine::core::LifeId;

pub const MAX_STEPS: u32 = 12;
/// Producer batches leave room for two complete intervals per authority tick.
pub const SEND_STEPS: u32 = 6;
pub const MAX_QUEUED: usize = 16;
pub const MAX_LAG: u64 = 32;
pub const BOOTSTRAP_LAG: u64 = 48;
pub const ACK_TICKS: u64 = 12;
/// The authority boundary that retires an expired character clock.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpiryOrigin {
    Admission,
    PrimaryTick,
    AdditionalTick,
}
/// Numeric clock evidence contains no input, position, or principal identity.
#[derive(Clone, Debug, Serialize)]
pub struct ExpirySample {
    pub actor: u64,
    pub epoch: u64,
    pub authority_tick: u64,
    pub origin: ExpiryOrigin,
    pub world_step: u64,
    pub confirmed_step: u64,
    pub projected_step: u64,
    pub received_at: u64,
    pub applied_sequence: u64,
    pub queued_frames: usize,
    pub queued_steps: u64,
    pub first_start: Option<u64>,
    pub last_end: Option<u64>,
}
/// Runtime-only totals and the first bounded expiry samples, separate from saves.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ExpiryObservations {
    pub total: u64,
    pub admission: u64,
    pub primary_tick: u64,
    pub additional_tick: u64,
    pub bootstrap: u64,
    pub samples: Vec<ExpirySample>,
    pub omitted: u64,
}
impl ExpiryObservations {
    pub(crate) fn record(&mut self, sample: ExpirySample) {
        self.total = self.total.saturating_add(1);
        let count = match sample.origin {
            ExpiryOrigin::Admission => &mut self.admission,
            ExpiryOrigin::PrimaryTick => &mut self.primary_tick,
            ExpiryOrigin::AdditionalTick => &mut self.additional_tick,
        };
        *count = count.saturating_add(1);
        if sample.applied_sequence == 0 {
            self.bootstrap = self.bootstrap.saturating_add(1);
        }
        if self.samples.len() < 32 {
            self.samples.push(sample);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub offset: u32,
    pub axes: [f32; 2],
    pub yaw: f32,
    pub until: u64,
    pub jump: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub life: LifeId,
    pub epoch: u64,
    pub sequence: u64,
    pub tick: u64,
    pub start: u64,
    pub steps: u32,
    pub segments: Vec<Segment>,
}
impl Frame {
    pub fn end(&self) -> Result<u64, String> {
        self.start
            .checked_add(u64::from(self.steps))
            .ok_or_else(|| "Movement interval clock exhausted".into())
    }
    fn split(&self, steps: u32) -> Result<(Self, Option<Self>), String> {
        self.validate()?;
        if steps == 0 || steps > self.steps {
            return Err("Invalid movement interval split".into());
        }
        let mut prefix = self.clone();
        prefix.steps = steps;
        prefix.segments.retain(|segment| segment.offset < steps);
        if steps == self.steps {
            return Ok((prefix, None));
        }
        let mut suffix = self.clone();
        suffix.start = prefix.end()?;
        suffix.steps -= steps;
        let mut carried = self
            .segments
            .iter()
            .rfind(|segment| segment.offset <= steps)
            .unwrap()
            .clone();
        carried.jump &= carried.offset == steps;
        carried.offset = 0;
        suffix.segments = vec![carried];
        suffix.segments.extend(
            self.segments
                .iter()
                .filter(|segment| segment.offset > steps)
                .map(|segment| {
                    let mut segment = segment.clone();
                    segment.offset -= steps;
                    segment
                }),
        );
        prefix.validate()?;
        suffix.validate()?;
        Ok((prefix, Some(suffix)))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.sequence == 0 {
            return Err("Movement interval requires a bound sequence".into());
        }
        self.validate_payload()
    }
    pub fn validate_payload(&self) -> Result<(), String> {
        if self.life.actor == 0
            || self.life.instance == 0
            || self.steps == 0
            || self.steps > MAX_STEPS
            || self.segments.is_empty()
            || self.segments.len() > self.steps as usize
            || self.segments[0].offset != 0
        {
            return Err("Invalid movement interval bounds".into());
        }
        self.end()?;
        let mut last = None;
        for segment in &self.segments {
            if segment.offset >= self.steps
                || last.is_some_and(|offset| segment.offset <= offset)
                || segment.axes.iter().any(|v| !v.is_finite() || v.abs() > 1.)
                || !segment.yaw.is_finite()
                || segment
                    .until
                    .saturating_sub(self.start + u64::from(segment.offset))
                    > super::HELD_STEPS
            {
                return Err("Invalid movement interval segment".into());
            }
            last = Some(segment.offset);
        }
        Ok(())
    }
}
/// A confirmed character clock cannot spend more time than the authority has simulated.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Clock {
    pub step: u64,
    pub applied_sequence: u64,
    pub received_at: u64,
    queue: VecDeque<Frame>,
}
impl Clock {
    pub fn new(step: u64, sequence: u64) -> Self {
        Self {
            step,
            applied_sequence: sequence,
            received_at: step,
            queue: VecDeque::new(),
        }
    }
    pub fn admit(&mut self, frame: Frame, world_step: u64) -> Result<(), String> {
        frame.validate()?;
        let start = match self.queue.back() {
            Some(last) => last.end()?,
            None => self.step,
        };
        let sequence = self
            .queue
            .back()
            .map_or(self.applied_sequence, |last| last.sequence);
        if self.queue.len() >= MAX_QUEUED {
            return Err("Movement interval queue is full".into());
        }
        if frame.start != start {
            return Err(format!(
                "Movement interval start {} is not contiguous with {start}",
                frame.start
            ));
        }
        if frame.sequence <= sequence {
            return Err(format!(
                "Movement interval sequence {} does not follow {sequence}",
                frame.sequence
            ));
        }
        let limit = world_step
            .checked_add(u64::from(MAX_STEPS))
            .ok_or("World movement budget exhausted")?;
        let end = frame.end()?;
        if end > limit {
            return Err(format!(
                "Movement interval end {end} exceeds authority credit {limit}"
            ));
        }
        if self.expired(world_step) {
            return Err(format!(
                "Movement interval clock expired at world step {world_step}, confirmed step {}",
                self.step
            ));
        }
        self.received_at = world_step;
        self.queue.push_back(frame);
        Ok(())
    }
    fn projected_step(&self, world_step: u64) -> Result<u64, String> {
        let mut projected = self.step;
        let mut budget = MAX_STEPS;
        for frame in &self.queue {
            frame.end()?;
            let steps = u64::from(frame.steps.min(budget))
                .min(world_step.saturating_sub(frame.start)) as u32;
            projected = frame.start + u64::from(steps);
            budget -= steps;
            if steps < frame.steps || budget == 0 {
                break;
            }
        }
        Ok(projected)
    }
    pub fn expired(&self, world_step: u64) -> bool {
        let Ok(projected) = self.projected_step(world_step) else {
            return true;
        };
        world_step.saturating_sub(projected)
            > if self.applied_sequence == 0 {
                BOOTSTRAP_LAG
            } else {
                MAX_LAG
            }
    }
    pub(crate) fn expiry_sample(
        &self,
        actor: u64,
        epoch: u64,
        authority_tick: u64,
        world_step: u64,
        origin: ExpiryOrigin,
    ) -> ExpirySample {
        let projected_step = self.projected_step(world_step).unwrap_or(self.step);
        ExpirySample {
            actor,
            epoch,
            authority_tick,
            origin,
            world_step,
            confirmed_step: self.step,
            projected_step,
            received_at: self.received_at,
            applied_sequence: self.applied_sequence,
            queued_frames: self.queue.len(),
            queued_steps: self.queue.iter().fold(0u64, |sum, frame| {
                sum.saturating_add(u64::from(frame.steps))
            }),
            first_start: self.queue.front().map(|frame| frame.start),
            last_end: self.queue.back().and_then(|frame| frame.end().ok()),
        }
    }
    pub fn take(&mut self, world_step: u64) -> Result<Vec<Frame>, String> {
        let mut work = Vec::new();
        let mut steps = 0;
        while let Some(frame) = self.queue.front() {
            let available = u64::from(frame.steps.min(MAX_STEPS - steps))
                .min(world_step.saturating_sub(frame.start)) as u32;
            if available == 0 {
                break;
            }
            let (prefix, suffix) = frame.split(available)?;
            self.queue.pop_front();
            steps += available;
            self.step = prefix.end()?;
            if let Some(suffix) = suffix {
                self.queue.push_front(suffix);
            } else {
                self.applied_sequence = prefix.sequence;
            }
            work.push(prefix);
        }
        Ok(work)
    }
    pub fn validate(
        &self,
        world_step: u64,
        life: LifeId,
        epoch: u64,
        accepted_sequence: u64,
    ) -> Result<(), String> {
        if self.step > world_step
            || self.received_at > world_step
            || self.queue.len() > MAX_QUEUED
            || self.applied_sequence > accepted_sequence
        {
            return Err("Invalid confirmed movement clock".into());
        }
        let mut start = self.step;
        let mut sequence = self.applied_sequence;
        for frame in &self.queue {
            frame.validate()?;
            if frame.life != life
                || frame.epoch != epoch
                || frame.start != start
                || frame.sequence <= sequence
                || frame.sequence > accepted_sequence
                || frame.end()?
                    > world_step
                        .checked_add(u64::from(MAX_STEPS))
                        .ok_or("World movement budget exhausted")?
            {
                return Err("Saved movement interval is foreign or unordered".into());
            }
            start = frame.end()?;
            sequence = frame.sequence;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(start: u64, steps: u32, sequence: u64) -> Frame {
        Frame {
            life: LifeId {
                instance: 1,
                actor: 14,
                generation: 0,
            },
            epoch: 1,
            sequence,
            tick: 1,
            start,
            steps,
            segments: vec![Segment {
                offset: 0,
                axes: [1., 0.],
                yaw: 0.,
                until: start + super::super::HELD_STEPS,
                jump: false,
            }],
        }
    }
    #[test]
    fn queued_intervals_cannot_spend_future_time_or_duplicate_elapsed_time() {
        let mut clock = Clock::new(4, 0);
        clock.admit(frame(4, 4, 1), 4).unwrap();
        assert!(clock.take(4).unwrap().is_empty());
        assert!(
            clock
                .admit(frame(4, 4, 2), 4)
                .unwrap_err()
                .contains("not contiguous")
        );
        assert!(
            clock
                .admit(frame(8, 12, 2), 4)
                .unwrap_err()
                .contains("exceeds authority credit")
        );
        let work = clock.take(8).unwrap();
        assert_eq!(work.len(), 1);
        assert_eq!(clock.step, 8);
        assert!(clock.admit(frame(4, 4, 2), 20).is_err());
        clock.admit(frame(8, 12, 2), 20).unwrap();
        clock.admit(frame(20, 4, 3), 24).unwrap();
        assert_eq!(clock.take(24).unwrap().len(), 1);
        assert_eq!(clock.step, 20);
        assert_eq!(clock.take(24).unwrap().len(), 1);
        assert_eq!(clock.step, 24);
    }
    #[test]
    fn partial_budget_consumes_queued_prefix_before_expiry() {
        let mut clock = Clock::new(72, 1);
        clock.admit(frame(72, 4, 2), 104).unwrap();
        clock.admit(frame(76, 12, 3), 104).unwrap();
        assert!(!clock.expired(112));
        assert_eq!(
            clock
                .expiry_sample(14, 1, 1, 112, ExpiryOrigin::PrimaryTick)
                .projected_step,
            84
        );
        let work = clock.take(112).unwrap();
        assert_eq!(work.iter().map(|f| f.steps).sum::<u32>(), MAX_STEPS);
        assert_eq!(clock.step, 84);
        assert_eq!(clock.applied_sequence, 2);
        clock.validate(112, work[0].life, 1, 3).unwrap();
        assert_eq!(clock.queue.front().unwrap().start, 84);
        assert_eq!(clock.queue.front().unwrap().steps, 4);
        assert_eq!(clock.take(116).unwrap()[0].steps, 4);
        assert_eq!(clock.step, 88);
        assert_eq!(clock.applied_sequence, 3);
    }

    #[test]
    fn partial_intervals_preserve_elapsed_time_segments_and_jump_edges() {
        let mut input = frame(4, 12, 1);
        input.segments[0].jump = true;
        input.segments.push(Segment {
            offset: 3,
            axes: [0., 1.],
            yaw: 1.,
            until: 24,
            jump: true,
        });
        input.segments.push(Segment {
            offset: 8,
            axes: [-1., 0.],
            yaw: 2.,
            until: 24,
            jump: true,
        });
        let expected = expand(&[input.clone()]);
        let mut clock = Clock::new(4, 0);
        clock.admit(input, 4).unwrap();
        let mut actual = Vec::new();
        for world in [7, 12, 16] {
            let work = clock.take(world).unwrap();
            assert!(work.iter().map(|f| f.steps).sum::<u32>() <= MAX_STEPS);
            assert_eq!(clock.step, world);
            clock.validate(world, frame(4, 1, 1).life, 1, 1).unwrap();
            actual.extend(expand(&work));
        }
        assert_eq!(clock.applied_sequence, 1);
        assert!(clock.queue.is_empty());
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(&expected) {
            assert_eq!(
                (a.at, a.held.axes, a.held.until, a.yaw, a.jump),
                (b.at, b.held.axes, b.held.until, b.yaw, b.jump)
            );
        }
    }

    #[test]
    fn refusal_diagnostics_preserve_the_confirmed_clock_and_queue() {
        let mut clock = Clock::new(4, 0);
        let original = serde_json::to_vec(&clock).unwrap();
        for (input, world, reason) in [
            (frame(8, 4, 1), 4, "not contiguous"),
            (frame(4, 4, 0), 4, "bound sequence"),
            (frame(4, 12, 1), 0, "exceeds authority credit"),
            (frame(4, 4, 1), 4 + BOOTSTRAP_LAG + 1, "clock expired"),
        ] {
            assert!(clock.admit(input, world).unwrap_err().contains(reason));
            assert_eq!(serde_json::to_vec(&clock).unwrap(), original);
        }
        clock.admit(frame(4, 4, 1), 4).unwrap();
        let queued = serde_json::to_vec(&clock).unwrap();
        assert!(
            clock
                .admit(frame(8, 4, 1), 4)
                .unwrap_err()
                .contains("does not follow")
        );
        assert_eq!(serde_json::to_vec(&clock).unwrap(), queued);
    }

    #[test]
    fn full_interval_queue_refuses_without_renewing_lag_credit() {
        let mut clock = Clock::new(4, 0);
        for index in 0..MAX_QUEUED as u64 {
            clock
                .admit(frame(4 + index, 1, index + 1), 4 + index)
                .unwrap();
        }
        let before = serde_json::to_vec(&clock).unwrap();
        let start = 4 + MAX_QUEUED as u64;
        assert!(
            clock
                .admit(frame(start, 1, MAX_QUEUED as u64 + 1), start)
                .unwrap_err()
                .contains("queue is full")
        );
        assert_eq!(serde_json::to_vec(&clock).unwrap(), before);
    }

    #[test]
    fn saved_intervals_refuse_foreign_controls_and_invalid_boundaries() {
        let mut clock = Clock::new(4, 0);
        let original = frame(4, 4, 1);
        clock.admit(original.clone(), 4).unwrap();
        clock.validate(4, original.life, 1, 1).unwrap();
        assert!(clock.validate(4, original.life, 2, 1).is_err());
        for offset in [4, u32::MAX] {
            let mut bad = original.clone();
            bad.segments[0].offset = offset;
            assert!(bad.validate().is_err());
        }
        let mut bad = original;
        bad.segments.push(bad.segments[0].clone());
        assert!(bad.validate().is_err());
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Step {
    pub at: u64,
    pub held: super::Held,
    pub yaw: f32,
    pub jump: bool,
}
pub(crate) fn expand(frames: &[Frame]) -> Vec<Step> {
    let mut work = Vec::new();
    for frame in frames {
        let mut index = 0;
        for offset in 0..frame.steps {
            while index + 1 < frame.segments.len() && frame.segments[index + 1].offset <= offset {
                index += 1;
            }
            let segment = &frame.segments[index];
            work.push(Step {
                at: frame.start + u64::from(offset),
                held: super::Held {
                    axes: segment.axes,
                    until: segment.until,
                },
                yaw: segment.yaw,
                jump: segment.jump && segment.offset == offset,
            });
        }
    }
    work
}
