# Cursor authentication-error follow-up

Investigated against main `10c9d3e2` using the supplied redacted JSONL export.
No user prompt, tool argument, output, credential, or original transcript is
checked into this repository.

## What the export establishes

The export has 13,915 events. At sequence 6177, after extensive reasoning and
tool activity, Cursor returns:

> Authentication error If you are logged in, try logging out and back in.

The following run resumes the same agent and completes at sequence 12085.
Several further turns also complete. The abandoned-run recovery is therefore
working in this incident, but it did not prevent the authentication failure.

At sequence 13915 a separate run using Grok exits with code 1. Its only retained
diagnostic is the SDK's minified `index.js:1` location. This does not identify
the exception. The earlier authentication failure used `claude-fable-5-1`.

There are no timestamps, request IDs, SDK error codes, or underlying engine
logs in the export. It cannot establish a root cause, credential expiry,
which service rejected authentication, whether a login occurred between runs,
or whether the two failures share a cause. Do not describe this as a proven
credential-cache bug or claim the upgrade below fixes the provider error.

## Follow-up: locally available user session

The user's subsequent screenshot matched a locally available journal and SDK
store. Only non-content timing/error metadata was read for this analysis; the
live user conversation was not resumed, changed, or used as a test fixture.

- The failed turn began **64.8121 minutes after the shim's first run**, after
  **25.055 minutes idle** following the previous completed turn.
- It failed in **971 ms** with the same authentication message and has a
  request ID in the SDK store. A fresh shim started the next run **15.591 seconds
  after the failure** and completed successfully on the same agent.
- Separate live SDK token exchanges returned JWT expiry timestamps about one
  hour after issuance. No token contents were logged or saved by the probe.

Inspection of the installed packages identified a relevant upstream fix:
SDK **1.0.28** caches its exchanged token without an expiry deadline, and only
handles authentication rejection around the initial transport call. SDK
**1.0.31** parses token expiry, refreshes before expiry, and wraps stream
consumption to invalidate tokens when stream authentication fails. It also
avoids permanently caching retryable token-exchange failures.

This is direct evidence of a stale-token bug in the old SDK and a strong match
for this user's timing. It does not prove every authentication error in every
session has that cause, nor does it recover the original customer's missing
backend diagnostics.

### Real SDK expiry regression

The test runs a real turn, advances **only the shim's test clock** to one minute
before the exchanged token expires, then sends a follow-up through the same
parked SDK process. It asserts another exchange occurred before closing that
process, and verifies context recall. This uses the actual installed SDK and
live Cursor service; it does not fake a successful provider response.

- **1.0.28:** initial 7 exchanges; zero additional exchanges on the aged-client
  follow-up. The explicit refresh assertion fails.
- **1.0.31:** initial 7 exchanges; 2 additional exchanges on follow-up; refresh
  assertion and context recall pass, as does subsequent same-agent resume.

The accelerated check demonstrates proactive refresh, not a wall-clock
one-hour soak or a server-side forced-expiry reproduction. The server still
considers the old token valid during the accelerated test; the failing baseline
asserts missing refresh rather than an invented provider authentication error.

Run the passing check with an authenticated SDK:

```sh
cursor_auth_probe=$(mktemp -d)
ZERON_CURSOR_STATE_DIR="$cursor_auth_probe/state" \
ZERON_CURSOR_AUTH_CLOCK="$cursor_auth_probe/clock" \
NODE_OPTIONS="--import=$PWD/crates/harness/tests/fixtures/cursor-auth-clock.mjs" \
cargo run -p zeron-harness --example cursor_stability_probe -- parked 2
```

The preload records only exchange counts, process IDs, and timestamps. The
control file and isolated state must not be shared with a real user process.

## Changes and rationale

- Update the pinned Cursor SDK from 1.0.28 to 1.0.31, the current npm version
  at investigation time. Public run/store APIs remain compatible in testing.
