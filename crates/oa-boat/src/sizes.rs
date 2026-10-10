//! Boat's size classes on GCE machine types, and what an hour of each costs.
//!
//! Prices are Google's public list prices for us-central1 (Cloud Billing
//! catalog, 2026-10-10): E2 core $0.02181/h and RAM $0.002924/GiB-h (spot
//! $0.01309 and $0.001754); N2D core $0.027502/h and RAM $0.003686/GiB-h
//! (spot $0.01401 and $0.001876); pd-balanced $0.10/GiB-month. They are an
//! estimate for reporting, never a bill.

/// One size class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    /// Boat's name for it (`small`, `default`, `large`, `xlarge`).
    pub name: &'static str,
    pub machine: &'static str,
    pub vcpu: i64,
    pub memory_gb: i64,
    /// The least boot disk; a larger template image wins.
    pub disk_gb: i64,
    /// Compute dollars an hour, on demand.
    pub standard_hourly: f64,
    /// Compute dollars an hour as a spot VM.
    pub spot_hourly: f64,
}

pub const SIZES: [Size; 4] = [
    Size {
        name: "small",
        machine: "e2-standard-2",
        vcpu: 2,
        memory_gb: 8,
        disk_gb: 50,
        standard_hourly: 0.06701,
        spot_hourly: 0.04021,
    },
    Size {
        name: "default",
        machine: "e2-standard-4",
        vcpu: 4,
        memory_gb: 16,
        disk_gb: 100,
        standard_hourly: 0.13402,
        spot_hourly: 0.08042,
    },
    Size {
        name: "large",
        machine: "n2d-standard-8",
        vcpu: 8,
        memory_gb: 32,
        disk_gb: 200,
        standard_hourly: 0.33797,
        spot_hourly: 0.17211,
    },
    Size {
        name: "xlarge",
        machine: "n2d-standard-16",
        vcpu: 16,
        memory_gb: 64,
        disk_gb: 300,
        standard_hourly: 0.67594,
        spot_hourly: 0.34422,
    },
];

/// pd-balanced: $0.10 per GiB-month over 730 hours.
pub const DISK_HOURLY_PER_GB: f64 = 0.10 / 730.0;

/// The class Boat calls `name`; `None` for an unknown one.
pub fn size(name: &str) -> Option<Size> {
    SIZES.iter().copied().find(|s| s.name == name)
}

/// The class a GCE machine type belongs to.
pub fn by_machine(machine: &str) -> Option<Size> {
    SIZES.iter().copied().find(|s| s.machine == machine)
}

/// How a VM is provisioned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provisioning {
    /// On demand: never preempted. Interactive work.
    Standard,
    /// Spot: preemptible, stops (keeping its disk) when GCE takes it back.
    Spot,
}

impl Provisioning {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Spot => "spot",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "standard" | "on-demand" | "ondemand" => Some(Self::Standard),
            "spot" => Some(Self::Spot),
            _ => None,
        }
    }
}

impl Size {
    pub fn hourly(&self, p: Provisioning) -> f64 {
        match p {
            Provisioning::Standard => self.standard_hourly,
            Provisioning::Spot => self.spot_hourly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_follow_the_catalog_rates() {
        let e2 = |c: f64, m: f64| c * 0.02181159 + m * 0.00292353;
        let n2d = |c: f64, m: f64| c * 0.027502 + m * 0.003686;
        for s in SIZES {
            let want = if s.machine.starts_with("e2") {
                e2(s.vcpu as f64, s.memory_gb as f64)
            } else {
                n2d(s.vcpu as f64, s.memory_gb as f64)
            };
            assert!((s.standard_hourly - want).abs() < 0.0005, "{}", s.name);
            assert!(s.spot_hourly < s.standard_hourly);
        }
        assert_eq!(size("large").unwrap().machine, "n2d-standard-8");
        assert_eq!(by_machine("e2-standard-2").unwrap().name, "small");
        assert!(size("huge").is_none());
    }
}
