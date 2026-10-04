//! Instance-scoped progress for committed presentation events.
use super::wire::{EventPage, Reply, Response, VERSION};
use crate::events::{Event, Kind};
use serde::{Deserialize, Serialize};

impl EventPage {
    pub fn validate(&self, instance: u64, tick: u64, after: u64, limit: u16) -> Result<(), String> {
        if !(1..=64).contains(&limit)
            || self.events.len() > usize::from(limit)
            || self.latest < after
            || self.next > self.latest
            || self.gap
                != self
                    .oldest
                    .is_some_and(|first| after.saturating_add(1) < first)
            || self
                .oldest
                .is_some_and(|first| first == 0 || first > self.latest)
            || (self.oldest.is_none() && (self.latest != 0 || !self.events.is_empty()))
            || (self.events.is_empty() && after < self.latest)
        {
            return Err("Invalid chamber event page bounds".into());
        }
        let mut serial = after.max(self.oldest.map_or(0, |first| first - 1));
        let mut previous_tick = 0;
        for event in &self.events {
            let expected = serial.checked_add(1).ok_or("Event serial exhausted")?;
            if event.instance != instance
                || event.actor.is_some_and(|a| a.instance != instance)
                || event.serial != expected
                || event.serial > self.latest
                || event.tick > tick
                || event.tick < previous_tick
                || !event.time.is_finite()
                || event.time < 0.
            {
                return Err("Invalid chamber event identity or continuity".into());
            }
            match &event.kind {
                Kind::Dialogue { text } if text.len() > 4096 => {
                    return Err("Chamber dialogue exceeds byte budget".into());
                }
                Kind::Damage { amount, .. }
                    if event.actor.is_none() || !(1..=1_000_000).contains(amount) =>
                {
                    return Err("Invalid chamber damage event".into());
                }
                Kind::Death | Kind::Respawn if event.actor.is_none() => {
                    return Err("Chamber lifecycle event has no actor life".into());
                }
                _ => {}
            }
            serial = event.serial;
            previous_tick = event.tick;
        }
        let next = self.events.last().map_or(after, |e| e.serial);
        if self.next != next {
            return Err("Chamber event continuation mismatch".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gap {
    pub first: u64,
    pub last: u64,
}
#[derive(Clone, Debug)]
pub struct Delivery {
    pub events: Vec<Event>,
    pub gap: Option<Gap>,
}

/// Progress only: holds no dialogue text, actor state, or authority commands.
#[derive(Clone, Debug)]
pub struct Cursor {
    instance: u64,
    after: u64,
    latest: u64,
    tick: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema: String,
    instance: u64,
    after: u64,
    latest: u64,
    tick: u64,
}
impl Cursor {
    pub fn new(instance: u64) -> Self {
        Self {
            instance,
            after: 0,
            latest: 0,
            tick: 0,
        }
    }
    pub fn instance(&self) -> u64 {
        self.instance
    }
    pub fn after(&self) -> u64 {
        self.after
    }
    /// Admits a response to the specified request cursor and page limit.
    pub fn admit(
        &mut self,
        response: &Response,
        requested_after: u64,
        limit: u16,
    ) -> Result<Delivery, String> {
        if response.version != VERSION
            || response.instance != self.instance
            || response.request_id == 0
            || response.tick < self.tick
            || requested_after > self.after
        {
            return Err("Event cursor response context mismatch".into());
        }
        let Reply::Events { page } = &response.body else {
            return Err("Event cursor requires an event page".into());
        };
        page.validate(self.instance, response.tick, requested_after, limit)?;
        if page.latest < self.latest {
            return Err("Committed event history regressed".into());
        }
        let gap = page.oldest.and_then(|oldest| {
            let first = self.after.checked_add(1)?;
            let last = oldest.saturating_sub(1);
            (first <= last).then_some(Gap { first, last })
        });
        let events = page
            .events
            .iter()
            .filter(|e| e.serial > self.after)
            .cloned()
            .collect();
        self.after = self.after.max(page.next);
        self.latest = page.latest;
        self.tick = response.tick;
        Ok(Delivery { events, gap })
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&Saved {
            schema: "verse.events.cursor.v1".into(),
            instance: self.instance,
            after: self.after,
            latest: self.latest,
            tick: self.tick,
        })
        .map_err(|_| "Cannot encode event cursor".into())
    }
    pub fn restore(bytes: &[u8], instance: u64) -> Result<Self, String> {
        if bytes.len() > 1024 {
            return Err("Event cursor checkpoint exceeds byte budget".into());
        }
        let saved: Saved =
            serde_json::from_slice(bytes).map_err(|_| "Malformed event cursor checkpoint")?;
        if saved.schema != "verse.events.cursor.v1"
            || saved.instance != instance
            || saved.after > saved.latest
        {
            return Err("Event cursor checkpoint context mismatch".into());
        }
        Ok(Self {
            instance,
            after: saved.after,
            latest: saved.latest,
            tick: saved.tick,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verse_engine::core::LifeId;
    fn response(after: u64, oldest: u64, latest: u64, limit: usize) -> Response {
        let first = (after + 1).max(oldest);
        let events: Vec<_> = (first..=latest)
            .take(limit)
            .map(|serial| Event {
                instance: 140,
                serial,
                tick: 1,
                time: 1.,
                actor: Some(LifeId {
                    instance: 140,
                    actor: 3,
                    generation: 0,
                }),
                kind: Kind::Damage {
                    amount: 45,
                    incoming: false,
                },
            })
            .collect();
        let next = events.last().map_or(after, |e| e.serial);
        Response {
            version: VERSION,
            request_id: 1,
            instance: 140,
            tick: 1,
            control: None,
            body: Reply::Events {
                page: EventPage {
                    events,
                    next,
                    latest,
                    oldest: Some(oldest),
                    gap: after + 1 < oldest,
                },
            },
        }
    }
    #[test]
    fn partial_duplicate_and_overlapping_pages_deliver_each_serial_once() {
        let mut cursor = Cursor::new(140);
        let first = response(0, 1, 5, 2);
        assert_eq!(cursor.admit(&first, 0, 2).unwrap().events.len(), 2);
        assert!(cursor.admit(&first, 0, 2).unwrap().events.is_empty());
        let overlap = response(1, 1, 5, 3);
        let received = cursor.admit(&overlap, 1, 3).unwrap();
        assert_eq!(
            received.events.iter().map(|e| e.serial).collect::<Vec<_>>(),
            vec![3, 4]
        );
        assert_eq!(cursor.after(), 4);
        assert_eq!(
            cursor.admit(&response(4, 1, 5, 1), 4, 1).unwrap().events[0].serial,
            5
        );
    }
    #[test]
    fn retention_gap_is_reported_once_and_reconnect_continues_from_progress() {
        let mut cursor = Cursor::new(140);
        let reply = response(0, 7, 10, 1);
        let received = cursor.admit(&reply, 0, 1).unwrap();
        assert_eq!(received.gap, Some(Gap { first: 1, last: 6 }));
        assert_eq!(cursor.after(), 7);
        assert!(cursor.admit(&reply, 0, 1).unwrap().gap.is_none());
        let mut restored = Cursor::restore(&cursor.checkpoint().unwrap(), 140).unwrap();
        let received = restored.admit(&response(7, 7, 10, 3), 7, 3).unwrap();
        assert_eq!(
            received.events.iter().map(|e| e.serial).collect::<Vec<_>>(),
            vec![8, 9, 10]
        );
        assert!(Cursor::restore(&cursor.checkpoint().unwrap(), 141).is_err());
    }
    #[test]
    fn malformed_contexts_lives_and_payloads_leave_progress_unchanged() {
        let mut cursor = Cursor::new(140);
        let original = cursor.checkpoint().unwrap();
        for field in 0..6 {
            let mut bad = response(0, 1, 2, 2);
            if field == 0 {
                bad.instance += 1;
            }
            if field == 1 {
                bad.version += 1;
            }
            let Reply::Events { page } = &mut bad.body else {
                panic!()
            };
            match field {
                2 => page.events[1].serial += 1,
                3 => page.events[0].actor.as_mut().unwrap().instance += 1,
                4 => {
                    page.events[0].kind = Kind::Damage {
                        amount: -45,
                        incoming: false,
                    }
                }
                5 => {
                    page.events[0].kind = Kind::Dialogue {
                        text: "x".repeat(4097),
                    }
                }
                _ => {}
            }
            assert!(cursor.admit(&bad, 0, 2).is_err());
            assert_eq!(original, cursor.checkpoint().unwrap());
        }
    }
    #[test]
    fn history_and_tick_regression_or_skipping_unseen_events_are_refused() {
        let mut cursor = Cursor::new(140);
        cursor.admit(&response(0, 1, 4, 2), 0, 2).unwrap();
        let saved = cursor.checkpoint().unwrap();
        assert!(cursor.admit(&response(2, 1, 3, 1), 2, 1).is_err());
        let mut old_tick = response(2, 1, 4, 2);
        old_tick.tick = 0;
        assert!(cursor.admit(&old_tick, 2, 2).is_err());
        assert!(cursor.admit(&response(3, 1, 4, 1), 3, 1).is_err());
        assert_eq!(saved, cursor.checkpoint().unwrap());
        assert!(Cursor::restore(&vec![b' '; 1025], 140).is_err());
    }
}
