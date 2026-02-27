use super::{
    Bound, EventId, NegentropyMessage, Range, RangePayload, Record, Result, TIMESTAMP_INFINITY,
    calculate_fingerprint, sort_records,
};

/// State for tracking reconciliation progress
#[derive(Debug, Clone)]
#[allow(dead_code)] // Will be used in process_message() implementation
pub struct ReconciliationState {
    /// Local event records (sorted)
    pub records: Vec<Record>,
    /// IDs we have that remote needs
    pub have: Vec<EventId>,
    /// IDs remote has that we need
    pub need: Vec<EventId>,
    /// Current position in records for range splitting
    pub position: usize,
}

impl ReconciliationState {
    /// Create new reconciliation state with sorted records
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn new(mut records: Vec<Record>) -> Self {
        sort_records(&mut records);
        Self {
            records,
            have: Vec::new(),
            need: Vec::new(),
            position: 0,
        }
    }

    /// Find records within a bound range
    ///
    /// Returns indices of records where lower_bound <= record < upper_bound
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn find_records_in_range(&self, lower: &Bound, upper: &Bound) -> Vec<usize> {
        let mut indices = Vec::new();

        for (i, record) in self.records.iter().enumerate() {
            // Check if record >= lower_bound
            if record.timestamp < lower.timestamp {
                continue;
            }
            if record.timestamp == lower.timestamp && !lower.id_prefix.is_empty() {
                // Compare ID prefix
                let prefix_len = lower.id_prefix.len().min(32);
                if &record.id[..prefix_len] < lower.id_prefix.as_slice() {
                    continue;
                }
            }

            // Check if record < upper_bound (exclusive)
            if upper.timestamp != TIMESTAMP_INFINITY {
                if record.timestamp > upper.timestamp {
                    break; // Records are sorted, can stop early
                }
                if record.timestamp == upper.timestamp {
                    if upper.id_prefix.is_empty() {
                        // No ID prefix means exclude all records at this timestamp
                        break;
                    } else {
                        // Compare ID prefix
                        let prefix_len = upper.id_prefix.len().min(32);
                        if &record.id[..prefix_len] >= upper.id_prefix.as_slice() {
                            break;
                        }
                    }
                }
            }

            indices.push(i);
        }

        indices
    }

    /// Calculate fingerprint for records in a range
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn calculate_range_fingerprint(&self, lower: &Bound, upper: &Bound) -> [u8; 16] {
        let indices = self.find_records_in_range(lower, upper);
        let ids: Vec<EventId> = indices.iter().map(|&i| self.records[i].id).collect();
        calculate_fingerprint(&ids)
    }

    /// Split a range into smaller sub-ranges
    ///
    /// Divides the range to isolate differences. Strategy:
    /// - If range has 0 records: return empty (skip range)
    /// - If range has 1 record: return ID list
    /// - If range has multiple: split at midpoint
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn split_range(&self, lower: &Bound, upper: &Bound) -> Result<Vec<Range>> {
        let indices = self.find_records_in_range(lower, upper);

        if indices.is_empty() {
            // No local records in this range - skip it
            return Ok(vec![Range::skip(upper.clone())]);
        }

        if indices.len() == 1 {
            // Single record - send as ID list
            let id = self.records[indices[0]].id;
            return Ok(vec![Range::id_list(upper.clone(), vec![id])]);
        }

        // Multiple records - split at midpoint
        let mid_idx = indices[indices.len() / 2];
        let mid_record = &self.records[mid_idx];

        // Create midpoint bound
        let mid_bound = Bound::new(mid_record.timestamp, mid_record.id[..8].to_vec())?;

        // Calculate fingerprints for each half
        let fp_lower = self.calculate_range_fingerprint(lower, &mid_bound);
        let fp_upper = self.calculate_range_fingerprint(&mid_bound, upper);

        Ok(vec![
            Range::fingerprint(mid_bound, fp_lower),
            Range::fingerprint(upper.clone(), fp_upper),
        ])
    }

