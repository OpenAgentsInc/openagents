# Coder account

OpenAgents account state for Coder Desktop. The crate uses the public Rust SDK;
it does not authenticate local workspaces or model providers. `Controller`
returns a read lease only after the service verifies the account. Replacing an
operation cancels its predecessor before the next asynchronous result can run.

The macOS store uses the app's own Keychain service and includes the profile and
API/website pair in its entry identity. Sign-out clears the stored login and
revokes the app session, including a saved session whose account discovery
failed. A deployment-specific marker prevents restore if Keychain deletion is
refused. Network failures preserve a recoverable credential without granting a
read lease. Other platforms currently report unavailable credential storage.

The GPUI control lives in `crates/ui/src/openagents_account.rs`. Production uses
`https://api.openagents.com/v1` and `https://openagents.com`. Development can set
both `CODER_DESKTOP_API_URL` and `CODER_DESKTOP_WEBSITE_URL`; setting only one is
an error. The SDK accepts HTTP only for explicit loopback development.

Run `cargo test -p coder-account -p coder-brand`. The native `account-fixture`
example uses an injected memory store, a local approval page, and no engine:

```sh
cargo run -p zeron-ui --example account-fixture --features account-fixture -- SCRATCH_DIR
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
