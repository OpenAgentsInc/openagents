# 2026-04-02 Standalone Forge CLI Validation Audit

Date: 2026-04-02

## Scope

Validate the current standalone Forge CLI path on this Mac as a real end-to-end
pass, not a docs-only review.

Validated surfaces:

- `target/debug/autopilotctl forge ...`
- `target/debug/autopilot_headless_forge`
- `scripts/autopilot/headless-forge-smoke.sh`

Required behaviors checked:

- build the relevant binaries
- smoke the repo-owned standalone Forge path
- autostart the no-window Forge host from a missing manifest
- autostart the no-window Forge host from a stale manifest
- verify manifest creation, headless-log creation, and a running host process
- validate JSON output with Python `json.loads`
- verify honest no-thread failures for `forge status --json` and
  `forge handoff request "..."`
- start an explicit host with `autopilot_headless_forge --manifest-path ...`
- target that explicit host with `autopilotctl --manifest ... forge hosted sessions --json`
- check whether a clearly disposable shared session was visible for attach

## Bottom line

The standalone Forge CLI path is good on this machine after one real fix.

What works in the final post-fix run:

- `scripts/autopilot/headless-forge-smoke.sh` passes unchanged.
- `autopilotctl forge hosted sessions --json` autostarts a hidden
  `autopilot_headless_forge` host from a missing manifest.
- the same command also autostarts a fresh hidden host when the manifest is
  stale.
- the autostarted host writes the manifest, writes the headless log, and stays
  running long enough to be inspected after a 5-second dwell.
- `forge status --json` and `forge handoff request "... --json"` fail honestly
  with `No Forge thread id was supplied and the desktop has no active thread.`
- the explicit host path works with a fresh manifest.
- all `forge hosted sessions --json` outputs were valid JSON and parsed cleanly.

What I could not validate honestly:

- attach and read-back status against a real shared session, because every
  final `forge hosted sessions --json` result on this machine was
  `{"sessions":[]}` and there was no safe disposable target to mutate.

## Fix found during validation

### Problem

The first explicit-host run worked for `forge hosted sessions --json`, but the
headless log also contained:

```text
error: unexpected argument '__internal-probe-server' found
```

That meant the no-window Forge host was trying to use itself as the fallback
Probe-sidecar binary, but `autopilot_headless_forge` did not understand the
internal Probe subcommands that `autopilot-desktop` accepts.

### Fix

Updated
`apps/autopilot-desktop/src/bin/autopilot_headless_forge.rs`
to accept:

- `__internal-probe-server`
- `__internal-probe-daemon`

and delegate them to `probe_server::server`.

Added unit coverage in the same file for:

- non-internal args falling back to the normal Clap path
- parsing the internal Probe server subcommand
- parsing the internal Probe daemon subcommand
- rejecting unknown internal Probe args

Targeted verification:

```bash
cargo test -p autopilot-desktop --bin autopilot_headless_forge
```

Observed result:

```text
running 4 tests
test tests::non_internal_args_fall_back_to_clap_path ... ok
test tests::parses_internal_probe_daemon_subcommand ... ok
test tests::parses_internal_probe_server_subcommand ... ok
test tests::rejects_unknown_internal_probe_arguments ... ok
```

After the fix, the explicit-host log no longer emitted
`__internal-probe-server`.

## Commands run

```bash
cargo build -p autopilot-desktop --bin autopilotctl --bin autopilot_headless_forge
cargo test -p autopilot-desktop --bin autopilot_headless_forge
scripts/autopilot/headless-forge-smoke.sh

target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge hosted sessions --json

target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge status --json

target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge handoff request "manual no-thread check" --json

target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge hosted sessions --json

target/debug/autopilot_headless_forge \
  --manifest-path /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json

target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json \
  forge hosted sessions --json
```

I also used `pgrep`, `ps`, `tail`, and Python `json.loads(...)` checks to
verify process liveness, log creation, and JSON validity after each path.

## Repo smoke result

Command:

```bash
scripts/autopilot/headless-forge-smoke.sh
```

Observed output:

```text
{
  "sessions": []
}
checking honest no-thread failures

standalone Forge smoke passed
manifest: /var/folders/v2/0ls5nf1j509db69fk73k7ms40000gn/T/tmp.My1AZs5hrf/forge-desktop-control.json
log: /var/folders/v2/0ls5nf1j509db69fk73k7ms40000gn/T/tmp.My1AZs5hrf/forge-headless.log
host pid: 57150
```

Note:

- the smoke script cleans up its temp directory on exit, so the path above is
  the real observed runtime path, not a retained artifact path

## Manual standalone autostart result

### Missing-manifest path

Manifest root:

```text
/tmp/openagents-forge-manual-postfix-20260402-210518
```

Command:

```bash
target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge hosted sessions --json
```

Observed JSON:

```json
{
  "sessions": []
}
```

Observed running process immediately and after 5 seconds:

```text
57418 00:00 /tmp/openagents-forge-validation-20260402-210518/target/debug/autopilot_headless_forge --manifest-path /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json
57418 00:05 /tmp/openagents-forge-validation-20260402-210518/target/debug/autopilot_headless_forge --manifest-path /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json
```

Observed manifest:

```json
{
  "auth_token": "RWlpnGbWoRbQxmMzEupGmkYON7Uy2M46",
  "base_url": "http://127.0.0.1:56761",
  "generated_at_epoch_ms": 1775189839677,
  "identity_path": "/Users/christopherdavid/.openagents/pylon/identity.mnemonic",
  "latest_session_log_path": "/Users/christopherdavid/.openagents/logs/autopilot/latest.jsonl",
  "listen_addr": "127.0.0.1:56761",
  "pid": 57418,
  "schema_version": 1
}
```

