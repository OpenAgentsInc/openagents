//! Minimal cron expression parser/runtime (5-field) for goal scheduling.

use std::collections::BTreeSet;

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CronScheduleSpec {
    pub minutes: BTreeSet<u32>,
    pub hours: BTreeSet<u32>,
    pub days_of_month: BTreeSet<u32>,
    pub months: BTreeSet<u32>,
    pub days_of_week: BTreeSet<u32>,
}

pub fn parse_cron_expression(expression: &str) -> Result<CronScheduleSpec, String> {
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(format!(
            "cron expression must have 5 fields (minute hour day-of-month month day-of-week), got {}",
            fields.len()
        ));
    }

    Ok(CronScheduleSpec {
        minutes: parse_field(fields[0], 0, 59, "minute")?,
        hours: parse_field(fields[1], 0, 23, "hour")?,
        days_of_month: parse_field(fields[2], 1, 31, "day-of-month")?,
        months: parse_field(fields[3], 1, 12, "month")?,
        days_of_week: normalize_days_of_week(parse_field(fields[4], 0, 7, "day-of-week")?),
    })
}

pub fn next_cron_run_epoch_seconds(
    spec: &CronScheduleSpec,
    timezone: &str,
    now_epoch_seconds: u64,
) -> Result<u64, String> {
    if !timezone.eq_ignore_ascii_case("utc")
        && !timezone.eq_ignore_ascii_case("etc/utc")
        && !timezone.eq_ignore_ascii_case("z")
    {
        return Err(format!(
            "unsupported cron timezone '{}' (supported: UTC)",
            timezone
        ));
    }

    let now = Utc
        .timestamp_opt(now_epoch_seconds as i64, 0)
        .single()
        .ok_or_else(|| "invalid epoch timestamp for cron evaluation".to_string())?;
    let mut candidate = round_up_to_next_minute(now);
    let upper_bound = now + Duration::days(366 * 5);

    while candidate <= upper_bound {
        if cron_matches(spec, candidate) {
            return Ok(candidate.timestamp() as u64);
        }
        candidate += Duration::minutes(1);
    }

    Err("unable to find next cron run in evaluation window".to_string())
}

fn parse_field(field: &str, min: u32, max: u32, label: &str) -> Result<BTreeSet<u32>, String> {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} field is empty"));
    }

    let mut values = BTreeSet::new();
    for part in trimmed.split(',') {
        parse_part(part.trim(), min, max, label, &mut values)?;
    }

    if values.is_empty() {
        return Err(format!("{label} field did not produce any values"));
    }
    Ok(values)
}

fn parse_part(
    part: &str,
    min: u32,
    max: u32,
    label: &str,
    output: &mut BTreeSet<u32>,
) -> Result<(), String> {
    if part == "*" {
        for value in min..=max {
            output.insert(value);
        }
        return Ok(());
    }

    let (range_expr, step) = if let Some((left, right)) = part.split_once('/') {
        let parsed_step = right
            .parse::<u32>()
            .map_err(|_| format!("{label} step '{}' is invalid", right))?;
        if parsed_step == 0 {
            return Err(format!("{label} step must be greater than zero"));
        }
        (left, parsed_step)
    } else {
        (part, 1)
    };

    if range_expr == "*" {
        let mut value = min;
        while value <= max {
            output.insert(value);
            value = value.saturating_add(step);
            if value == u32::MAX {
                break;
            }
        }
        return Ok(());
    }

    let (start, end) = if let Some((left, right)) = range_expr.split_once('-') {
        let start = parse_value(left, min, max, label)?;
        let end = parse_value(right, min, max, label)?;
        if start > end {
            return Err(format!("{label} range '{}' is descending", range_expr));
        }
        (start, end)
    } else {
        let value = parse_value(range_expr, min, max, label)?;
        (value, value)
    };

    let mut value = start;
    while value <= end {
        output.insert(value);
        value = value.saturating_add(step);
        if value == u32::MAX {
            break;
        }
    }
    Ok(())
}

fn parse_value(raw: &str, min: u32, max: u32, label: &str) -> Result<u32, String> {
    let parsed = raw
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("{label} value '{}' is invalid", raw.trim()))?;
    if parsed < min || parsed > max {
        return Err(format!(
            "{label} value {} is out of range ({}-{})",
            parsed, min, max
        ));
    }
    Ok(parsed)
}

fn normalize_days_of_week(input: BTreeSet<u32>) -> BTreeSet<u32> {
    input
        .into_iter()
        .map(|value| if value == 7 { 0 } else { value })
        .collect()
}

fn round_up_to_next_minute(value: DateTime<Utc>) -> DateTime<Utc> {
    let base = value
        .with_second(0)
        .and_then(|next| next.with_nanosecond(0))
        .unwrap_or(value);
    if base.timestamp() <= value.timestamp() {
        base + Duration::minutes(1)
    } else {
        base
    }
}

fn cron_matches(spec: &CronScheduleSpec, candidate: DateTime<Utc>) -> bool {
    let minute = candidate.minute();
    let hour = candidate.hour();
    let day_of_month = candidate.day();
    let month = candidate.month();
    let day_of_week = candidate.weekday().num_days_from_sunday();

    spec.minutes.contains(&minute)
        && spec.hours.contains(&hour)
        && spec.days_of_month.contains(&day_of_month)
        && spec.months.contains(&month)
        && spec.days_of_week.contains(&day_of_week)
}

#[cfg(test)]
mod tests {
    use super::{next_cron_run_epoch_seconds, parse_cron_expression};

    #[test]
    fn parse_accepts_wildcard_and_steps() {
        let spec = parse_cron_expression("*/15 * * * *").expect("cron should parse");
        assert!(spec.minutes.contains(&0));
        assert!(spec.minutes.contains(&15));
        assert!(spec.minutes.contains(&45));
        assert_eq!(spec.hours.len(), 24);
    }

    #[test]
    fn parse_rejects_invalid_field_count() {
        let error = parse_cron_expression("* * * *")
            .expect_err("expression with 4 fields should be rejected");
        assert!(error.contains("must have 5 fields"));
    }

    #[test]
    fn next_run_computes_expected_minute() {
        let spec = parse_cron_expression("*/10 * * * *").expect("cron should parse");
        let next = next_cron_run_epoch_seconds(&spec, "UTC", 1_700_000_001)
            .expect("next run should resolve");
        assert_eq!(next, 1_700_000_400);
    }

    #[test]
    fn next_run_rejects_unknown_timezone() {
        let spec = parse_cron_expression("* * * * *").expect("cron should parse");
        let error = next_cron_run_epoch_seconds(&spec, "America/Denver", 1_700_000_000)
            .expect_err("non-UTC timezone should be rejected");
        assert!(error.contains("unsupported cron timezone"));
    }
}
