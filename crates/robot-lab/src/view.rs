//! The checked scene and episode shared by chat, rooms, and devices.
use crate::document::{Episode, Reference, Robot, Scene};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub schema: String,
    pub subject: Reference,
    pub number: u64,
    pub scene_reference: Reference,
    pub scene: Scene,
    pub robot_reference: Reference,
    pub robot: Robot,
    pub episode: Option<Episode>,
}

impl Bundle {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "robot-view.v1" || self.number == 0 {
            return Err("This robot view couldn't be opened".into());
        }
        self.subject.validate().map_err(|e| e.to_string())?;
        self.scene_reference.validate().map_err(|e| e.to_string())?;
        self.robot_reference.validate().map_err(|e| e.to_string())?;
        self.robot.validate().map_err(|e| e.to_string())?;
        self.scene.validate().map_err(|e| e.to_string())?;
        if self.robot.source_axes != crate::document::Axes::ZUp || self.robot.meters_per_unit != 1.0
        {
            return Err("Use a checked URDF in meters for this robot view".into());
        }
        if self
            .scene
            .objects
            .iter()
            .any(|o| matches!(o.geometry.shape, crate::document::Shape::Mesh { .. }))
        {
            return Err("Use primitive objects in this robot view".into());
        }
        if self.scene.robots.len() != 1 || self.scene.robots[0].robot != self.robot_reference {
            return Err("Open a scene with its saved robot".into());
        }
        self.robot
            .validate_pose(&self.scene.robots[0].joints)
            .map_err(|e| e.to_string())?;
        let movable: Vec<_> = self
            .robot
            .joints
            .iter()
            .filter(|j| j.kind != crate::document::JointKind::Fixed)
            .collect();
        if !movable
            .iter()
            .all(|j| self.scene.robots[0].joints.contains_key(&j.name))
        {
            return Err("Set a starting position for every movable joint".into());
        }
        if let Some(episode) = &self.episode {
            episode.validate().map_err(|e| e.to_string())?;
            for step in &episode.steps {
                self.robot
                    .validate_state(&step.joints)
                    .map_err(|e| e.to_string())?;
                if !movable.iter().all(|j| step.joints.contains_key(&j.name)) {
                    return Err("Record every movable joint before opening this attempt".into());
                }
                for joint in &movable {
                    if joint.limits.is_some_and(|l| {
                        step.joints[&joint.name] < l.lower || step.joints[&joint.name] > l.upper
                    }) && !step.limit_violations.contains(&joint.name)
                    {
                        return Err("Record each joint position that exceeds its limits".into());
                    }
                }
            }
        } else if self.subject != self.scene_reference {
            return Err("This robot view names a different scene".into());
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 18 << 20 {
            return Err("This robot view is too large".into());
        }
        Ok(())
    }
    pub fn mode(&self) -> &'static str {
        match &self.episode {
            None => "Inspecting",
            Some(episode) if episode.mode == crate::document::EpisodeMode::Demonstration => {
                "Demonstration"
            }
            Some(_) => "Saved attempt",
        }
    }
}
