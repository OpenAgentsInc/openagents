//! The Calendar tools: `calendar.list_events` and
//! `calendar.find_free_time` read (policy allow; free time is a `POST` to
//! Calendar's free/busy query, which only reads); and
//! `calendar.create_event`, which always asks (policy require-approval):
//! the host runs it only after the person confirmed the exact event
//! [`card`] showed them.
//!
//! Days are the person's: "today", "tomorrow", and "week" are read in
//! their calendar's time zone (Calendar's setting), with the time zone
//! database built in.

use std::fmt::Write as _;

use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use serde::Deserialize;
use serde_json::{Value, json};

use super::rest::{ApiError, Cite, Rest, valid_address};
use super::{CALENDAR_EVENTS, CALENDAR_FULL, CALENDAR_READONLY, Endpoints};
use crate::core::{HttpMethod, Policy, ToolSpec};

/// The most events one listing returns.
pub const MAX_EVENTS: u32 = 100;
/// The most days one listing or search for free time covers.
pub const MAX_DAYS: i32 = 31;
/// The most free slots one search returns.
pub const MAX_SLOTS: usize = 30;

/// Calendar as one connection.
#[derive(Clone, Debug)]
pub struct Calendar {
    rest: Rest,
    root: String,
}

/// A span of days asked for: `today`, `tomorrow`, `week` (seven days from
/// today), or from one date to another (both included).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Range {
    #[serde(default)]
    pub range: Option<String>,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub start: Option<String>,
    /// `YYYY-MM-DD`, included.
    #[serde(default)]
    pub end: Option<String>,
}

/// The days `range` names, as the first day and the day after the last,
/// in `tz` on `today`.
///
/// # Errors
///
/// A sentence for the model.
pub fn days(range: &Range, today: Date) -> Result<(Date, Date), String> {
    let parse = |text: &str| {
        text.trim()
            .parse::<Date>()
            .map_err(|_| format!("{text} isn't a date like 2026-10-15."))
    };
    let after = |date: Date, n: i32| {
        date.checked_add(n.days())
            .map_err(|_| "That date is out of range.".to_owned())
    };
    let (first, end) = match (range.range.as_deref(), &range.start) {
        (_, Some(start)) => {
            let first = parse(start)?;
            let last = match &range.end {
                Some(end) => parse(end)?,
                None => first,
            };
            (first, after(last, 1)?)
        }
        (Some("today") | None, None) => (today, after(today, 1)?),
        (Some("tomorrow"), None) => (after(today, 1)?, after(today, 2)?),
        (Some("week"), None) => (today, after(today, 7)?),
        (Some(other), None) => {
            return Err(format!(
                "range is today, tomorrow, or week, not {other}; or give start and end dates."
            ));
        }
    };
    let span = first
        .until(end)
        .map_err(|_| "That date is out of range.".to_owned())?
        .get_days();
    if span <= 0 || span > MAX_DAYS {
        return Err(format!(
            "Give an end on or after the start, at most {MAX_DAYS} days."
        ));
    }
    Ok((first, end))
}

/// Now, as a model is told it (UTC).
#[must_use]
pub fn now_text() -> String {
    Timestamp::now().strftime("%a %Y-%m-%d %H:%M").to_string()
}

/// The start of `date` in `tz`.
fn midnight(date: Date, tz: &TimeZone) -> Result<Zoned, String> {
    date.to_zoned(tz.clone())
        .map_err(|_| "That day doesn't exist in your time zone.".to_owned())
}

/// One event as a listing shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    pub location: String,
    pub link: String,
    pub attendees: usize,
    pub status: String,
}

impl Event {
    fn of(value: &Value) -> Self {
        let when = |field: &str| {
            value[field]["dateTime"]
                .as_str()
                .or_else(|| value[field]["date"].as_str())
                .unwrap_or_default()
                .to_owned()
        };
        Self {
            id: value["id"].as_str().unwrap_or_default().to_owned(),
            title: value["summary"].as_str().unwrap_or("(no title)").to_owned(),
            start: when("start"),
            end: when("end"),
            all_day: value["start"]["date"].is_string(),
            location: value["location"].as_str().unwrap_or_default().to_owned(),
            link: value["htmlLink"].as_str().unwrap_or_default().to_owned(),
            attendees: value["attendees"].as_array().map_or(0, Vec::len),
            status: value["status"].as_str().unwrap_or_default().to_owned(),
        }
    }

    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "title": self.title,
            "start": self.start,
            "end": self.end,
            "all_day": self.all_day,
            "location": self.location,
            "attendees": self.attendees,
            "link": self.link,
        })
    }

    #[must_use]
    pub fn cite(&self) -> Cite {
        Cite {
            title: self.title.clone(),
            link: self.link.clone(),
        }
    }
}

