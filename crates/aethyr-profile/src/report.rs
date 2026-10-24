//! What frame and failure reports carry about the device and `Detail`.

use serde::{Deserialize, Serialize};

use crate::{Budget, ControllerState, DeviceProfile, KnobValue, REPORT_SCHEMA};

/// The device profile, the budget, the controller, and every knob's current
/// value. Frame reports and failure reports embed one (ADR 0007).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailReport {
    pub schema: String,
    pub profile: DeviceProfile,
    pub budget: Budget,
    pub controller: ControllerState,
    pub knobs: Vec<KnobValue>,
}

impl DetailReport {
    pub fn new(
        profile: DeviceProfile,
        budget: Budget,
        controller: ControllerState,
        knobs: Vec<KnobValue>,
    ) -> Self {
        Self {
            schema: REPORT_SCHEMA.to_owned(),
            profile,
            budget,
            controller,
            knobs,
        }
    }
}
