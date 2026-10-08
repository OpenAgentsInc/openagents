//! Usage-limit text as Claude Code and Codex print it: whether error text
//! says a usage or rate limit stopped a session, and the reset time it
//! states. Moved from `coder_delegate::limit`, which re-exports it, so the
//! computer's own status check reads the same words the delegate does.

/// Phrases a provider's limit message uses, lowercased. Each is matched
/// only against error text: a failed result, an error event, or standard
/// error, never against what a model wrote.
const PHRASES: &[&str] = &[
    "hit your session limit",
    "hit your usage limit",
    "hit your weekly limit",
    "hit your limit",
    "usage limit",
    "rate_limit_error",
    "rate limit reached",
    "rate_limit_exceeded",
    "usage_limit_reached",
    "usage_limit_exceeded",
    "exceeded retry limit, last status: 429",
    "429 too many requests",
];

/// Whether error text says a usage or rate limit stopped the session.
#[must_use]
pub fn says_limited(text: &str) -> bool {
    let lower = text.to_lowercase();
    PHRASES.iter().any(|phrase| lower.contains(phrase))
}

/// Reads an ISO 8601 UTC timestamp such as `2026-09-23T11:30:58.118Z` as
/// seconds since the epoch.
#[must_use]
pub fn parse_iso(text: &str) -> Option<u64> {
    let text = text.trim();
    let (date, time) = text.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    let clock = time.get(..8)?;
    let mut clock_parts = clock.split(':');
    let hour: u64 = clock_parts.next()?.parse().ok()?;
    let minute: u64 = clock_parts.next()?.parse().ok()?;
    let second: u64 = clock_parts.next()?.parse().ok()?;
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    u64::try_from(era * 146_097 + day_of_era - 719_468).ok()
}

