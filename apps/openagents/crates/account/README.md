# Coder account

OpenAgents account state for Coder Desktop and iPhone. The crate uses the public Rust SDK;
it does not authenticate local workspaces or model providers. `Controller`
returns a read lease only after the service verifies the account. Replacing an
operation cancels its predecessor before the next asynchronous result can run.

The macOS and iOS stores use the app's own Keychain service and includes the profile and
API/website pair in its entry identity. Sign-out clears the stored login and
revokes the app session, including a saved session whose account discovery
failed. A deployment-specific marker prevents restore if Keychain deletion is
refused. Network failures preserve a recoverable credential without granting a
read lease. Other platforms report unavailable persistent credential storage; explicit headless credentials can use the memory store.

The GPUI control lives in `crates/ui/src/openagents_account.rs`. Production uses
`https://api.openagents.com/v1` and `https://openagents.com`. Development can set
both `CODER_DESKTOP_API_URL` and `CODER_DESKTOP_WEBSITE_URL`; setting only one is
an error. The SDK accepts HTTP only for explicit loopback development.

Run `cargo test -p openagents-account -p openagents-brand`. The native `account-fixture`
example uses an injected memory store, a local approval page, and no engine:

```sh
cargo run -p openagents-ui --example account-fixture --features account-fixture -- SCRATCH_DIR
```

Open the URL in `SCRATCH_DIR/browser-url` in an isolated test browser and approve
it. The example captures the waiting, signed-in, and signed-out controls, checks
local and remote sign-out, and exits. It never opens the system browser or uses
Keychain. Normal app sign-in opens the SDK's validated website URL in the system
browser.

For automated approval and an isolated macOS development bundle, run the built
example through the browser helper (requires Python's `websocket-client`):

```sh
openagents browser run -- python3 scripts/check-account-fixture.py \
  /path/to/target/debug/examples/account-fixture /path/to/new/scratch-directory
```

`history::History` reads one owned or shared-project index at a time, with at
most 200 summaries per explicit page. Project names have separate bounded
pages. Its memory-only index keeps pins, archives, source, and existing IDs;
refresh replaces old rows after the first successful page. Failed reads retain
an explicitly stale list, while access refusals clear it. Disposing the view or
changing accounts cancels old reads. Nothing creates a document or starts work.

The sidebar's saved-chat route carries `ExistingChat`, including its account
lease, independently of local engine navigation. The native fixture's
`--history` option checks 205 rows, archive/shared views, retry, selection without
an engine, and removal of private rows and navigation on sign-out.

`reader::Reader` accepts an `ExistingChat` directly, validates each saved page
through the SDK, and cancels earlier selections and downloads. It keeps a
32 MiB message-content budget and at most 2,000 messages in one view window;
continuing explicitly releases older messages. Oversized pages can be retried
one message at a time. The UI virtualizes rows and presents very long text in
64 KiB parts. It never converts messages into executable engine documents.

The `--reader` native fixture opens a target outside the loaded sidebar page,
reads 405 messages, fetches a file and capture on demand, reloads after a content
change, and checks sign-out cleanup. Save file uses the native destination
picker; the fixture verifies downloaded bytes without opening that picker.

`desktop::register` shares the account controller with an embedded engine for
that data directory. The engine captures its account identity when it opens a
synced profile and refuses another account's credential until the runtime is
replaced. `desktop::sync_scope` separates caches by account, sync URL, API, and
website. An account request in progress never blocks sign-out notifications.

`Controller::import_credential` uses the SDK's supported credential flow and the
same storage, cancellation, and logout path as device-code login. The launcher
captures `CODER_DESKTOP_CREDENTIAL` before starting threads and removes it from
the environment so agent children cannot inherit it. Explicit environment
credentials use memory storage and a separate restore fence. They do not change
the saved Keychain login.
