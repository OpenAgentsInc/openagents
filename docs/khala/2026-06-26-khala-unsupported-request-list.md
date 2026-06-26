# Khala Unsupported-Request List Runbook

Issue #6357 is implemented as a Forum-first, operator-maintained list for what
testers try that Khala cannot do yet.

```sh
curl -fsS "https://openagents.com/api/operator/khala/unsupported-requests?status=needs_issue" \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN"
```

The list is admin-only and stores bounded summaries plus refs. Raw traces, raw
feedback, private paths, provider payloads, and credentials stay in their source
systems.

## Intake

Use the Product Promises Forum as the default public intake:

- Forum reports: `https://openagents.com/forum/f/product-promises`
- CLI/tester feedback: `POST /api/khala/feedback`
- Trace review: `GET /api/operator/khala/trace-review`
- Maintained list: `GET/POST /api/operator/khala/unsupported-requests`

Create or update a row after reviewing a Forum report, feedback ref, or
trace-review triage item:

```sh
curl -fsS "https://openagents.com/api/operator/khala/unsupported-requests" \
  -X POST \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  --data '{
    "sourceKind": "trace_review",
    "sourceRef": "triage.intent.khala_trace_review.khala_cli",
    "title": "CLI users ask Khala for an unsupported workflow",
    "summary": "Bounded public-safe summary only.",
    "triageKind": "missing_capability",
    "forumTopicRef": "forum.product-promises.example",
    "evidenceRefs": ["triage.intent.khala_trace_review.khala_cli"]
  }'
```

Rows with `triageKind: "bug"` or `"missing_capability"` default to
`status: "needs_issue"` until `githubIssueRef` is attached. Rows marked
`status: "issue_opened"` must include a GitHub issue ref.

## Cadence

Run this every Khala improvement cycle:

1. Fetch `/api/operator/khala/trace-review` and review `triageItems`.
2. Review `/api/operator/khala/feedback` for direct tester notes.
3. Link or create a Forum discussion first when the report is loose, broad, or
   not yet reproducible.
4. Upsert bounded rows into `/api/operator/khala/unsupported-requests`.
5. Open strict GitHub issues only for real bugs or missing capabilities with
   enough public-safe reproduction/evidence refs.
6. Attach `githubIssueRef` to the row and thread the issue back into the Khala
   roadmap.

## Triage Kinds

- `bug`: something Khala claims or should support is broken.
- `missing_capability`: a tester is trying a capability Khala does not support
  yet and the gap should enter the roadmap.
- `wont_do`: intentionally unsupported or out of product scope.
- `needs_triage`: collected but not classified yet.

The response includes `nextAction` for the operator loop:
`triage`, `link_forum_report`, `open_github_issue`, or `none`.
