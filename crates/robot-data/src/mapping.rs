//! Explicit feature-to-joint conversion; no calibration is inferred.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{Result, name, require};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub schema: String,
    pub robot_type: String,
    pub robot_version: String,
    pub joints: Vec<JointMapping>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointMapping {
    pub feature: String,
    pub joint: String,
    pub unit: Unit,
    /// Added after conversion into radians or meters.
    pub offset: f64,
    /// -1 or 1; robot and recording frames may have opposite positive axes.
    pub direction: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "unit", rename_all = "snake_case", deny_unknown_fields)]
pub enum Unit {
    Radians,
    Degrees,
    Meters,
    Normalized {
        source_min: f64,
        source_max: f64,
        target_min: f64,
        target_max: f64,
    },
}

impl Mapping {
    pub fn validate(&self, robot_type: &str, features: &[String]) -> Result<()> {
        require(
            self.schema == "robot-joint-mapping.v1",
            "Choose a supported joint mapping.",
        )?;
        require(
            self.robot_type == robot_type,
            "Choose a joint mapping for this recording's robot.",
        )?;
        name(&self.robot_type)?;
        require(
            self.robot_version.len() == 64
                && self.robot_version.bytes().all(|b| b.is_ascii_hexdigit()),
            "Choose an exact saved robot version.",
        )?;
        require(
            !self.joints.is_empty() && self.joints.len() == features.len() && features.len() <= 64,
            "Map every recorded joint exactly once.",
        )?;
        let mut joints = BTreeSet::new();
        let mut mapped = BTreeSet::new();
        for joint in &self.joints {
            name(&joint.feature)?;
            name(&joint.joint)?;
            require(
                joints.insert(&joint.joint)
                    && mapped.insert(&joint.feature)
                    && features.contains(&joint.feature),
                "Map each feature to a different robot joint.",
            )?;
            require(
                joint.offset.is_finite()
                    && joint.lower.is_finite()
                    && joint.upper.is_finite()
                    && joint.lower < joint.upper
                    && (joint.direction == -1.0 || joint.direction == 1.0),
                "Use finite mapping offsets, limits, and a direction of minus one or one.",
            )?;
            if let Unit::Normalized {
                source_min,
                source_max,
                target_min,
                target_max,
            } = joint.unit
            {
                require(
                    [source_min, source_max, target_min, target_max]
                        .iter()
                        .all(|v| v.is_finite())
                        && source_min < source_max
                        && target_min < target_max,
                    "Specify the recording's calibration range and the robot's joint range.",
                )?;
            }
        }
        Ok(())
    }

    pub fn convert(&self, features: &[String], values: &[f64]) -> Result<BTreeMap<String, f64>> {
        self.validate(&self.robot_type, features)?;
        require(
            values.len() == features.len() && features.len() == self.joints.len(),
            "The recorded joint count doesn't match its mapping.",
        )?;
        let mut result = BTreeMap::new();
        for joint in &self.joints {
            let index = features
                .iter()
                .position(|f| f == &joint.feature)
                .ok_or_else(|| {
                    crate::Error::Invalid("The mapping names a missing feature.".into())
                })?;
            let input = values[index];
            require(
                input.is_finite(),
                "The recording contains an invalid joint value.",
            )?;
            let value = match joint.unit {
                Unit::Radians | Unit::Meters => input,
                Unit::Degrees => input.to_radians(),
                Unit::Normalized {
                    source_min,
                    source_max,
                    target_min,
                    target_max,
                } => {
                    require(
                        input >= source_min && input <= source_max,
                        "A recorded joint is outside its calibration range.",
                    )?;
                    target_min
                        + (input - source_min) / (source_max - source_min)
                            * (target_max - target_min)
                }
            } * joint.direction
                + joint.offset;
            require(
                value.is_finite() && value >= joint.lower && value <= joint.upper,
                "A recorded pose is outside this robot's joint limits.",
            )?;
            result.insert(joint.joint.clone(), value);
        }
        Ok(result)
    }
}