/// What `calendar.create_event` takes. Times are the person's local times
/// (`YYYY-MM-DDTHH:MM`) in their calendar's time zone.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewEvent {
    pub title: String,
    pub start: String,
    #[serde(default)]
    pub end: Option<String>,
    /// Instead of `end`; 60 when neither is given.
    #[serde(default)]
    pub minutes: Option<u32>,
    #[serde(default)]
    pub attendees: Vec<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A [`NewEvent`] checked, with its times read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub event: NewEvent,
    pub start: DateTime,
    pub end: DateTime,
    pub time_zone: String,
}

impl NewEvent {
    /// Checked against `time_zone`.
    ///
    /// # Errors
    ///
    /// A sentence for the model.
    pub fn plan(&self, time_zone: &str) -> Result<Planned, String> {
        let title = self.title.trim();
        if title.is_empty() || title.chars().count() > 300 || title.contains(['\r', '\n']) {
            return Err("The title is one line of 1 to 300 characters.".into());
        }
        let read = |text: &str| {
            text.trim()
                .parse::<DateTime>()
                .map_err(|_| format!("{text} isn't a local time like 2026-10-15T10:00."))
        };
        let start = read(&self.start)?;
        let end = match (&self.end, self.minutes) {
            (Some(end), _) => read(end)?,
            (None, minutes) => start
                .checked_add(i64::from(minutes.unwrap_or(60).clamp(5, 24 * 60)).minutes())
                .map_err(|_| "That time is out of range.".to_owned())?,
        };
        if end <= start || start.until(end).map_or(true, |s| s.get_days() > 14) {
            return Err("The end is after the start, within 14 days.".into());
        }
        if self.attendees.len() > 50 || !self.attendees.iter().all(|a| valid_address(a.trim())) {
            return Err("Each attendee is one plain email address (up to 50).".into());
        }
        if self
            .location
            .as_deref()
            .is_some_and(|l| l.chars().count() > 500)
            || self
                .description
                .as_deref()
                .is_some_and(|d| d.chars().count() > 5_000)
        {
            return Err("The location or description is too long.".into());
        }
        TimeZone::get(time_zone)
            .map_err(|_| "Your calendar's time zone isn't known.".to_owned())?;
        Ok(Planned {
            event: self.clone(),
            start,
            end,
            time_zone: time_zone.to_owned(),
        })
    }
}

