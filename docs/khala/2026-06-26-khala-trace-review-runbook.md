# Khala Trace Review Runbook

Issue #6356 is implemented as an admin/operator review endpoint:

```sh
curl -fsS "https://openagents.com/api/operator/khala/trace-review?hours=24&limit=10" \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN"
```

The report reviews the last `hours` over:

- `agent_traces`
- `token_usage_events`
- `pylon_codex_raw_events`

It returns only aggregates and refs: model mix, demand-source buckets, outcome
buckets, owner-only trace refs, raw-event row refs, failure-mode refs, recurring
intent refs, and triage items. It does not return raw trajectories, raw Codex SDK
payloads, provider secrets, local paths, or user/customer content.

## Operating Loop

Run this once per Khala improvement cycle, or before starting a burndown batch:

1. Fetch the report for the last 24 hours.
2. Review `failureModes` first. Convert high/medium priority `triageItems` into
   GitHub issues when they are not already represented.
3. Review `userIntents` and `notableTraces` for repeated capability gaps. Feed
   unsupported requests into the #6357 ledger:
   `GET/POST /api/operator/khala/unsupported-requests`.
4. Use `modelMix`, `outcomes`, and `demandSources` to spot backend regressions,
   fallback drift, empty responses, estimated token rows, and traffic shifts.
5. Keep raw trace inspection owner-only through `/traces?token=...` or the
   admin database tools. Shared summaries should cite refs and aggregate counts
   only.

Example triage item shape:

```json
{
  "triageRef": "triage.khala_trace_review.empty_response",
  "kind": "bug",
  "priority": "medium",
  "title": "Token rows with zero completion/output tokens",
  "evidenceRefs": ["table.token_usage_events.output_tokens_zero"],
  "suggestedIssueTitle": "[Khala trace review] Token rows with zero completion/output tokens"
}
```

The route is intentionally not part of the public OpenAPI contract. It is an
operator/admin surface for turning owner-scoped traces into backlog items while
preserving the raw-data boundary.

The #6357 unsupported-request ledger runbook is
`docs/khala/2026-06-26-khala-unsupported-request-list.md`.
