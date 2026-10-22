//! Inclusive NIP-01 history pages with NIP-67 completeness hints.

use std::collections::BTreeSet;

const PAGE: usize = 500;
const MAX_PAGE: usize = 100_000;

pub(super) struct History {
    pub until: u64,
    pub limit: usize,
    oldest: u64,
    count: usize,
    boundary: BTreeSet<String>,
    page_boundary: BTreeSet<String>,
    expanded: bool,
    pub complete: Option<bool>,
}

impl History {
    pub fn new(until: u64) -> Self {
        Self {
            until,
            limit: PAGE,
            oldest: until,
            count: 0,
            boundary: BTreeSet::new(),
            page_boundary: BTreeSet::new(),
            expanded: false,
            complete: None,
        }
    }

    pub fn event(&mut self, at: u64, id: &str) {
        self.count += 1;
        if at < self.oldest {
            self.oldest = at;
            self.page_boundary.clear();
        }
        if at == self.oldest {
            self.page_boundary.insert(id.to_owned());
        }
    }

    /// Whether another page is needed. Never move past an unfinished boundary.
    pub fn end(&mut self) -> Result<bool, String> {
        if self.complete == Some(true) {
            return Ok(false);
        }
        if self.count == 0 {
            return if self.complete == Some(false) {
                Err("relay reports more suggestion history but returned no events".into())
            } else {
                Ok(false)
            };
        }
        // A legacy relay gives no definitive completeness information.
        if self.complete.is_none() && self.count < self.limit {
            return Ok(false);
        }
        if self.oldest < self.until {
            self.until = self.oldest;
            self.boundary = std::mem::take(&mut self.page_boundary);
            self.limit = PAGE;
            self.expanded = false;
        } else {
            let progressed = self
                .page_boundary
                .iter()
                .any(|id| !self.boundary.contains(id));
            if (self.expanded && !progressed) || self.limit >= MAX_PAGE {
                return Err(format!(
                    "suggestion history is truncated at timestamp {}; no records were skipped. \
                     Raise the relay's NOSTR_RELAY_MAX_LIMIT and \
                     NOSTR_RELAY_SEND_QUEUE_CAPACITY, then restart this service",
                    self.until
                ));
            }
            self.boundary.append(&mut self.page_boundary);
            self.limit = (self.limit * 2).min(MAX_PAGE);
            self.expanded = true;
        }
        self.oldest = self.until;
        self.count = 0;
        self.page_boundary.clear();
        self.complete = None;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay(rows: &[(u64, String)], cap: usize) -> Result<BTreeSet<String>, (String, History)> {
        let mut history = History::new(10_000);
        let mut received = BTreeSet::new();
        for _ in 0..100 {
            let matching: Vec<_> = rows.iter().filter(|(at, _)| *at <= history.until).collect();
            let limit = history.limit.min(cap);
            for (at, id) in matching.iter().take(limit) {
                history.event(*at, id);
                received.insert(id.clone());
            }
            history.complete = Some(matching.len() <= limit);
            match history.end() {
                Ok(false) => return Ok(received),
                Ok(true) => {}
                Err(error) => return Err((error, history)),
            }
        }
        panic!("history did not stop");
    }

    #[test]
    fn a_relay_cap_of_127_does_not_truncate_multiple_pages() {
        let rows: Vec<_> = (0..2_017)
            .map(|n| (9_000 - n, format!("{n:064x}")))
            .collect();
        let received = replay(&rows, 127).ok().expect("complete history");
        assert_eq!(received.len(), rows.len());
    }

    #[test]
    fn an_equal_timestamp_bucket_can_grow_without_skipping_older_events() {
        let mut rows: Vec<_> = (0..600).map(|n| (8_000, format!("{n:064x}"))).collect();
        rows.push((7_999, format!("{:064x}", 601)));
        let received = replay(&rows, 1_000).ok().expect("complete bucket");
        assert_eq!(received.len(), 601);
    }

    #[test]
    fn an_unpageable_timestamp_stops_visibly_without_advancing() {
        let rows: Vec<_> = (0..600).map(|n| (8_000, format!("{n:064x}"))).collect();
        let (error, history) = replay(&rows, 127).err().expect("visible failure");
        assert!(error.contains("no records were skipped"));
        assert!(error.contains("NOSTR_RELAY_SEND_QUEUE_CAPACITY"));
        assert_eq!(history.until, 8_000);
        assert_eq!(history.boundary.len(), 127);
    }

    #[test]
    fn more_without_any_events_is_an_error_and_finish_is_definitive() {
        let mut history = History::new(10_000);
        history.complete = Some(false);
        assert!(history.end().is_err());
        history.complete = Some(true);
        assert!(!history.end().unwrap());
    }
}