/// What an event to be added says, as the person is shown it before it is
/// added. Adding it in a chat runs only when this exact text was shown.
#[must_use]
pub fn card(planned: &Planned) -> String {
    let event = &planned.event;
    let mut out = String::from("**New calendar event**\n\n");
    let _ = writeln!(out, "Title: {}  ", event.title.trim());
    let same_day = planned.start.date() == planned.end.date();
    let _ = writeln!(
        out,
        "When: {} to {} ({})  ",
        planned.start.strftime("%a %Y-%m-%d %H:%M"),
        if same_day {
            planned.end.strftime("%H:%M").to_string()
        } else {
            planned.end.strftime("%a %Y-%m-%d %H:%M").to_string()
        },
        planned.time_zone
    );
    if let Some(location) = event.location.as_deref().filter(|l| !l.trim().is_empty()) {
        let _ = writeln!(
            out,
            "Where: {}  ",
            location.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }
    if !event.attendees.is_empty() {
        let _ = writeln!(
            out,
            "Invites: {} (Google emails them when it's added)  ",
            event
                .attendees
                .iter()
                .map(|a| a.trim())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    out.trim_end().to_owned()
}

/// Free stretches of `minutes` or more, each working day from `day_start`
/// to `day_end` (local hours), not before `now`, around `busy`.
#[must_use]
pub fn free_slots(
    days: (Date, Date),
    tz: &TimeZone,
    now: Timestamp,
    busy: &[(Timestamp, Timestamp)],
    minutes: i64,
    hours: (i8, i8),
) -> Vec<(Zoned, Zoned)> {
    let mut out = Vec::new();
    let mut day = days.0;
    while day < days.1 && out.len() < MAX_SLOTS {
        let closing = if hours.1 >= 24 {
            day.tomorrow().map(|d| d.at(0, 0, 0, 0))
        } else {
            Ok(day.at(hours.1, 0, 0, 0))
        };
        let (Ok(open), Ok(close)) = (
            day.at(hours.0, 0, 0, 0).to_zoned(tz.clone()),
            closing.and_then(|c| c.to_zoned(tz.clone())),
        ) else {
            break;
        };
        let mut from = open.timestamp().max(now);
        let close = close.timestamp();
        let mut taken: Vec<&(Timestamp, Timestamp)> = busy
            .iter()
            .filter(|(start, end)| *end > from && *start < close)
            .collect();
        taken.sort_by_key(|(start, _)| *start);
        for (start, end) in taken
            .into_iter()
            .map(|(s, e)| (*s, *e))
            .chain([(close, close)])
        {
            let until = start.min(close);
            if until.as_second() - from.as_second() >= minutes * 60 {
                out.push((from.to_zoned(tz.clone()), until.to_zoned(tz.clone())));
            }
            from = from.max(end);
            if from >= close || out.len() >= MAX_SLOTS {
                break;
            }
        }
        let Ok(next) = day.tomorrow() else { break };
        day = next;
    }
    out
}

impl Calendar {
    #[must_use]
    pub fn new(http: reqwest::Client, endpoints: &Endpoints, token: String) -> Self {
        Self {
            rest: Rest::new(http, token),
            root: format!("{}/calendar/v3", endpoints.calendar.trim_end_matches('/')),
        }
    }

    /// The person's time zone, from their Calendar setting (UTC when it
    /// can't be read).
    pub async fn time_zone(&self) -> (String, TimeZone) {
        let named = self
            .rest
            .get(&format!("{}/users/me/settings/timezone", self.root), &[])
            .await
            .ok()
            .and_then(|v| v["value"].as_str().map(str::to_owned));
        if let Some(name) = named
            && let Ok(tz) = TimeZone::get(&name)
        {
            return (name, tz);
        }
        ("UTC".to_owned(), TimeZone::UTC)
    }

    async fn window(
        &self,
        range: &Range,
    ) -> Result<(String, TimeZone, Zoned, Zoned, (Date, Date)), ApiError> {
        let (name, tz) = self.time_zone().await;
        let today = Timestamp::now().to_zoned(tz.clone()).date();
        let span = days(range, today).map_err(ApiError::Invalid)?;
        let first = midnight(span.0, &tz).map_err(ApiError::Invalid)?;
        let end = midnight(span.1, &tz).map_err(ApiError::Invalid)?;
        Ok((name, tz, first, end, span))
    }

    /// The events on the person's main calendar in `range`, in order.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn events(&self, range: &Range) -> Result<(String, Vec<Event>), ApiError> {
        let (name, _, first, end, _) = self.window(range).await?;
        let (min, max) = (first.timestamp().to_string(), end.timestamp().to_string());
        let limit = MAX_EVENTS.to_string();
        let listed = self
            .rest
            .get(
                &format!("{}/calendars/primary/events", self.root),
                &[
                    ("timeMin", min.as_str()),
                    ("timeMax", max.as_str()),
                    ("singleEvents", "true"),
                    ("orderBy", "startTime"),
                    ("maxResults", limit.as_str()),
                    ("timeZone", name.as_str()),
                ],
            )
            .await?;
        let events = listed["items"]
            .as_array()
            .into_iter()
            .flatten()
            .map(Event::of)
            .filter(|e| e.status != "cancelled")
            .collect();
        Ok((name, events))
    }

    /// Free time in `range`: stretches of `minutes` or more within
    /// `hours` on each day.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn free(
        &self,
        range: &Range,
        minutes: i64,
        hours: (i8, i8),
    ) -> Result<(String, Vec<(Zoned, Zoned)>), ApiError> {
        let (name, tz, first, end, span) = self.window(range).await?;
        let answer = self
            .rest
            .post(
                &format!("{}/freeBusy", self.root),
                &[],
                &json!({
                    "timeMin": first.timestamp().to_string(),
                    "timeMax": end.timestamp().to_string(),
                    "timeZone": name,
                    "items": [{"id": "primary"}],
                }),
            )
            .await?;
        let busy: Vec<(Timestamp, Timestamp)> = answer["calendars"]["primary"]["busy"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| {
                Some((
                    b["start"].as_str()?.parse().ok()?,
                    b["end"].as_str()?.parse().ok()?,
                ))
            })
            .collect();
        Ok((
            name,
            free_slots(span, &tz, Timestamp::now(), &busy, minutes, hours),
        ))
    }

    /// Add `planned`. The host calls this only after the person confirmed
    /// it ([`card`]); invitations go out when it has attendees.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn create(&self, planned: &Planned) -> Result<Event, ApiError> {
        let at = |time: &DateTime| json!({"dateTime": time.strftime("%Y-%m-%dT%H:%M:%S").to_string(), "timeZone": planned.time_zone});
        let event = &planned.event;
        let mut body = json!({
            "summary": event.title.trim(),
            "start": at(&planned.start),
            "end": at(&planned.end),
        });
        if !event.attendees.is_empty() {
            body["attendees"] = json!(
                event
                    .attendees
                    .iter()
                    .map(|a| json!({"email": a.trim()}))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(location) = &event.location {
            body["location"] = json!(location);
        }
        if let Some(description) = &event.description {
            body["description"] = json!(description);
        }
        let updates = if event.attendees.is_empty() {
            "none"
        } else {
            "all"
        };
        let made = self
            .rest
            .post(
                &format!("{}/calendars/primary/events", self.root),
                &[("sendUpdates", updates)],
                &body,
            )
            .await?;
        Ok(Event::of(&made))
    }
}

fn scopes(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn range_properties() -> Value {
    json!({
        "range": {"type": "string", "enum": ["today", "tomorrow", "week"], "description": "Days in the person's time zone; week is seven days from today."},
        "start": {"type": "string", "description": "Instead of range: the first day, YYYY-MM-DD."},
        "end": {"type": "string", "description": "The last day, YYYY-MM-DD (included)."}
    })
}

/// The Calendar tools.
#[must_use]
pub fn tool_specs() -> Vec<ToolSpec> {
    let read = scopes(&[
        CALENDAR_READONLY,
        CALENDAR_EVENTS,
        CALENDAR_FULL,
        "https://www.googleapis.com/auth/calendar.events.readonly",
    ]);
    let mut free = range_properties();
    free["minutes"] = json!({"type": "integer", "description": "The shortest free stretch, in minutes (30 by default)."});
    free["day_start"] = json!({"type": "integer", "description": "The hour the day starts, 0 to 23 (9 by default)."});
    free["day_end"] = json!({"type": "integer", "description": "The hour the day ends, 1 to 24 (18 by default)."});
    vec![
        ToolSpec {
            id: "calendar.list_events".into(),
            description: "List the events on the person's Google Calendar for today, tomorrow, the week, or dates. Returns each event's title, start, end, place, attendee count, and link, in the person's time zone.".into(),
            method: HttpMethod::Get,
            input_schema: json!({"type": "object", "properties": range_properties(), "additionalProperties": false}),
            scopes: read.clone(),
            policy: None,
        },
        ToolSpec {
            id: "calendar.find_free_time".into(),
            description: "Find free time on the person's Google Calendar: stretches of at least the given minutes within working hours on each day of the range.".into(),
            method: HttpMethod::Post,
            input_schema: json!({"type": "object", "properties": free, "additionalProperties": false}),
            scopes: read,
            // A free/busy query only reads.
            policy: Some(Policy::Allow),
        },
        ToolSpec {
            id: "calendar.create_event".into(),
            description: "Propose an event on the person's Google Calendar. It is shown to them and added only after they say yes to it.".into(),
            method: HttpMethod::Post,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "start": {"type": "string", "description": "Local time, YYYY-MM-DDTHH:MM, in the person's time zone."},
                    "end": {"type": "string", "description": "Local time, YYYY-MM-DDTHH:MM."},
                    "minutes": {"type": "integer", "description": "Instead of end: how long (60 by default)."},
                    "attendees": {"type": "array", "items": {"type": "string"}, "description": "Email addresses to invite."},
                    "location": {"type": "string"},
                    "description": {"type": "string"}
                },
                "required": ["title", "start"],
                "additionalProperties": false
            }),
            scopes: scopes(&[CALENDAR_EVENTS, CALENDAR_FULL]),
            policy: Some(Policy::RequireApproval),
        },
    ]
}

