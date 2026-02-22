# Codex App-Server Parity Audit (Desktop + Runtime Control)

Date: 2026-02-22

## Scope

This audit measures OpenAgents Codex integration coverage against the Codex app-server protocol surface in `~/code/codex`.

Source-of-truth protocol files:

- `~/code/codex/codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts:61`
- `~/code/codex/codex-rs/app-server-protocol/schema/typescript/ServerNotification.ts:47`
- `~/code/codex/codex-rs/app-server-protocol/schema/typescript/ServerRequest.ts:16`

OpenAgents implementation surfaces reviewed:

- `crates/codex-client/src/client.rs:436`
- `apps/autopilot-desktop/src/main.rs:1498`
- `apps/autopilot-desktop/src/main.rs:2694`
- `apps/autopilot-desktop/src/main.rs:3191`
- `apps/autopilot-desktop/src/main.rs:6469`
- `apps/autopilot-desktop/src/full_auto.rs:231`
- `crates/openagents-codex-control/src/lib.rs:43`
- `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php:15`

## Executive Summary

- App-server client request surface in Codex schema: `61` methods.
- OpenAgents typed wrapper coverage: `22/61` (`36.1%`).
- OpenAgents desktop actively executes: `7/61` (`11.5%`).
- Runtime control lane exposes: `6/61` (`9.8%`) via explicit allowlist.
- Server notifications in Codex schema: `39` methods.
  - Transport pass-through in desktop: effectively `39/39` (forwards all notification methods without filtering).
  - Explicit semantic handling/reducer logic: `8/39` (`20.5%`).
- Server-initiated requests in Codex schema: `7` methods.
  - Explicit auto-response handlers: `3/7` (`42.9%`).
  - Modern slash-method request handling only: `1/5` (`20.0%`).

Bottom line: current integration is a deliberate operational subset, not full app-server parity.

## Coverage Matrix

1. Client request protocol coverage (Codex schema -> OpenAgents)
- Total protocol methods: `61`
- Wrapped in `AppServerClient`: `22` (`36.1%`) at `crates/codex-client/src/client.rs:436`
- Missing in wrapper: `39`

2. Active desktop execution coverage (what desktop actually calls)
- Active methods: `initialize`, `thread/start`, `thread/resume`, `thread/read`, `thread/list`, `turn/start`, `turn/interrupt`
- Coverage: `7/61` (`11.5%`)
- Evidence:
  - Remote control dispatch: `apps/autopilot-desktop/src/main.rs:1498`
  - Startup initialize: `apps/autopilot-desktop/src/main.rs:2614`

3. Runtime remote-control coverage
- Allowlisted control methods: `thread/start`, `thread/resume`, `turn/start`, `turn/interrupt`, `thread/list`, `thread/read`
- Coverage: `6/61` (`9.8%`)
- Evidence:
  - `crates/openagents-codex-control/src/lib.rs:43`
  - `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php:15`

4. Server notification coverage
- Total protocol notifications: `39`
- Forwarding behavior:
  - All notifications are forwarded to UI/event bus (`method` + `params`) at `apps/autopilot-desktop/src/main.rs:3178`
  - Runtime sync currently accepts any non-empty notification method at `apps/autopilot-desktop/src/main.rs:1287`
- Explicit semantic reducers in desktop/full-auto logic: `8/39` (`20.5%`)
  - `thread/started`, `thread/compacted`, `thread/tokenUsage/updated`
  - `turn/started`, `turn/completed`, `turn/diff/updated`, `turn/plan/updated`
  - `item/completed`
  - Evidence: `apps/autopilot-desktop/src/main.rs:2725`, `apps/autopilot-desktop/src/main.rs:2777`, `apps/autopilot-desktop/src/full_auto.rs:245`

5. Server-initiated request coverage
- Total protocol server requests: `7`
- Explicit response handling in desktop:
  - `execCommandApproval`
  - `applyPatchApproval`
  - `item/tool/requestUserInput`
