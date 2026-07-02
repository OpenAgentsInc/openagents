# Khala Code Error-State Corpus

Status: ROADMAP_QA Q4.4 / issue #8030 implemented.

The error-state corpus lives in
`packages/khala-qa-harness/src/seed-corpus.ts` as
`KHALA_CODE_QA_ERROR_STATE_CASES`. It is fixture-tier only: the scenarios arm a
named public-safe error case, call the real typed RPC method for that surface,
and keep all degraded payloads inside existing schema-allowed fields.

Covered cases:

- `codex_binary_missing`
- `auth_expired`
- `pylon_offline`
- `single_rpc_failure_partial_degradation`
- `corrupt_session_state_recovery`
- `mcp_server_down`
- `network_loss_mid_turn`
- `interrupt_mid_tool_call`
- `app_server_crash_restart`

Every case asserts:

- the target RPC response passes the schema oracle;
- the response includes a typed degraded-state projection;
- no driver or fixture console errors are observed;
- the fixture explicitly preserves data (`dataLoss: false` or the matching
  `qa.error_state.<case>.data_preserved` marker).

The app-server crash case also covers the recovery path: it observes an errored
app-server status, restarts the fixture app server, then resumes the active
thread.

Coverage flows through `errorStateCasesExercised` in the coverage ledger and
`errorStateCases` in the seed-corpus manifest/frontier report.
