//! Atomic multidimensional reservations; unresolved usage remains held.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub type Units = BTreeMap<String, u64>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    pub reserved: Units,
    pub settled: Option<Units>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub ceiling: Units,
    pub spent: Units,
    pub requests: BTreeMap<String, Hold>,
}
impl Ledger {
    pub fn new(ceiling: Units) -> Result<Self, String> {
        if ceiling.is_empty() {
            return Err("a finite aggregate allowance is required".into());
        }
        Ok(Self {
            ceiling,
            spent: Units::new(),
            requests: BTreeMap::new(),
        })
    }
    pub fn held(&self, name: &str) -> Result<u64, String> {
        self.requests
            .values()
            .filter(|h| h.settled.is_none())
            .try_fold(0u64, |n, h| {
                n.checked_add(h.reserved.get(name).copied().unwrap_or(0))
                    .ok_or_else(|| "reservation sum overflow".into())
            })
    }
    pub fn reserve(&mut self, id: &str, requested: Units) -> Result<bool, String> {
        if let Some(previous) = self.requests.get(id) {
            if previous.reserved != requested {
                return Err("idempotency_conflict: changed reservation".into());
            }
            return Ok(false);
        }
        if self.requests.len() >= 4096 {
            return Err("reservation retention limit exceeded".into());
        }
        if requested
            .keys()
            .any(|name| !self.ceiling.contains_key(name))
        {
            return Err("unsupported aggregate accounting dimension".into());
        }
        for (name, ceiling) in &self.ceiling {
            if !super::fits(
                self.spent.get(name).copied().unwrap_or(0),
                self.held(name)?,
                requested.get(name).copied().unwrap_or(0),
                *ceiling,
            ) {
                return Err(format!("aggregate {name} allowance is spent or held"));
            }
        }
        self.requests.insert(
            id.into(),
            Hold {
                reserved: requested,
                settled: None,
            },
        );
        Ok(true)
    }
    /// Unknown usage keeps the entire original hold. A known outcome never
    /// refunds what was spent or permits a second settlement with changed units.
    pub fn settle(&mut self, id: &str, actual: Option<Units>) -> Result<(), String> {
        let hold = self.requests.get(id).ok_or("reservation unavailable")?;
        let Some(actual) = actual else {
            return Ok(());
        };
        if let Some(previous) = &hold.settled {
            if *previous != actual {
                return Err("conflict: changed settled usage".into());
            }
            return Ok(());
        }
        if actual.keys().any(|name| !hold.reserved.contains_key(name))
            || hold
                .reserved
                .iter()
                .any(|(name, ceiling)| actual.get(name).copied().unwrap_or(0) > *ceiling)
        {
            return Err("unknown: usage exceeded its reserved ceiling; hold retained".into());
        }
        let mut spent = self.spent.clone();
        for (name, used) in &actual {
            let n = spent
                .get(name)
                .copied()
                .unwrap_or(0)
                .checked_add(*used)
                .ok_or("spent allowance overflow")?;
            spent.insert(name.clone(), n);
        }
        self.spent = spent;
        self.requests.get_mut(id).ok_or("reservation")?.settled = Some(actual);
        Ok(())
    }
    pub fn unresolved(&self) -> bool {
        self.requests.values().any(|h| h.settled.is_none())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn units(calls: u64, wall: u64) -> Units {
        BTreeMap::from([("calls".into(), calls), ("wall_ms".into(), wall)])
    }
    #[test]
    fn concurrent_children_unknown_usage_and_periods_cannot_replenish_allowance() {
        let mut l = Ledger::new(units(3, 100)).unwrap();
        assert!(l.reserve("first", units(2, 80)).unwrap());
        assert!(!l.reserve("first", units(2, 80)).unwrap());
        assert!(l.reserve("changed", units(2, 30)).is_err());
        l.settle("first", None).unwrap();
        assert_eq!(l.held("calls").unwrap(), 2);
        l.settle("first", Some(units(2, 50))).unwrap();
        l.reserve("later-period", units(1, 50)).unwrap();
        l.settle("later-period", Some(units(1, 1))).unwrap();
        assert!(!l.unresolved());
        assert!(l.reserve("new-period", units(1, 1)).is_err());
    }
    #[test]
    fn overruns_and_changed_retries_keep_the_original_hold() {
        let mut l = Ledger::new(units(1, 100)).unwrap();
        l.reserve("work", units(1, 50)).unwrap();
        assert!(l.reserve("work", units(1, 51)).is_err());
        assert!(l.settle("work", Some(units(1, 51))).is_err());
        assert_eq!(l.held("wall_ms").unwrap(), 50);
        l.settle("work", Some(units(1, 10))).unwrap();
        assert!(l.settle("work", Some(units(0, 0))).is_err());
    }
}
