//! The resource vector: what one admitted task holds.
//!
//! An admission is a claim on the host, stated in five dimensions. Four
//! of them are quantities the plan sums and bounds — executor slots, CPU
//! units, a memory reservation in MiB, and integration lanes. The fifth,
//! the quiet-host lane, is a flag with an exclusion rule: a task that
//! needs a quiet host admits only onto an empty host, and while it holds
//! the host nothing else admits.
//!
//! None of this is OS isolation. A memory reservation is a number the
//! plan accounts for, not a limit the kernel enforces; CPU units are a
//! share of a stated budget, not `cgroups`. The vector exists so the
//! scheduler can say, deterministically and in the open, why a task did
//! or did not fit.

use serde::{Deserialize, Serialize};

/// What one task holds while admitted.
///
/// `executor_slots` is the number of concurrent-executor places the task
/// occupies — nearly always one. `cpu_units` and `memory_mib` are the
/// build's stated appetite, compared against the host's declared totals.
/// `quiet_host` marks work that must run alone, and `integration` marks
/// work that lands changes and therefore serializes through the
/// integration lane.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Resources {
    /// Executor places held.
    #[serde(default = "one_slot")]
    pub executor_slots: u32,
    /// CPU units held, out of the host's declared total.
    #[serde(default)]
    pub cpu_units: u32,
    /// Memory reservation held, in MiB.
    #[serde(default)]
    pub memory_mib: u64,
    /// The task admits only onto an otherwise empty host, and excludes
    /// all other work — including work admitted after it — while it runs.
    #[serde(default)]
    pub quiet_host: bool,
    /// The task lands changes and holds one integration lane.
    #[serde(default)]
    pub integration: bool,
}

/// What the host offers.
///
/// The host's declared totals for each quantitative dimension, plus the
/// number of integration lanes — ordinarily one, because landing two
/// changes at once is not landing them deterministically.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Capacity {
    /// Concurrent executor places.
    pub executor_slots: u32,
    /// Total CPU units.
    pub cpu_units: u32,
    /// Total memory reservable, in MiB.
    pub memory_mib: u64,
    /// Integration lanes; one in practice.
    #[serde(default = "one_lane")]
    pub integration_lanes: u32,
}

/// What the admitted set currently holds.
///
/// `quiet` is not summed from the vector — a host is quiet because a
/// quiet-host task is admitted, and the flag records that fact rather
/// than a quantity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InUse {
    /// Executor places held.
    pub executor_slots: u32,
    /// CPU units held.
    pub cpu_units: u32,
    /// Memory held, in MiB.
    pub memory_mib: u64,
    /// Integration lanes held.
    pub integration: u32,
    /// A quiet-host task is admitted.
    pub quiet: bool,
}

impl Default for Resources {
    /// A task always occupies an executor place, so the zero vector is
    /// one slot and nothing else.
    fn default() -> Self {
        Self {
            executor_slots: 1,
            cpu_units: 0,
            memory_mib: 0,
            quiet_host: false,
            integration: false,
        }
    }
}

fn one_slot() -> u32 {
    1
}

fn one_lane() -> u32 {
    1
}

/// The resource dimension a request exceeded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    /// Executor places.
    ExecutorSlots,
    /// CPU units.
    CpuUnits,
    /// Memory reservation.
    MemoryMib,
    /// Integration lanes.
    Integration,
}

impl Bound {
    /// The bound's name in explanations.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ExecutorSlots => "executor-slots",
            Self::CpuUnits => "cpu-units",
            Self::MemoryMib => "memory-mib",
            Self::Integration => "integration",
        }
    }
}

impl std::fmt::Display for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl InUse {
    /// Add one task's hold.
    pub fn add(&mut self, resources: &Resources) {
        self.executor_slots = self.executor_slots.saturating_add(resources.executor_slots);
        self.cpu_units = self.cpu_units.saturating_add(resources.cpu_units);
        self.memory_mib = self.memory_mib.saturating_add(resources.memory_mib);
        self.integration = self
            .integration
            .saturating_add(u32::from(resources.integration));
        self.quiet |= resources.quiet_host;
    }

    /// The first bound `resources` would exceed, added to this hold.
    ///
    /// The quiet-host lane is not checked here — exclusion is a
    /// scheduling rule the plan applies, not a quantity.
    #[must_use]
    pub fn exceeds(&self, capacity: &Capacity, resources: &Resources) -> Option<Bound> {
        if self
            .executor_slots
            .checked_add(resources.executor_slots)
            .is_none_or(|sum| sum > capacity.executor_slots)
        {
            return Some(Bound::ExecutorSlots);
        }
        if self
            .cpu_units
            .checked_add(resources.cpu_units)
            .is_none_or(|sum| sum > capacity.cpu_units)
        {
            return Some(Bound::CpuUnits);
        }
        if self
            .memory_mib
            .checked_add(resources.memory_mib)
            .is_none_or(|sum| sum > capacity.memory_mib)
        {
            return Some(Bound::MemoryMib);
        }
        if self
            .integration
            .checked_add(u32::from(resources.integration))
            .is_none_or(|sum| sum > capacity.integration_lanes)
        {
            return Some(Bound::Integration);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_arithmetic_cannot_wrap_into_spare_capacity() {
        let capacity = Capacity {
            executor_slots: u32::MAX,
            cpu_units: u32::MAX,
            memory_mib: u64::MAX,
            integration_lanes: u32::MAX,
        };
        let mut used = InUse {
            executor_slots: u32::MAX,
            ..InUse::default()
        };
        assert_eq!(
            used.exceeds(
                &capacity,
                &Resources {
                    executor_slots: 1,
                    ..Resources::default()
                }
            ),
            Some(Bound::ExecutorSlots)
        );
        used.executor_slots = 0;
        used.memory_mib = u64::MAX;
        assert_eq!(
            used.exceeds(
                &capacity,
                &Resources {
                    memory_mib: 1,
                    ..Resources::default()
                }
            ),
            Some(Bound::MemoryMib)
        );
    }

    fn host() -> Capacity {
        Capacity {
            executor_slots: 2,
            cpu_units: 8,
            memory_mib: 8192,
            integration_lanes: 1,
        }
    }

    #[test]
    fn the_first_exceeded_bound_is_named() {
        let task = Resources {
            executor_slots: 1,
            cpu_units: 4,
            memory_mib: 4096,
            ..Resources::default()
        };
        let mut used = InUse::default();
        used.add(&task);
        assert_eq!(used.exceeds(&host(), &task), None);
        used.add(&task);
        assert_eq!(
            used.exceeds(&host(), &task),
            Some(Bound::ExecutorSlots),
            "two tasks fill both slots"
        );
    }

    #[test]
    fn memory_is_accounted_in_mib() {
        let big = Resources {
            executor_slots: 1,
            cpu_units: 1,
            memory_mib: 9000,
            ..Resources::default()
        };
        assert_eq!(
            InUse::default().exceeds(&host(), &big),
            Some(Bound::MemoryMib)
        );
    }

    #[test]
    fn one_integration_lane_serializes_landing() {
        let landing = Resources {
            integration: true,
            ..Resources::default()
        };
        let mut used = InUse::default();
        used.add(&landing);
        assert_eq!(
            used.exceeds(&host(), &landing),
            Some(Bound::Integration),
            "a second landing waits for the lane"
        );
    }
}
