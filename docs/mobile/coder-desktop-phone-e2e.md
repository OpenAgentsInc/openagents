# Check desktop-to-phone sync

This is the repeatable #11849 check for the fork's UIKit app and Rust engine.
Run staging first, then production. Both sync services validate accounts with
the production public OpenAgents API. Use a dedicated test account, an isolated
headless host, and the simulator. No real model or owner account is used.

## Prerequisites

- Current `main` of `OpenAgentsInc/coder-desktop`, Rust 1.98.1, Xcode, and the
  iPhone 17 Pro Max simulator `8010197D-517D-43DA-9559-46742EFABC65`.
- A dedicated OpenAgents test account and a private file containing
  `OPENAGENTS_TOKEN=...`. Do not print its contents or use a personal account.
  The qualified account is `acct_83d16a35955ef908`; its approved file on this
  Mac is `~/work/.secrets/openagents-test-account.env`.
- Your own external Cargo target directory. Never use the owner's agent0/agent3
  directories, running desktop data, or IPC port 49777.

## Run in the fork

```sh
scratch=$(openagents scratch)
python3 scripts/ios/check-sync.py \
  --target-dir "$HOME/work/coder-desktop-target-agent1" \
  --scratch "$scratch/coder-phone-live-$(date +%s)" \
  --credential-file "$HOME/work/.secrets/openagents-test-account.env" \
  --account acct_83d16a35955ef908
```

For another dedicated test account, supply its file and expected account ID.
The coordinator checks that ID through the public SDK before starting anything.
The script builds the fork's `zeron` binary, the Rust `mobile_sync_fixture`
coordinator, and the simulator core. It runs staging before production and saves
per-run logs, result bundles, Keychain proof, and host assertions in scratch.
Use `--deployment staging` or `--deployment production` for a focused rerun;
`--skip-build` is only for binaries and fixture bindings just built from the
same checkout.

The fixture owns a fresh scratch home, data directory, project, and headless
child. It allocates a random port, refuses 49777, and checks the child owns the
listener before sending IPC. Its environment contains no model-provider
credentials. The launcher removes the temporary account credential before any
agent child can start.

The simulator-only `account-fixture` core feature and Swift's
`CODER_ACCOUNT_FIXTURE` flag enable one-use loopback credential delivery. The
account token stays in Rust and Keychain. `-account-fixture` separates the app's
test account entries, caches, display profile, device ID, and drafts from normal
app use. The old `sh.zeron.ios` installation is not replaced.

Build-only checks use `CODE_SIGNING_ALLOWED=NO`. Keychain UI checks use Xcode's
local simulator ad-hoc signing. No developer-account login or browser approval
is needed for this dedicated-key check.

## Scenario

1. The desktop creates a session. The phone sees it and opens the live transcript.
2. The phone answers single-choice and multiple-choice questions. The host
   verifies the chosen answers in its transcript.
3. The phone sends a message and queues a follow-up. The host verifies both.
4. The phone removes the queued follow-up, then stops the current turn. The
   host verifies the aborted assistant entry.
5. The phone goes to the home screen and returns to the same transcript.
6. The coordinator stops its own host. The phone sends while it is offline;
   the host restarts from its scratch data and adopts that message.
7. The phone stops the turn and signs out. A fresh app diagnostic confirms its
   fixture Keychain entry is gone.

Only the first mock turn receives `modelOptions.mockQuestions: true`.
Subsequent turns stream until stopped. This avoids racing queue controls with
a second question prompt and does not interpret message text as a test command.

See the [recorded staging and production results](coder-desktop-phone-evidence.json).

## Results and cleanup

Require `PASS` for both deployments and both cleanups. Each `result.json` must
contain all six host checks, one restart, `registry_fixtures_removed: true`, and
`child_stopped: true`. `keychain-result.json` must report `signed_in`,
`saw_session`, and `keychain_empty` as true. The script checks the app's diagnostic
is newer than that run's configuration; stale output cannot pass.

The test's teardown clears its fixture credential even after an assertion fails.
The runner always asks its Rust coordinator to clean up. Cleanup interrupts only
its test turn, deletes its space, tombstones its device/chat/session rows, and
stops its child. It never resets the account's registry. Private room rows remain
subject to the service's storage policy; this is not a claim that every room byte
was physically erased. The parent test credential is neither changed nor revoked.

If the runner is interrupted, send `POST /stop` to `control` in that run's
`config.json`, or send Ctrl-C to its coordinator. Require it to exit successfully.
A failed UI run can also clear only its fixture credential by launching the test
app with `-account-fixture -signedout -no-auth-browser`. Do not clear the normal
app's Keychain entry or data. Remove scratch run folders when their evidence is
no longer needed.

After the check, rebuild the core without the fixture and regenerate normal
bindings before committing or archiving:

```sh
RUSTUP_TOOLCHAIN=1.98.1 \
CARGO_TARGET_DIR="$HOME/work/coder-desktop-target-agent1" \
  scripts/ios/build-core.sh iphonesimulator
```

Rust refuses the fixture feature outside the simulator. The build script also
refuses Release and detects a mismatched staged core even with
`ZERON_SKIP_CORE=1`. Distribution builds use neither fixture flag.

The default app server is `https://sync.openagents.com`; these checks use the
same services' `run.app` addresses while DNS/certificate provisioning is pending.
A physical iPhone and real GitHub approval remain owner checks in workspace
`NEEDS_OWNER.md`. No App Store upload belongs to this check.