- Coverage: `3/7` (`42.9%`) at `apps/autopilot-desktop/src/main.rs:6469`
- Missing explicit handling:
  - `item/commandExecution/requestApproval`
  - `item/fileChange/requestApproval`
  - `item/tool/call`
  - `account/chatgptAuthTokens/refresh`

## What We Do Not Implement (or only partially implement)

1. Large request-method surface remains unavailable in wrapper (`39` methods)
- Notable missing families:
  - Thread management extensions: `thread/fork`, `thread/name/set`, `thread/unarchive`, `thread/rollback`, `thread/compact/start`, `thread/loaded/list`
  - Turn control extension: `turn/steer`
  - Skills/app surfaces: `skills/remote/list`, `skills/remote/export`, `skills/config/write`, `app/list`
  - MCP/config/admin surfaces: `config/mcpServer/reload`, `configRequirements/read`, `experimentalFeature/list`, `windowsSandbox/setupStart`
  - Legacy conversation/auth methods (camelCase lane) are mostly absent.

2. Desktop uses a narrow subset of what wrapper already supports
- Wrapper supports `22`; desktop currently executes `7`.
- Example wrappers not actively used in desktop flow: `model/list`, `review/start`, `command/exec`, `account/*`, `mcpServerStatus/list`, `mcpServer/oauth/login`, `thread/archive`.

3. Server-request parity gap for modern approval/tooling flow
- `item/commandExecution/requestApproval` and `item/fileChange/requestApproval` are observed in full-auto state accounting (`apps/autopilot-desktop/src/full_auto.rs:287`) but not explicitly answered with typed approval payloads.
- `item/tool/call` and `account/chatgptAuthTokens/refresh` are not implemented.

4. Notification semantic coverage is limited
- Notifications are not dropped, but only a small subset drives state transitions and decision loops.
- Several protocol notifications are pass-through only (no typed reducer/action layer).

## Reflection: Getting to Full Parity (if desired)

1. Decide parity target explicitly before coding
- Option A: Keep intentional subset for remote mobile control (current strategy).
- Option B: Full app-server parity (all request/notification/request-response surfaces).
- Option C: Full parity only for modern slash-method API; treat legacy camelCase as non-goal.

2. Move from hand-maintained wrapper to generated parity map
- Generate a machine-readable method manifest from Codex schema and compare it against `crates/codex-client/src/client.rs` in CI.
- Fail CI on drift (new protocol methods without explicit disposition: implement/defer/reject).

3. Expand `AppServerClient` to full target surface
- Add wrappers for all in-scope missing methods (or mark intentionally unsupported with explicit errors).
- Keep request/response DTOs typed end-to-end.

4. Implement a typed server-request router
- Replace the generic `{}` fallback at `apps/autopilot-desktop/src/main.rs:3203` with strict per-method handlers.
- Add typed responses for:
  - `item/commandExecution/requestApproval`
  - `item/fileChange/requestApproval`
  - `item/tool/call`
  - `account/chatgptAuthTokens/refresh`

5. Build a real notification reducer layer
- Map each in-scope notification method to reducer behavior.
- Keep pass-through for observability, but make semantic handling explicit and test-covered.

6. Parity tests against upstream protocol fixtures
- Add tests that assert every method in the chosen parity target is either:
  - implemented and validated, or
  - intentionally unsupported with a stable error contract.

7. Only expand runtime mobile control lane where needed
- Current control lane intentionally exposes `6` methods.
- If parity target includes more remote actions, update:
  - `crates/openagents-codex-control/src/lib.rs`
  - `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
  - corresponding iOS/desktop request/receipt handling and tests.

## Recommendation

If the goal is reliable mobile control of an active desktop thread, full app-server parity is not required and would add significant surface area.

If the goal is "desktop-equivalent Codex host" behavior across surfaces, adopt Option C first (modern slash-method parity), then decide whether remaining legacy camelCase methods are still required.