/// One call of a Calendar tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    List(Range),
    Free {
        range: Range,
        minutes: i64,
        hours: (i8, i8),
    },
    Create(NewEvent),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FreeArgs {
    #[serde(default)]
    range: Option<String>,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    end: Option<String>,
    #[serde(default)]
    minutes: Option<i64>,
    #[serde(default)]
    day_start: Option<i8>,
    #[serde(default)]
    day_end: Option<i8>,
}

impl Call {
    /// Whether `name` is a Calendar tool (its id or wire name).
    #[must_use]
    pub fn names(name: &str) -> bool {
        name.starts_with("calendar.") || name.starts_with("calendar_")
    }

    /// The call `name` with `arguments`.
    ///
    /// # Errors
    ///
    /// A sentence saying what was wrong.
    pub fn parse(name: &str, mut arguments: Value) -> Result<Self, String> {
        let bad = |tool: &str| format!("{tool} takes the fields its schema declares.");
        let id = name
            .strip_prefix("calendar_")
            .map_or_else(|| name.to_owned(), |rest| format!("calendar.{rest}"));
        match id.as_str() {
            "calendar.list_events" => serde_json::from_value(arguments)
                .map(Self::List)
                .map_err(|_| bad("calendar.list_events")),
            "calendar.find_free_time" => {
                let args: FreeArgs = serde_json::from_value(arguments)
                    .map_err(|_| bad("calendar.find_free_time"))?;
                let hours = (args.day_start.unwrap_or(9), args.day_end.unwrap_or(18));
                if !(0..24).contains(&hours.0) || !(1..=24).contains(&hours.1) || hours.1 <= hours.0
                {
                    return Err("day_start and day_end are hours, the start before the end.".into());
                }
                Ok(Self::Free {
                    range: Range {
                        range: args.range,
                        start: args.start,
                        end: args.end,
                    },
                    minutes: args.minutes.unwrap_or(30).clamp(5, 12 * 60),
                    hours,
                })
            }
            "calendar.create_event" => {
                if let Some(text) = arguments["attendees"].as_str() {
                    let list: Vec<Value> = text
                        .split([',', ';'])
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| json!(s))
                        .collect();
                    arguments["attendees"] = Value::Array(list);
                }
                serde_json::from_value(arguments)
                    .map(Self::Create)
                    .map_err(|_| bad("calendar.create_event"))
            }
            _ => Err(format!("There is no Calendar tool named {name}.")),
        }
    }

    /// The tool's id.
    #[must_use]
    pub fn tool(&self) -> &'static str {
        match self {
            Self::List(_) => "calendar.list_events",
            Self::Free { .. } => "calendar.find_free_time",
            Self::Create(_) => "calendar.create_event",
        }
    }
}