Observed log path:

```text
/tmp/openagents-forge-manual-postfix-20260402-210518/forge-headless.log
```

Observed log tail:

```text
starting autopilot-headless-forge; manifest will be written to /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json
INFO autopilot_desktop::nip28_chat_lane: nip28: lane starting
INFO breez_sdk_spark::sdk_builder: Building SparkWallet with user agent: breez-sdk-spark/551f453
WARN spark::operator::rpc::connection_manager: Failed to install rustls crypto provider, ignoring error
INFO autopilot_desktop::spark_wallet: Spark wallet context network=mainnet identity_path=/Users/christopherdavid/.openagents/pylon/identity.mnemonic fingerprint=02646b508a..6f258cdb
ERROR autopilot_desktop::input: ui error [autopilot.chat]: Codex lane disabled for this desktop session
INFO breez_sdk_spark::sdk: Sync trigger changed: SyncRequest { sync_type: SyncType(Wallet | WalletState | Deposits | LnurlMetadata), reply: Mutex { data: None } }
```

The log was also checked to confirm that `__internal-probe-server` did not
appear in the final fixed run.

### Honest no-thread failures

Command:

```bash
target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge status --json
```

Observed stderr and exit:

```text
STATUS_EXIT=1
Error: No Forge thread id was supplied and the desktop has no active thread.
```

Command:

```bash
target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge handoff request "manual no-thread check" --json
```

Observed stderr and exit:

```text
HANDOFF_EXIT=1
Error: No Forge thread id was supplied and the desktop has no active thread.
```

### Stale-manifest path

After killing PID `57418`, I reran:

```bash
target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json \
  forge hosted sessions --json
```

Observed JSON:

```json
{
  "sessions": []
}
```

Observed replacement process:

```text
57714 00:00 /tmp/openagents-forge-validation-20260402-210518/target/debug/autopilot_headless_forge --manifest-path /tmp/openagents-forge-manual-postfix-20260402-210518/forge-desktop-control.json
```

Observed rewritten manifest:

```json
{
  "auth_token": "24DyxjnixBWDUJLC9RFa5YLW6enJb9x3",
  "base_url": "http://127.0.0.1:56802",
  "generated_at_epoch_ms": 1775189847501,
  "identity_path": "/Users/christopherdavid/.openagents/pylon/identity.mnemonic",
  "latest_session_log_path": "/Users/christopherdavid/.openagents/logs/autopilot/latest.jsonl",
  "listen_addr": "127.0.0.1:56802",
  "pid": 57714,
  "schema_version": 1
}
```

That confirmed the stale manifest path re-autostarted the hidden host and
replaced the endpoint/token tuple.

## Explicit host result

Manifest root:

```text
/tmp/openagents-forge-explicit-postfix-20260402-210518
```

Host command:

```bash
target/debug/autopilot_headless_forge \
  --manifest-path /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json
```

Targeting command:

```bash
target/debug/autopilotctl \
  --manifest /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json \
  forge hosted sessions --json
```

Observed JSON:

```json
{
  "sessions": []
}
```

Observed process immediately and after 5 seconds:

```text
57795 00:09 /tmp/openagents-forge-validation-20260402-210518/target/debug/autopilot_headless_forge --manifest-path /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json
57795 00:14 /tmp/openagents-forge-validation-20260402-210518/target/debug/autopilot_headless_forge --manifest-path /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json
```

Observed manifest:

```json
{
  "auth_token": "7mKfDdG0kqTuIjZQfPA1z-CJ6oHkgAd7",
  "base_url": "http://127.0.0.1:56824",
  "generated_at_epoch_ms": 1775189851851,
  "identity_path": "/Users/christopherdavid/.openagents/pylon/identity.mnemonic",
  "latest_session_log_path": "/Users/christopherdavid/.openagents/logs/autopilot/latest.jsonl",
  "listen_addr": "127.0.0.1:56824",
  "pid": 57795,
  "schema_version": 1
}
```

Observed log path:

```text
/tmp/openagents-forge-explicit-postfix-20260402-210518/forge-headless.log
```

Observed log tail:

```text
starting autopilot-headless-forge; manifest will be written to /tmp/openagents-forge-explicit-postfix-20260402-210518/forge-desktop-control.json
INFO autopilot_desktop::nip28_chat_lane: nip28: lane starting
INFO breez_sdk_spark::sdk_builder: Building SparkWallet with user agent: breez-sdk-spark/551f453
WARN spark::operator::rpc::connection_manager: Failed to install rustls crypto provider, ignoring error
INFO autopilot_desktop::spark_wallet: Spark wallet context network=mainnet identity_path=/Users/christopherdavid/.openagents/pylon/identity.mnemonic fingerprint=02646b508a..6f258cdb
ERROR autopilot_desktop::input: ui error [autopilot.chat]: Codex lane disabled for this desktop session
INFO breez_sdk_spark::sdk: Sync trigger changed: SyncRequest { sync_type: SyncType(Wallet | WalletState | Deposits | LnurlMetadata), reply: Mutex { data: None } }
```

The final explicit-host log did not contain
`unexpected argument '__internal-probe-server' found`.

## Shared-session attach check

I checked for a safe attach candidate through the same final validated path.

Every final hosted-session discovery output on this machine was:

```json
{
  "sessions": []
}
```

So there was no clearly disposable shared session to attach to, and I did not
mutate any real teammate session.

## Remaining gaps

- No disposable shared session was visible, so attach and post-attach status
  read-back were not validated in this pass.
- The no-window host still logs
  `Codex lane disabled for this desktop session`, which matches the documented
  default behavior for `autopilot_headless_forge` and did not block any Forge
  CLI path in this run.