    /// Add an ID to the "have" set (we have, remote needs)
    #[allow(dead_code)] // Will be used in process_message() implementation
    pub fn add_have(&mut self, id: EventId) {
        if !self.have.contains(&id) {
            self.have.push(id);
        }
    }

    /// Add an ID to the "need" set (remote has, we need)
    #[allow(dead_code)] // Will be called by process_message()
    pub fn add_need(&mut self, id: EventId) {
        if !self.need.contains(&id) {
            self.need.push(id);
        }
    }

    /// Process an incoming Negentropy message and generate response
    ///
    /// This is the core reconciliation algorithm. For each incoming range:
    /// 1. If it's a Skip - no remote records in this range
    /// 2. If it's Fingerprint - compare with local fingerprint:
    ///    - If match: skip range (both sides agree)
    ///    - If mismatch: split range to isolate differences
    /// 3. If it's IdList - compare with local IDs:
    ///    - Add IDs we have but they don't to "have" set
    ///    - Add IDs they have but we don't to "need" set
    ///
    /// Returns a response message with our ranges
    #[allow(dead_code)] // Will be used in relay/client integration
    pub fn process_message(&mut self, incoming: &NegentropyMessage) -> Result<NegentropyMessage> {
        let mut response_ranges = Vec::new();
        let mut prev_bound = Bound::zero();

        for incoming_range in &incoming.ranges {
            let upper = &incoming_range.upper_bound;

            match &incoming_range.payload {
                RangePayload::Skip => {
                    // Remote has no records in [prev_bound, upper)
                    // Send all our records in this range
                    let our_ranges = self.split_range(&prev_bound, upper)?;
                    response_ranges.extend(our_ranges);
                }

                RangePayload::Fingerprint(remote_fp) => {
                    // Compare fingerprints
                    let local_fp = self.calculate_range_fingerprint(&prev_bound, upper);

                    if &local_fp == remote_fp {
                        // Fingerprints match - both sides agree on this range
                        response_ranges.push(Range::skip(upper.clone()));
                    } else {
                        // Fingerprints differ - split to isolate differences
                        let our_ranges = self.split_range(&prev_bound, upper)?;
                        response_ranges.extend(our_ranges);
                    }
                }

                RangePayload::IdList(remote_ids) => {
                    // Get our IDs in this range
                    let indices = self.find_records_in_range(&prev_bound, upper);
                    let local_ids: Vec<EventId> =
                        indices.iter().map(|&i| self.records[i].id).collect();

                    // Find IDs we have that they don't
                    for local_id in &local_ids {
                        if !remote_ids.contains(local_id) {
                            self.add_have(*local_id);
                        }
                    }

                    // Find IDs they have that we don't
                    for remote_id in remote_ids {
                        if !local_ids.contains(remote_id) {
                            self.add_need(*remote_id);
                        }
                    }

                    // Respond with our IDs for this range
                    if local_ids.is_empty() {
                        response_ranges.push(Range::skip(upper.clone()));
                    } else {
                        response_ranges.push(Range::id_list(upper.clone(), local_ids));
                    }
                }
            }

            prev_bound = upper.clone();
        }

        // Ensure we cover the full range up to infinity
        if prev_bound.timestamp != TIMESTAMP_INFINITY {
            let remaining_ranges = self.split_range(&prev_bound, &Bound::infinity())?;
            response_ranges.extend(remaining_ranges);
        }

        Ok(NegentropyMessage::new(response_ranges))
    }

    /// Check if reconciliation is complete
    ///
    /// Reconciliation is complete when both sides have exchanged ID lists
    /// for all ranges (no more fingerprints to compare)
    #[allow(dead_code)] // Will be used in relay/client integration
    pub fn is_complete(&self, last_message: &NegentropyMessage) -> bool {
        // If all ranges are Skip or IdList (no Fingerprint), we're done
        last_message
            .ranges
            .iter()
            .all(|r| matches!(r.payload, RangePayload::Skip | RangePayload::IdList(_)))
    }
}
