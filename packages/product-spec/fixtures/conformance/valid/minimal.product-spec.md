---
spec_format_version: "0.1"
title: "YouTube Transcription Search"
artifact_type: "prd"
spec_revision: 1
author: "ProductSpec"
created_at: "2026-07-04T00:00:00Z"
updated_at: "2026-07-04T00:00:00Z"
---

## Problem

Researchers using YouTube videos as source material spend too much time finding exact quotes they can cite.

## Hypothesis

If searchable transcripts expose timestamped passages, researchers will cite video sources more often because evidence becomes easier to find and verify.

## Scope

In: paste a YouTube URL, generate a transcript, search it, jump to timestamps, and copy citations.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: Valid public YouTube URLs create transcript pages.
- id: AC-2
  criterion: Search returns timestamped transcript passages.
- id: AC-3
  criterion: Private or unsupported videos return clear errors.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: first_transcript_search_rate
  target: ">= 60%"
  window: first transcript session
```
