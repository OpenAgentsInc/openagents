# Web Parity Status (Rust Shell)

Status: active parity tracker for Rust web shell.

## Core parity lanes

1. Auth/session + sync token issuance.
2. Codex thread/message command path.
3. Khala websocket replay/live update rendering.
4. Maintenance-mode bypass flow.

## Current policy

1. Stage changes on `staging.openagents.com` first.
2. Keep production rollback lane until explicit cutover approval.
3. Track release evidence in `docs/reports/`.