/// What a call produced: the result, and the events it used (to link).
#[derive(Clone, Debug)]
pub struct Outcome {
    pub result: Value,
    pub cited: Vec<Cite>,
}

/// Run `call` on `calendar`. [`Call::Create`] adds the event: the host
/// runs it only after the person's confirm.
///
/// # Errors
///
/// [`ApiError`].
pub async fn run(calendar: &Calendar, call: &Call) -> Result<Outcome, ApiError> {
    match call {
        Call::List(range) => {
            let (zone, events) = calendar.events(range).await?;
            Ok(Outcome {
                result: json!({
                    "time_zone": zone,
                    "events": events.iter().map(Event::json).collect::<Vec<_>>(),
                }),
                cited: events
                    .iter()
                    .filter(|e| !e.link.is_empty())
                    .map(Event::cite)
                    .collect(),
            })
        }
        Call::Free {
            range,
            minutes,
            hours,
        } => {
            let (zone, slots) = calendar.free(range, *minutes, *hours).await?;
            Ok(Outcome {
                result: json!({
                    "time_zone": zone,
                    "free": slots.iter().map(|(from, to)| json!({
                        "start": from.strftime("%a %Y-%m-%d %H:%M").to_string(),
                        "end": to.strftime("%H:%M").to_string(),
                    })).collect::<Vec<_>>(),
                }),
                cited: Vec::new(),
            })
        }
        Call::Create(event) => {
            let (zone, _) = calendar.time_zone().await;
            let planned = event.plan(&zone).map_err(ApiError::Invalid)?;
            let made = calendar.create(&planned).await?;
            Ok(Outcome {
                result: json!({"added": true, "event": made.json()}),
                cited: vec![made.cite()],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_are_read_from_words_or_dates() {
        let today: Date = "2026-10-14".parse().unwrap();
        let d = |s: &str| s.parse::<Date>().unwrap();
        let word = |w: &str| Range {
            range: Some(w.into()),
            ..Range::default()
        };
        assert_eq!(days(&Range::default(), today), Ok((today, d("2026-10-15"))));
        assert_eq!(
            days(&word("tomorrow"), today),
            Ok((d("2026-10-15"), d("2026-10-16")))
        );
        assert_eq!(days(&word("week"), today), Ok((today, d("2026-10-21"))));
        let dates = Range {
            start: Some("2026-10-20".into()),
            end: Some("2026-10-22".into()),
            ..Range::default()
        };
        assert_eq!(days(&dates, today), Ok((d("2026-10-20"), d("2026-10-23"))));
        assert!(days(&word("year"), today).is_err());
        let long = Range {
            start: Some("2026-01-01".into()),
            end: Some("2026-12-01".into()),
            ..Range::default()
        };
        assert!(days(&long, today).is_err());
    }

    #[test]
    fn free_time_is_the_working_day_around_what_is_busy() {
        let tz = TimeZone::get("America/Chicago").unwrap();
        let day: Date = "2026-10-15".parse().unwrap();
        let at = |h: i8, m: i8| day.at(h, m, 0, 0).to_zoned(tz.clone()).unwrap().timestamp();
        let busy = [(at(10, 0), at(11, 30)), (at(13, 0), at(17, 45))];
        let slots = free_slots(
            (day, day.tomorrow().unwrap()),
            &tz,
            at(0, 0),
            &busy,
            30,
            (9, 18),
        );
        let shown: Vec<String> = slots
            .iter()
            .map(|(a, b)| format!("{}-{}", a.strftime("%H:%M"), b.strftime("%H:%M")))
            .collect();
        assert_eq!(shown, ["09:00-10:00", "11:30-13:00"]);
        // Nothing before now.
        let later = free_slots(
            (day, day.tomorrow().unwrap()),
            &tz,
            at(12, 0),
            &busy,
            30,
            (9, 18),
        );
        assert_eq!(later[0].0.strftime("%H:%M").to_string(), "12:00");
    }

    #[test]
    fn a_new_event_is_checked_and_its_card_shows_when_and_who() {
        let event = NewEvent {
            title: "Planning".into(),
            start: "2026-10-15T10:00".into(),
            end: None,
            minutes: Some(30),
            attendees: vec!["a@b.co".into()],
            location: None,
            description: None,
        };
        let planned = event.plan("America/Chicago").unwrap();
        let shown = card(&planned);
        assert!(
            shown.contains("When: Thu 2026-10-15 10:00 to 10:30 (America/Chicago)"),
            "{shown}"
        );
        assert!(shown.contains("Invites: a@b.co"));
        let mut bad = event.clone();
        bad.start = "tomorrow at ten".into();
        assert!(bad.plan("UTC").is_err());
        let mut bad = event;
        bad.end = Some("2026-10-15T09:00".into());
        assert!(bad.plan("UTC").is_err());
    }

    #[test]
    fn calls_parse_and_creating_always_asks() {
        assert!(matches!(
            Call::parse("calendar_list_events", json!({"range": "week"})),
            Ok(Call::List(_))
        ));
        assert!(
            Call::parse(
                "calendar_find_free_time",
                json!({"day_start": 18, "day_end": 9})
            )
            .is_err()
        );
        assert!(Call::parse("calendar_list_events", json!({"calendar": "x"})).is_err());
        let specs = tool_specs();
        let policy = |id: &str| specs.iter().find(|s| s.id == id).unwrap().default_policy();
        assert_eq!(policy("calendar.list_events"), Policy::Allow);
        assert_eq!(policy("calendar.find_free_time"), Policy::Allow);
        assert_eq!(policy("calendar.create_event"), Policy::RequireApproval);
    }
}
