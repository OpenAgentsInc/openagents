# Live browser input

`coax/web/player-browser.coax` builds on `lib/net/cdp.coax` for real-time,
ordinary input. `cdp_launch_live` explicitly opens a visible hardware browser;
the existing launchers remain headless. Live navigation rejects clock-control
query parameters. `pb_tap` hit-tests the current target before mouse input;
`pb_type` uses the browser's input transport. Read-only observation expressions
are internal library calls, never a user-supplied script operation.

`player-webdriver.coax` provides the same transport for Safari on macOS and
dedicated Safari simulators/devices. The caller supplies standard capabilities.
Safari must have Remote Automation enabled. Keyboard holds are one bounded
W3C action chain with an explicit release. `wd_stick` describes synchronized
touch sources; a server must actually support them before a product may qualify
touch gameplay. Successful protocol delivery alone is not a gameplay verdict.

The caller owns a private evidence directory and renews a one-second input
lease. Run `pb_watchdog` or `wd_watchdog` in a separate process. It releases
inputs on expiry, and closes only its dedicated browser session if the owner
dies. No input may remain held during model/network waits. Recorders capture
real-time presented frames, preserve receipt timing in ffconcat, and honor
`record.pause`/`record.stop`; credentials must be entered before recording.
Caller orchestration owns budgets, account leases, retention, assertions,
encoding and qualification. These helpers do not constitute a passing test.

Browser hosts publish immutable `coaxialAssetReceipts` with SHA-256 and byte
length for the exact `/build/*.wasm` and `.wgsl` bytes handed to their runtimes.
No second fetch, credentials or engine state enters a receipt. Compare those
with a release catalog; missing or mismatched receipts prevent qualification.
`node coax/web/player-assets.test.ts` checks this boundary independently.

Native checks: `coax test coax/web/player-browser.coax --library` and
`coax test coax/web/player-webdriver.coax --library`. Product-specific journeys,
accounts and assertions belong downstream.
