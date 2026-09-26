---
id: alerting.hysteresis-and-debounce
version: 1
kind: method
title: Hysteresis and debounce in alert state machines
summary: >-
  An alert that doesn't flicker needs state kept across evaluations:
  hysteresis uses a higher threshold to raise than to clear, and debounce
  requires several consecutive readings before the state changes, resetting
  the count whenever a reading goes the other way.
tags: [alerting, hysteresis, debounce, state-machine, thresholds, flapping, monitoring, consecutive]
applies_when: >-
  Code raises and clears alerts from a stream of scores or windows, or the
  task mentions flapping, flickering, debouncing, cooldowns, or consecutive
  windows.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Horowitz and Hill, The Art of Electronics, 3rd edition (Cambridge, 2015), the Schmitt trigger"
    - "Beyer, Murphy, Rensin, Kawahara, and Thorne (editors), The Site Reliability Workbook (O'Reilly, 2018), chapter 5, Alerting on SLOs"
    - "Prometheus documentation, Alerting rules: the for clause"
evidence: []
---

## Details

**Hysteresis** (a Schmitt trigger): two thresholds, `raise_at > clear_at`.
When clear, raise only when the score reaches `raise_at`; when raised, clear
only when the score falls below `clear_at`. Between them, the state stays
as it was. One threshold for both directions flickers when the score hovers
near it.

**Debounce**: require `k` consecutive readings past the threshold before
changing state. Keep one counter for the pending change:

- A reading that supports the change increments it; the state changes when
  it reaches `k` (on the k-th reading, not the k+1-th), and the counter
  resets.
- A reading that doesn't support the change resets it to 0. Counting total
  rather than consecutive readings is a common defect.
- Clearing is debounced the same way when the task asks for it; clearing on
  the first quiet reading makes the alert flicker.

Other rules:

- The state and counters persist between calls, on an object or in stored
  state. Recomputing them from each window alone loses the history.
- Decide `>=` versus `>` once and use it consistently for both thresholds.
- A cooldown after an alert suppresses new alerts for a set number of
  readings; count it down every reading, not only on alerting ones.
- A `nan` score should neither raise nor clear; comparisons with `nan` are
  false, which silently counts as "below threshold".

## How to check

Feed hand-written score sequences and assert the state after each reading: a
score that alternates around one threshold never raises; `k - 1` high
readings then a low one don't raise; `k` in a row do; a raised alert stays
raised between the two thresholds.