- Preserve allowlisted SDK error codes and request IDs from send/wait errors
  and failed run results. These were previously discarded. Do not serialize
  causes, headers, credentials, or SDK configuration objects.
- Catch SDK background promise rejections and uncaught exceptions at the
  process boundary. Send the actual error through the existing fatal protocol,
  then shut down with a two-second hard deadline. Never continue using the
  faulted process. This avoids Node's minified source dump obscuring the error.
- Add the exact reported error, an unhandled rejection, and an uncaught
  exception to production-shim regression coverage. Each must retain useful
  diagnostics, omit a secret planted in the cause object, and permit a fresh
  process to resume the same history. The prompt ledger must show only the
  failed request and the explicitly requested continuation, with no replay.
- Log failed Cursor runs at warning level, with session ID and error details,
  so default engine logs retain the failure without verbose logging.
- Allow the opt-in live probe to choose its model with
  `ZERON_CURSOR_TEST_MODEL`.

## Validation and limits

See [results.json](results.json) for measured results. Cursor CLI update reported
already current at `2026.09.15-d2fe57e`; the actual harness uses the SDK separately.

The live `claude-fable-5-1` attempt was rejected with **Model Blocked**, asking
for administrator enablement. Its provider request ID was preserved by the new
error path. This is an account access restriction, not a passing auth reproduction.
Grok live turns and Composer fault recovery are tested separately.

The original customer mid-turn authentication rejection has **not** been
reproduced live. The subsequent local incident provides evidence for the
old SDK expiry bug and the passing-new/failing-old refresh check above. Automatically resending a tool-using prompt would risk duplicate
side effects and is intentionally not introduced. Existing recovery preserves
saved checkpoints; it cannot manufacture provider state that was never saved.

Reproduce without credentials:

```sh
cargo test -p zeron-harness
```

Opt-in live checks (use provider quota and disposable workspaces):

```sh
cargo run -p zeron-harness --example cursor_stability_probe -- models 1000
ZERON_CURSOR_STATE_DIR=$(mktemp -d) cargo run -p zeron-harness --example cursor_stability_probe -- sessions 6
ZERON_CURSOR_STATE_DIR=$(mktemp -d) ZERON_CURSOR_TEST_MODEL=grok-4.6 cargo run -p zeron-harness --example cursor_stability_probe -- parked 6
```

## Ongoing safeguards

- `cursor-sdk-update.yml` checks npm's stable SDK version daily and on manual
  dispatch. It opens or updates a dedicated dependency PR, never downgrades or
  auto-merges, and explicitly dispatches compatibility checks on that branch.
  Explicit dispatch matters: PRs created with `GITHUB_TOKEN` do not trigger
  ordinary PR workflows. The repository permits Actions to create PRs.
- `cursor-compatibility.yml` runs the complete harness suite on relevant PRs.
  The release workflow also invokes it and requires success before publication.
- The credential-free SDK auth contract test installs the **exact engine pin**,
  exposes its actual transport interceptor from a disposable bundle copy, and
  substitutes network/clock inputs. It checks token expiry, 100 concurrent
  callers sharing a refresh, stream invalidation without replay, transient
  exchange recovery, and rejection of invalid credentials without a retry storm.
  Instrumentation intentionally fails if the private bundle layout changes;
  an SDK update then requires review instead of silently skipping the test.
  No real credentials or authenticated service are used by this CI test.
- Existing harness tests cover parked sessions, steering bursts, cancellation,
  crash recovery, large catalogs, and last-good model retention. Authenticated
  service tests remain opt-in: the repository has no Cursor CI credential.
- `engine.info` and the synced device row report the owning engine's selected
  Cursor SDK. Settings → Devices displays it for local and remote devices.
  Older engines report unknown; custom shim overrides are marked unverified.
  Synced SDK metadata is tied to the app version so an older writer changing
  its version cannot leave a newer SDK version falsely displayed.

These workflows become active when this PR lands on the default branch.
The version display describes the engine's configured SDK, not the native CLI
or an assertion that Cursor has already been installed/used on that device.
