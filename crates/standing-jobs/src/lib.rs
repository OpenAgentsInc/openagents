//! Finite slot and allowance primitives shared by standing jobs and NIP-AUTO.
//! This crate owns no executor, credentials, source adapter, or job history.
/// A fixed UTC interval. Every operation checks arithmetic and its finite end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub first: u64,
    pub every: u64,
    pub expires: u64,
}
impl Interval {
    pub fn latest(self, now: u64) -> Option<(u64, u64)> {
        if self.every == 0 || self.first >= self.expires || now < self.first {
            return None;
        }
        let last = now.min(self.expires.checked_sub(1)?);
        let slot = last.checked_sub(self.first)?.checked_div(self.every)?;
        Some((slot, self.due(slot)?))
    }
    pub fn due(self, slot: u64) -> Option<u64> {
        self.every
            .checked_mul(slot)?
            .checked_add(self.first)
            .filter(|due| self.every > 0 && *due < self.expires)
    }
}
/// Admission includes every unresolved occurrence at its full reserved ceiling.
#[must_use]
pub fn money_available(spent: f64, per_occurrence: f64, unmetered: u32, ceiling: f64) -> bool {
    if ![spent, per_occurrence, ceiling]
        .iter()
        .all(|n| n.is_finite() && *n >= 0.0)
    {
        return false;
    }
    let reserved = per_occurrence * (f64::from(unmetered) + 1.0);
    reserved.is_finite() && (spent + reserved).is_finite() && spent + reserved <= ceiling
}
/// Integer admission never treats an unknown hold as unused allowance.
#[must_use]
pub fn fits(spent: u64, held: u64, requested: u64, ceiling: u64) -> bool {
    spent
        .checked_add(held)
        .and_then(|n| n.checked_add(requested))
        .is_some_and(|n| n <= ceiling)
}
/// The legacy daily/weekly schedule; timezone offsets remain explicit minutes.
/// AUTO itself uses fixed UTC intervals and never infers a calendar schedule.
#[must_use]
pub fn calendar_slot(at: &str, weekday: Option<u8>, utc_offset: i32, now: u64) -> Option<u64> {
    let (hour, minute) = at.split_once(':')?;
    let hour = hour.parse::<i64>().ok()?;
    let minute = minute.parse::<i64>().ok()?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        return None;
    }
    let now = i64::try_from(now).ok()?;
    let offset = i64::from(utc_offset).checked_mul(60)?;
    let local = now.checked_add(offset)?;
    let day = local.div_euclid(86_400);
    for back in 0..8 {
        let d = day.checked_sub(back)?;
        if weekday.is_some_and(|w| w != (d + 3).rem_euclid(7) as u8) {
            continue;
        }
        let slot = d
            .checked_mul(86_400)?
            .checked_add((hour * 60 + minute) * 60)?
            .checked_sub(offset)?;
        if slot <= now {
            return u64::try_from(slot).ok();
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missed_fixed_slots_select_only_the_latest_finite_slot() {
        let interval = Interval {
            first: 100,
            every: 60,
            expires: 300,
        };
        assert_eq!(interval.latest(99), None);
        assert_eq!(interval.latest(299), Some((3, 280)));
        assert_eq!(interval.latest(10000), Some((3, 280)));
        assert_eq!(interval.due(4), None);
        assert_eq!(interval.due(u64::MAX), None);
    }
    #[test]
    fn pending_costs_stay_held_and_invalid_money_is_not_zero() {
        assert!(!money_available(0.0, 1.0, 1, 1.0));
        assert!(money_available(0.0, 1.0, 0, 1.0));
        assert!(!money_available(f64::NAN, 1.0, 0, 10.0));
        assert!(!fits(1, 2, 1, 3));
        assert!(!fits(u64::MAX, 0, 1, u64::MAX));
    }
    #[test]
    fn calendar_offsets_and_extreme_times_are_checked() {
        const MONDAY: u64 = 1_791_158_400;
        assert_eq!(
            calendar_slot("02:00", Some(0), 0, MONDAY + 3 * 86400),
            Some(MONDAY + 7200)
        );
        assert_eq!(
            calendar_slot("02:00", None, -120, MONDAY + 18000),
            Some(MONDAY + 14400)
        );
        assert_eq!(calendar_slot("02:00", None, 0, u64::MAX), None);
    }
}

pub mod ledger;
