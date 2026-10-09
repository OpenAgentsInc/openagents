//! Builds complete intervals from predicted input without changing authority.
use super::*;
use crate::movement::{
    Profile,
    frames::{Frame, MAX_STEPS, Segment},
};
impl Local {
    pub fn movement_profile(&self) -> Option<Profile> {
        self.baseline.map(|b| b.profile)
    }
    /// Caps completed local steps by the latest verified authority world-time credit.
    pub fn movement_frame_limit(&self) -> Option<u64> {
        self.baseline
            .filter(|b| b.profile == Profile::Frames)
            .map(|_| {
                self.step
                    .min(self.world_credit.saturating_add(u64::from(MAX_STEPS)))
            })
    }
    /// Refreshes exact authority credit without advancing prediction or replaying input.
    /// A response from another life or epoch cannot renew this clock.
    pub fn movement_credit(
        &mut self,
        life: LifeId,
        epoch: u64,
        world_step: u64,
    ) -> Result<(), String> {
        let baseline = self
            .baseline
            .ok_or("Movement credit has no prediction baseline")?;
        if baseline.profile != Profile::Frames
            || self.context() != Some((life, epoch))
            || world_step < self.world_credit
            || world_step.checked_add(u64::from(MAX_STEPS)).is_none()
        {
            return Err("Movement credit context or clock is invalid".into());
        }
        self.world_credit = world_step;
        Ok(())
    }
    /// Admits verified authority time for the active owned interval context.
    pub fn grant_world_credit(
        &mut self,
        life: LifeId,
        epoch: u64,
        world_step: u64,
    ) -> Result<(), String> {
        self.movement_credit(life, epoch, world_step)
    }
    pub fn physics_step(&self) -> u64 {
        self.step
    }
    /// Recovers elapsed authority time in one bounded prediction batch.
    pub fn recover_world_credit(&mut self) -> Result<(), String> {
        if self.movement_profile() != Some(Profile::Frames) {
            return Ok(());
        }
        let steps = self
            .world_credit
            .saturating_sub(self.step)
            .min(u64::from(MAX_STEPS));
        if steps > 0 {
            self.advance(steps as f64 / 120.)?;
        }
        Ok(())
    }
    /// An unbound proposal has sequence zero; the transport must bind it before submission.
    pub fn movement_frame(&self, start: u64, steps: u32) -> Result<Frame, String> {
        let baseline = self
            .baseline
            .ok_or("Movement interval prediction is inactive")?;
        let end = start
            .checked_add(u64::from(steps))
            .ok_or("Movement interval clock exhausted")?;
        if baseline.profile != Profile::Frames
            || start < baseline.physics_step
            || steps == 0
            || steps > MAX_STEPS
            || end > self.step
        {
            return Err("Movement proposal is outside the confirmed prediction interval".into());
        }
        let mut held = baseline.held;
        let mut yaw = self.movement_yaw(&baseline);
        for input in self.inputs.iter().filter(|i| i.step < start) {
            if let Intent::Move { axes, yaw: next } = input.intent {
                held.refresh(axes, input.step)?;
                yaw = next;
            }
        }
        let mut points: std::collections::BTreeSet<_> = self
            .inputs
            .iter()
            .filter(|i| i.step >= start && i.step < end)
            .map(|i| i.step)
            .collect();
        points.insert(start);
        let mut segments = Vec::new();
        for step in points {
            let mut jump = false;
            for input in self.inputs.iter().filter(|i| i.step == step) {
                match input.intent {
                    Intent::Move { axes, yaw: next } => {
                        held.refresh(axes, step)?;
                        yaw = next;
                    }
                    Intent::Jump => jump = true,
                    _ => return Err("Unsupported movement interval intent".into()),
                }
            }
            segments.push(Segment {
                offset: (step - start) as u32,
                axes: held.axes,
                until: held.until,
                yaw,
                jump,
            });
        }
        let frame = Frame {
            life: baseline.life,
            epoch: baseline.epoch,
            sequence: 0,
            tick: self.tick,
            start,
            steps,
            segments,
        };
        frame.validate_payload()?;
        Ok(frame)
    }
    /// Binds every input covered by a transmitted interval, including its carried hold.
    pub fn bind_movement_frame(&mut self, frame: &Frame) -> Result<(), String> {
        frame.validate()?;
        let baseline = self
            .baseline
            .ok_or("Movement interval prediction is inactive")?;
        if frame.life != baseline.life
            || frame.epoch != baseline.epoch
            || frame.sequence <= baseline.applied_sequence
        {
            return Err("Movement interval binding has a foreign control".into());
        }
        let expected = self.movement_frame(frame.start, frame.steps)?;
        if expected.segments != frame.segments {
            return Err("Movement interval binding changes predicted input".into());
        }
        let end = frame.end()?;
        if self
            .inputs
            .iter()
            .any(|i| i.step >= end && i.sequence.is_some_and(|seq| seq <= frame.sequence))
        {
            return Err("Movement interval binding regresses sequence order".into());
        }
        for input in self
            .inputs
            .iter_mut()
            .filter(|i| i.step < end && i.sequence.is_none())
        {
            input.sequence = Some(frame.sequence);
        }
        Ok(())
    }
}
