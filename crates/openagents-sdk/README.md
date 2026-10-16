# OpenAgents Rust client

`openagents-sdk` is the public client package. Applications may name the
dependency `openagents`; that alias does not rename the existing CLI.
Use a Git dependency pinned to a commit containing this crate until package
publication is authorized. There is no crates.io release yet. Async operations
run on the application's Tokio runtime.

```toml
[dependencies]
openagents-sdk = { git = "https://github.com/OpenAgentsInc/openagents.git", rev = "c2baf83c52356aee1f7d119d689e077ce07ae768" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

This pin passed an outside-checkout build and an account/workspace read against
the public API with a dedicated test account; see the
[verification record](../../docs/sdk/auth-client-evidence.json).

```rust
use openagents_sdk::{CancellationToken, Client, Deployment};

async fn example() -> Result<(), openagents_sdk::Error> {
    let client = Client::new(Deployment::production())?;
    let mut login = client.start_login("Coder Desktop", "My Mac").await?;
    // Show the code and open this validated URL using the application's UI.
    let url = login.verification_url().clone();
    let code = login.user_code().to_owned();
    let session = client.wait_for_login(&mut login, &CancellationToken::new()).await?;
    let account = client.account(&session).await?;
    // Persist session.record() explicitly in the application's OS credential store.
    client.sign_out(&session).await?;
    Ok(())
}
```

For a local fixture or staging, pass the matching explicit API base (ending
in `/v1`) and website origin to `Deployment::new`. Production uses
`https://api.openagents.com/v1` and `https://openagents.com`. Plain HTTP is
allowed only for an explicitly configured loopback address. Credentials
cannot move between deployment pairs.

`CredentialStore` is a caller-owned hook; no store is selected automatically.
Serialize `CredentialRecord` only into the app's credential store. Restore it
with `Client::restore`, which checks the current account before returning it.
An outage leaves the caller's stored record intact. Invalid/expired credentials
require login again. App sessions have a known expiry; imported sign-in keys or
sessions may have no locally known expiry and are checked by the service.
There is no refresh-token API. On logout, always clear local credentials and
private views, including when the remote sign-out request fails. Sign-in keys
are forgotten locally rather than revoked by `sign_out`.

Login polling shares the existing CLI protocol, respects the server's interval,
and stops on cancellation, denial, expiry, or an uncertain response. A lost
redemption response requires a new login; it is never automatically replayed.
Canceling a request cannot revoke a session whose token was never received;
the website can remove it. No credential is adopted from CLI files or
`OPENAGENTS_TOKEN`, and no browser is launched by this library.

Requests have bounded timeouts (20 seconds by default, configurable up to two
minutes), a 1 MiB JSON response limit, and no redirects or implicit retries.
Typed rate-limit/unavailable errors retain `Retry-After` as seconds or an HTTP
date. Error diagnostics omit response bodies and credential values.

Normal dependencies contain the HTTP client and portable device protocol;
the CLI's `system` feature is disabled. Server/tenancy dependencies are test
fixtures only. Tests exercise the real public website handlers over synthetic
stores, plus cancellation, response loss, redirects, bounds, and account changes.
See [the SDK plan](../../docs/sdk/delivery-plan.md) for history and Computers work.
