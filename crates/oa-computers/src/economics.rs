//! Service costs and occupied worker capacity for one measured window.
//! Missing measurements stay unknown. These reports never charge an account.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub duration_ms: u64,
    pub worker_vcpus: u32,
    /// Allocated guest vCPUs multiplied by observed running milliseconds.
    /// Use `None` if any running interval in the window is unknown.
    pub guest_vcpu_ms: Option<u64>,
    /// Sum of command wall times, including concurrent commands. This is not CPU time.
    pub command_wall_ms: Option<u64>,
    pub task_attempts: u64,
    /// Verified useful results; a successful process exit alone does not qualify.
    pub useful_tasks: u64,
    /// Complete provider cost for this window, including shared idle capacity,
    /// disks, database, NAT, storage, and transfers. Use `None` if incomplete.
    pub provider_cost_micros: Option<u64>,
    pub currency: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub duration_ms: u64,
    pub worker_capacity_vcpu_ms: u64,
    pub guest_vcpu_ms: Option<u64>,
    /// Capacity without an allocated guest, not a measurement of idle CPU usage.
    pub unallocated_worker_vcpu_ms: Option<u64>,
    pub command_wall_ms: Option<u64>,
    pub task_attempts: u64,
    pub useful_tasks: u64,
    pub provider_cost_micros: Option<u64>,
    /// Total service cost divided by verified useful results, rounded up.
    /// Shared costs stay in this service report, never in a customer's usage.
    pub service_cost_per_useful_task_micros: Option<u64>,
    pub currency: String,
    pub customer_charge: Option<u64>,
    pub pricing: &'static str,
}

impl Window {
    pub fn report(&self) -> Result<Report, &'static str> {
        if self.duration_ms == 0 || !(1..=4096).contains(&self.worker_vcpus) {
            return Err("Use a positive duration and a worker size from 1 to 4096 vCPUs.");
        }
        if self.currency.len() != 3 || !self.currency.bytes().all(|c| c.is_ascii_uppercase()) {
            return Err("Use a three-letter currency code.");
        }
        if self.useful_tasks > self.task_attempts {
            return Err("Useful tasks cannot exceed the number of attempts.");
        }
        let capacity = self
            .duration_ms
            .checked_mul(u64::from(self.worker_vcpus))
            .ok_or("The measurement window is too large.")?;
        let unallocated = self
            .guest_vcpu_ms
            .map(|guest| {
                capacity
                    .checked_sub(guest)
                    .ok_or("Guest capacity exceeds the configured worker capacity.")
            })
            .transpose()?;
        let per_useful = self.provider_cost_micros.and_then(|cost| {
            (self.useful_tasks > 0)
                .then(|| cost / self.useful_tasks + u64::from(cost % self.useful_tasks != 0))
        });
        Ok(Report {
            schema: "openagents.computers.service-economics.v1",
            duration_ms: self.duration_ms,
            worker_capacity_vcpu_ms: capacity,
            guest_vcpu_ms: self.guest_vcpu_ms,
            unallocated_worker_vcpu_ms: unallocated,
            command_wall_ms: self.command_wall_ms,
            task_attempts: self.task_attempts,
            useful_tasks: self.useful_tasks,
            provider_cost_micros: self.provider_cost_micros,
            service_cost_per_useful_task_micros: per_useful,
            currency: self.currency.clone(),
            customer_charge: None,
            pricing: "not_enabled",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> Window {
        Window {
            duration_ms: 1000,
            worker_vcpus: 8,
            guest_vcpu_ms: Some(2500),
            command_wall_ms: Some(1500),
            task_attempts: 4,
            useful_tasks: 3,
            provider_cost_micros: Some(10),
            currency: "USD".into(),
        }
    }

    #[test]
    fn capacity_and_shared_costs_stay_at_service_scope() {
        let report = window().report().unwrap();
        assert_eq!(report.worker_capacity_vcpu_ms, 8000);
        assert_eq!(report.unallocated_worker_vcpu_ms, Some(5500));
        assert_eq!(report.service_cost_per_useful_task_micros, Some(4));
        assert_eq!(report.command_wall_ms, Some(1500));
        assert_eq!(report.customer_charge, None);
        assert_eq!(report.pricing, "not_enabled");
    }

    #[test]
    fn outages_do_not_become_zero_usage_or_zero_cost() {
        let mut input = window();
        input.guest_vcpu_ms = None;
        input.command_wall_ms = None;
        input.provider_cost_micros = None;
        let report = input.report().unwrap();
        assert_eq!(report.unallocated_worker_vcpu_ms, None);
        assert_eq!(report.command_wall_ms, None);
        assert_eq!(report.service_cost_per_useful_task_micros, None);
        let value = serde_json::to_value(report).unwrap();
        assert!(value["guest_vcpu_ms"].is_null());
        assert!(value["provider_cost_micros"].is_null());
        assert!(value["customer_charge"].is_null());
    }

    #[test]
    fn observed_zero_is_distinct_from_missing_measurements() {
        let mut input = window();
        input.guest_vcpu_ms = Some(0);
        input.provider_cost_micros = Some(0);
        let report = input.report().unwrap();
        assert_eq!(report.unallocated_worker_vcpu_ms, Some(8000));
        assert_eq!(report.service_cost_per_useful_task_micros, Some(0));
        input.useful_tasks = 0;
        assert_eq!(
            input.report().unwrap().service_cost_per_useful_task_micros,
            None
        );
    }

    #[test]
    fn maxima_round_without_overflow_and_invalid_totals_refuse() {
        let mut input = window();
        input.provider_cost_micros = Some(u64::MAX);
        input.useful_tasks = 1;
        assert_eq!(
            input.report().unwrap().service_cost_per_useful_task_micros,
            Some(u64::MAX)
        );
        input.duration_ms = u64::MAX;
        assert!(input.report().is_err());
        input = window();
        input.guest_vcpu_ms = Some(8001);
        assert!(input.report().is_err());
        input = window();
        input.useful_tasks = 5;
        assert!(input.report().is_err());
    }

    #[test]
    fn input_cannot_supply_a_customer_charge_or_unknown_fields() {
        let input = serde_json::json!({"duration_ms":1000,"worker_vcpus":1,
            "guest_vcpu_ms":null,"command_wall_ms":null,"task_attempts":0,
            "useful_tasks":0,"provider_cost_micros":null,"currency":"USD",
            "customer_charge":100});
        assert!(serde_json::from_value::<Window>(input).is_err());
    }
}
