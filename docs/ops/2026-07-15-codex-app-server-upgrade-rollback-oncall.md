# Codex app-server upgrade, rollback, and degraded-mode runbook

## Release authority

OpenAgents Desktop has two independently reported protocol targets:

- `current-source` is the reviewed upstream wire/fixture target. It must report
  126 client requests, 1 client notification, 11 reverse requests, and 72
  notifications. Members newer than the packaged binary remain visibly
  `requires-binary-upgrade`. They are not callable through a generic escape
  hatch.
- `bundled-0.144.1` is the release runtime. Its exact executable hash, generated
  schema export, reviewed compatibility supplements, 125/1/11/69 manifest, and
  every family smoke must match before release.

The generated evidence is
[`docs/receipts/2026-07-15-codex-app-server-conformance.json`](../receipts/2026-07-15-codex-app-server-conformance.json).
Transport, handler disposition, native projection, product presentation,
authority, fixture, and compatible-real-binary percentages are separate. One
percentage cannot substitute for another.

## Upgrade

1. Update the exact Codex package and platform artifacts together.
2. Regenerate both protocol lanes and notification/member fixtures from the
   candidate source and binary. Review the drift. Never edit generated output
   to conceal it.
3. Add a typed handler, native projection, product surface or policy-owned
   unavailable disposition, authority class, fixture, and family smoke for
   every added member.
4. Update the experimental aggregate gate. Experimental opt-in must remain
   false until its generated method set and handler set are identical.
5. Run protocol, reverse-RPC, lifecycle, item/review, composer queue/steer,
   ecosystem, host, experimental, and binary-manifest suites. Exercise restart,
   unknown-message, overload, notification-gap, multi-window decision, and
   recovery-corruption falsifiers.
6. Regenerate the conformance receipt. Release only if the bundled target is
   100% in every column and the release evaluator has zero blockers.

## Degraded mode

`incompatible`, `degraded`, and `repairing` are product states, not retry
details. Stop new turn/process/realtime admission, retain stable intent and
queue identities, keep reverse requests exactly once, and show the state on
every local surface. Unknown or malformed messages create bounded compatibility
receipts and block release. They are never silently converted or projected as
success. Public diagnostics contain state, generation, method, reason, and
occurrence count only. Raw payloads, credentials, paths, command output, audio,
and private errors remain in bounded private retention.

## Rollback

1. Stop admission and flush durable queue, native journal, reverse-decision,
   thread-lifecycle, and private authority receipts.
2. Restore the previous binary **and its matching generated manifest/schema**.
   Never pair an old binary with a new decoder or vice versa.
3. Restart the supervisor. It creates a new generation. Old watches, commands,
   processes, searches, imports, realtime sessions, and remote grants cannot be
   inherited.
4. Reconcile visible threads from app-server, retain the same queued intent and
   client-message IDs, and require fresh authority for every dangerous action.
5. Run the binary-manifest smoke and the affected family smoke before reopening
   admission.

## On-call triage

- `binary_manifest_mismatch`: quarantine the build. Compare executable hash and
  exported schemas with the bundled manifest.
- `protocol_decode_drift`: inspect private compatibility receipts by method and
  reason. Do not request raw payloads in public logs.
- `supervisor_not_ready`: inspect generation/reconnect state, then reconcile
  visible threads after readiness.
- `reverse_request_unsettled`: deny at the exactly-once arbiter boundary and
  inspect causal decision receipts.
- `queue_journal_corrupt` or `recovery_corrupt`: stop promotion. Preserve the
  file for private diagnosis and restore from the last valid revision without
  inventing completion.
- `transport_overload`: stop admission, allow bounded teardown, and investigate
  producer/consumer pressure before raising caps.
- `family_smoke_failure`: quarantine only after identifying the capability
  family. Never waive it using the aggregate method-count dashboard.
