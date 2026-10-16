# OpenAgents Rust client

`openagents-sdk` is the public client package. Applications may name the
dependency `openagents`; that alias does not rename the existing CLI.
Use a Git dependency pinned to a commit containing this crate until package
publication is authorized. There is no crates.io release yet. Async operations
run on the application's Tokio runtime.

```toml
[dependencies]
openagents-sdk = { git = "https://github.com/OpenAgentsInc/openagents.git", rev = "ceabf529d2698fce04c3d433e222258457b74e41" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

This pin passed an outside-checkout build, account/workspace discovery, and
complete saved-history reads against the public API with a dedicated test
account; see the [verification record](../../docs/sdk/history-client-evidence.json).

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
minutes), and no redirects or implicit retries. Auth and list JSON are bounded
to 1 MiB; saved transcript pages are bounded to 17 MiB. Input files and computer
captures are bounded to their saved sizes (at most 10 MiB and 15 MiB).
Typed rate-limit/unavailable errors retain `Retry-After` as seconds or an HTTP
date. Error diagnostics omit response bodies and credential values.

Normal dependencies contain the HTTP client and portable device protocol;
the CLI's `system` feature is disabled. Server/tenancy dependencies are test
fixtures only. Tests exercise the real public website handlers over synthetic
stores, plus cancellation, response loss, redirects, bounds, and account changes.
See [the SDK plan](../../docs/sdk/delivery-plan.md) for history and Computers work.

The `history` module provides `Client::history`, `history_projects`,
`shared_history`, `transcript`, and `download`. Readers fetch one page per call,
accept a cancellation token, and bind every result/reference to the session's
account and deployment. Use the deployed `/v1/history` contract; the legacy
`/threads` offset lists and newest-200 previews are not full-history readers.
`HistoryIndex` optionally merges owned/shared summaries by chat ID and sorts
pinned and archived lists. Its ordering covers loaded pages; fetch through the
end before treating the list as complete. Replace it on access refresh and
clear it on sign-out. The SDK does not persist private history.

Unknown source, role, file-kind, and tool-status values remain `Other(String)`;
unknown message content remains in `SavedMessage::extra`. Missing tool detail
is `None`; empty generated-file/artifact arrays mean those objects weren't
saved. A file/capture without `download_ref()` has unsupported metadata or
exceeds the supported bound. Valid metadata can still return `NotFound` when
saved bytes are gone. Downloads use only configured API paths and never follow
server-supplied URLs or redirects. The library writes no downloaded files.

`Changed` or `InvalidCursor` requires a new reader and a fresh transcript;
never append pages from different revisions. Transient errors leave the
position unchanged. Duplicate/nonadvancing IDs and truncated pages are errors,
not successful completion. Cancel readers on account changes and compare the
returned `ChatRef` before applying a late result to a view. Clear or replace
cached shared content when access is refreshed; every new server read uses the
existing access check (up to 30 seconds of successful-check caching).

```rust
use openagents_sdk::{CancellationToken, ChatRef, Client, Error, Session};

async fn read_selected(
    client: &Client, session: &Session, selected: &ChatRef, cancel: &CancellationToken,
) -> Result<usize, Error> {
    let mut reader = client.transcript(session, selected)?.with_page_size(100)?;
    let mut count = 0;
    while let Some(page) = reader.next_page(client, session, cancel).await? {
        // Render this account-bound page before requesting another one.
        count += page.data.len();
    }
    Ok(count)
}
```