/// The year a day count since the epoch falls in.
fn year_of(days: u64) -> i64 {
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    year_of_era + era * 400 + i64::from(month <= 2)
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// The reset time a limit message states, read against `now` (seconds
/// since the epoch).
///
/// Older Claude Code builds print `Claude AI usage limit reached|<epoch>`,
/// the reset as Unix seconds after the bar. Claude Code also says `resets 11:50am (UTC)` or `resets Sep 24, 5pm (UTC)`;
/// Codex says `Try again at 3:04 PM` or `try again at Sep 24th, 2026 3:04
/// PM`. A time without a date is the next such time after `now`. A time
/// in a named zone other than UTC is not read, since this crate carries
/// no zone database; a time with no zone is read as UTC, the clock of the
/// task containers the CLIs run in.
#[must_use]
pub fn reset_from_message(message: &str, now: u64) -> Option<u64> {
    let lower = message.to_lowercase();
    if let Some(at) = lower.find("limit reached|") {
        let digits: String = lower[at + "limit reached|".len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        return digits.parse().ok().filter(|reset: &u64| *reset > 0);
    }
    let start = ["resets at ", "resets ", "try again at "]
        .iter()
        .find_map(|marker| lower.find(marker).map(|at| at + marker.len()))?;
    let rest = &lower[start..];
    // The zone, when the message names one in parentheses.
    let (spec, zone) = match rest.find('(') {
        Some(open) => {
            let close = rest[open..].find(')').map_or(rest.len(), |at| open + at);
            (&rest[..open], Some(rest[open + 1..close].trim()))
        }
        None => (rest, None),
    };
    if let Some(zone) = zone
        && !matches!(zone, "utc" | "gmt" | "etc/utc" | "z" | "etc/gmt")
    {
        return None;
    }
    // Words up to the end of the sentence.
    let spec = spec
        .split(['\n', '·'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.');
    let words: Vec<&str> = spec
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    let mut month = None;
    let mut day = None;
    let mut year = None;
    let mut clock = None;
    let mut index = 0;
    while index < words.len() {
        let word = words[index].trim_end_matches('.');
        if let Some(m) = MONTHS.iter().position(|m| word.starts_with(m)) {
            month = Some(u32::try_from(m).unwrap_or(0) + 1);
        } else if let Some(minutes) = clock_of(word, words.get(index + 1).copied()) {
            clock = Some(minutes);
            break;
        } else {
            let digits: String = word.chars().take_while(char::is_ascii_digit).collect();
            let rest = &word[digits.len()..];
            if !digits.is_empty() && matches!(rest, "" | "st" | "nd" | "rd" | "th") {
                let n: i64 = digits.parse().ok()?;
                if n >= 1_000 {
                    year = Some(n);
                } else if month.is_some() && day.is_none() {
                    day = Some(u32::try_from(n).ok()?);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        index += 1;
    }
    let minutes = clock?;
    let seconds_of_day = minutes * 60;
    let today = now / 86_400;
    match (month, day) {
        (Some(month), Some(day)) => {
            let this_year = year.unwrap_or_else(|| year_of(today));
            let at = days_from_civil(this_year, month, day)? * 86_400 + seconds_of_day;
            // A date without a year that has already passed is next year's.
            if year.is_none() && at + 86_400 < now {
                Some(days_from_civil(this_year + 1, month, day)? * 86_400 + seconds_of_day)
            } else {
                Some(at)
            }
        }
        (None, None) => {
            let at = today * 86_400 + seconds_of_day;
            Some(if at <= now { at + 86_400 } else { at })
        }
        _ => None,
    }
}

/// Reads a clock time such as `11:50am`, `5pm`, `3:04 pm`, or `15:04`
/// from `word` and, for a separate `am` or `pm`, `next`. Returns minutes
/// past midnight.
fn clock_of(word: &str, next: Option<&str>) -> Option<u64> {
    let (body, suffix) = if let Some(body) = word.strip_suffix("am") {
        (body, Some(false))
    } else if let Some(body) = word.strip_suffix("pm") {
        (body, Some(true))
    } else {
        match next.map(|n| n.trim_end_matches('.')) {
            Some("am") => (word, Some(false)),
            Some("pm") => (word, Some(true)),
            _ => (word, None),
        }
    };
    let (hour, minute) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?),
        None if suffix.is_some() => (body.parse::<u64>().ok()?, 0),
        None => return None,
    };
    if minute > 59 {
        return None;
    }
    let hour = match suffix {
        Some(pm) => {
            if !(1..=12).contains(&hour) {
                return None;
            }
            (hour % 12) + if pm { 12 } else { 0 }
        }
        None if hour <= 23 => hour,
        None => return None,
    };
    Some(hour * 60 + minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-09-23T11:30:58Z, when the retained sessions were throttled.
    const NOW: u64 = 1_790_163_058;
    // 2026-09-23T11:50:00Z, the `resetsAt` their streams carried.
    const RESET: u64 = 1_790_164_200;

    #[test]
    fn reads_iso_timestamps() {
        assert_eq!(parse_iso("2026-09-23T11:30:58.118Z"), Some(NOW));
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("not a time"), None);
    }

    #[test]
    fn reads_claude_codes_reset_time() {
        let said = "You've hit your session limit · resets 11:50am (UTC)";
        assert!(says_limited(said));
        assert_eq!(reset_from_message(said, NOW), Some(RESET));
        // After the reset time the same words mean tomorrow.
        assert_eq!(reset_from_message(said, RESET + 60), Some(RESET + 86_400));
        let dated = "You've hit your weekly limit · resets Sep 24, 5pm (UTC)";
        assert_eq!(
            reset_from_message(dated, NOW),
            parse_iso("2026-09-24T17:00:00Z")
        );
        // A zone this crate can't read leaves the reset unknown.
        let zoned = "You've hit your limit · resets 5pm (America/New_York)";
        assert!(says_limited(zoned));
        assert_eq!(reset_from_message(zoned, NOW), None);
    }

    #[test]
    fn reads_the_epoch_after_claude_codes_bar() {
        let said = "Claude AI usage limit reached|1790164200";
        assert!(says_limited(said));
        assert_eq!(reset_from_message(said, NOW), Some(RESET));
        assert_eq!(
            reset_from_message("Claude AI usage limit reached|", NOW),
            None
        );
    }

    #[test]
    fn reads_codexs_reset_time() {
        let today = "You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 3:04 PM.";
        assert!(says_limited(today));
        assert_eq!(
            reset_from_message(today, NOW),
            parse_iso("2026-09-23T15:04:00Z")
        );
        let dated = "You've hit your usage limit. Try again at Sep 25th, 2026 9:15 AM.";
        assert_eq!(
            reset_from_message(dated, NOW),
            parse_iso("2026-09-25T09:15:00Z")
        );
        assert_eq!(
            reset_from_message("You've hit your usage limit.", NOW),
            None
        );
    }

    #[test]
    fn ordinary_errors_are_not_limits() {
        assert!(!says_limited("API Error: 400 invalid_request_error"));
        assert!(!says_limited("the turn failed"));
        assert!(says_limited(
            "exceeded retry limit, last status: 429 Too Many Requests"
        ));
    }
}
