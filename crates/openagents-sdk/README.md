# OpenAgents Rust client

`openagents-sdk` is the public client package. Applications may name the
dependency `openagents`; that alias does not rename the existing CLI.
Use a Git dependency pinned to a commit containing this crate until package
publication is authorized. There is no crates.io release yet. Async operations
run on the application's Tokio runtime.

```toml
[dependencies]
openagents-sdk = { git = "https://github.com/OpenAgentsInc/openagents.git", rev = "7ff3991b2ff2d9b4ebbea3d4898f804f0d5d9b89" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The pin above passed an outside-checkout client-only build and the complete
public Computers workflow, with all test resources deleted; see the
[Computers verification record](../../docs/sdk/computers-client-evidence.json).
Earlier auth/history qualification used `4b5b70d72b1f1d142170417c283937876dc5734d`;
its [verification record](../../docs/sdk/history-client-evidence.json) retains
that revision and the dedicated-account public API reads.

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
are forgotten locally rather than revoked by `sign_out`. Use `sign_out_record`
to revoke a saved app session when account discovery cannot complete. This
does not restore the session or authorize history reads.

Login polling shares the existing CLI protocol, respects the server's interval,
and stops on cancellation, denial, expiry, or an uncertain response. A lost
redemption response requires a new login; it is never automatically replayed.
Canceling a request cannot revoke a session whose token was never received;
the website can remove it. No credential is adopted from CLI files or
`OPENAGENTS_TOKEN`, and no browser is launched by this library.

Requests have bounded timeouts (20 seconds by default, configurable up to two
minutes), and no redirects. Auth and history have no implicit retries;
Computers has its own bounded, operation-specific retry policy. Auth and list JSON are bounded
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

Solver imports its work service's limited credential with
`Client::authenticate_computer_job`. The issuer checks that credential on every
operation; it authorizes the saved job's computer and commands. Reacquire it
from the work claim after restart. See the
[Solver migration](../../docs/sdk/solver-computers.md) for its scope and cleanup.

The experimental `computers` module executes the generated public Computers
contract through this same client and account session. It allocates cloud
computers through the public API; it does not link or run a host, provider,
process harness, or database. Missing file capability means unsupported, and
unknown server states remain readable without counting as ready or successful.

```rust
use openagents_sdk::{Client, Session, CancellationToken, computers::CreateOptions};

async fn allocate(client: &Client, session: &Session) -> Result<(), openagents_sdk::computers::Error> {
    let computers = client.computers(session)?;
    let request = computers.create_request(CreateOptions::default())?;
    // Save this request privately before sending; reuse it after response loss.
    let computer = computers.create(&request, &CancellationToken::new()).await?;
    let reference = computer.reference().clone();
    // Wait for Awake before commands, or explicitly stop, wake, and delete.
    Ok(())
}
```

`create_request` and `command_request` generate a key once; their serializable
records bind settings to an account and deployment. Save them privately before
sending and retain them across restarts. Commands and create retry only with
the same key and exact body. `Error::Uncertain` means inspect the original
resource or reuse the original keyed request. Never resolve it with a new
command or key. `Error::code`, `status`, `refusal`, and `retry_after` expose
bounded server information without including private error messages in logs.
No payment or automatic x402 spending occurs.

HTTP 402 responses return `Error::PaymentRequired` (including the Computers
error type). `payment_required().header()` exposes one unmodified
`PAYMENT-REQUIRED` text header up to 64 KiB, or a typed reason for a missing,
repeated, oversized, or non-text header. The header is untrusted; the SDK does
not decode or validate its offer, contact its URLs, or send a payment. Debug
and display output omit the header and response body. Computers does not retry
402; an earlier lost mutation response still returns `Uncertain` with the
challenge available through the same accessor. These new error variants require
an additional arm in exhaustive matches. Automatic payment remains deferred
under [#11822](https://github.com/OpenAgentsInc/openagents/issues/11822).

Configure `Client::with_timeouts` for connect and per-request deadlines,
then `Computers::with_operation_timeout` and `with_retries` for a bounded
operation. Reads and keyed creates/commands retry eligible failures with
jitter and `Retry-After`; a retry delay outside the caller's bounds returns
an error rather than retrying early. Auth/history keep their previous behavior.
Stop, wake, delete, command cancellation, and file uploads send explicit
intents once. A lost upload response requires `reconcile_write` to compare
current bytes with the retained size/hash; it never triggers another upload.
Matching bytes establish what is present, not which writer produced them.

`wait` and `wait_command` poll with a caller deadline. A local timeout or
cancellation sends no remote stop or cancellation. `OutputReader` fetches
bounded binary stdout/stderr pages and can be serialized with its exact byte
cursors for reconnect. Dropping it does not stop the command. Check gaps,
terminal state, and the command outcome; an unknown state never means success.
File bodies are binary, bounded to the advertised limit and 4 MiB, and verified
against SHA-256, ETag, and `Cache-Control: no-store`. Paths are relative to the
workspace. Writes default to refusing replacement; choose `allow_overwrite`
on the intent before sending if replacement is intended.

The [runnable Computers example](examples/computers.rs) only accepts the
dedicated production or staging test account. Set `OPENAGENTS_TEST_TOKEN`
explicitly, then run with a private checkpoint directory:

```sh
sdk_scratch="$(openagents scratch)"
cargo run -p openagents-sdk --example computers -- "$sdk_scratch/computers-sdk-check"
```

It creates a computer, uploads binary bytes, checks them through a command,
stops/wakes the computer, verifies the same file, removes the file, and deletes
the computer. It attempts cleanup on failure and retains original request
identities for inspection or restart. Its evidence contains IDs, booleans,
and timing, without credentials or command/file output. Keep checkpoints
private. If deletion is uncertain, inspect that computer before another
explicit delete intent; the example does not replay a saved delete.
Generated bindings remain owned by the
[public contract workflow](../../docs/sdk/computers.md#published-contract-and-generated-rust-bindings).
Package publication remains an owner decision.

## Agent Lab

`Client::agent_lab(session)` saves versioned drafts, suites, and finite experiments,
starts and reads test runs, records review, stops or archives work, and explicitly
promotes a candidate to the existing agent catalog. It uses the account-private
`/v1/labs/agent` API. Supporting computers prepare separate project copies and execute
ordinary agent assignments. Saved `agent_lab::Request<T>` values bind mutations to
one account and deployment; persist them privately before sending and reuse
the original request ID after an uncertain response. Lab JSON reads are bounded
to 16 MiB so full version histories remain readable.

See [Agent Lab](../../docs/agent-lab/README.md) and
[`examples/agent_lab.rs`](examples/agent_lab.rs). These additions require a commit
containing Agent Lab; the older Computers-only pin above does not include them.

## Labs

`Client::labs(&session)` creates an account-bound `labs::Client` for kinds,
tools, documents, versions, cards, jobs, artifacts, notes, and events. The module
re-exports `lab-kit` types. Normal client dependencies do not include a Labs
server or store. Enable `labs-cad` for local CAD previews through the same
`Cad::check` and `Cad::apply` rules as the server. Preview measurements must
match the proposed content; the server rebuilds it before saving.

External workspaces enabling `labs-cad` must copy the four public
`[patch.crates-io]` entries from the
[standalone Labs example](../../examples/sdk-labs/Cargo.toml) into their own
workspace manifest. Cargo does not inherit dependency workspace patches. The
SDK's default features do not need them.

```rust
use openagents_sdk::{CancellationToken, Client, Session, labs};

async fn observe(client: &Client, session: &Session) -> Result<(), labs::Error> {
    let labs = client.labs(session)?;
    let cancel = CancellationToken::new();
    let document = labs.document_ref("me", "bracket")?;
    let version = labs.version(&document, 1, &cancel).await?;
    let request = labs.job_request(labs::CreateJob {
        space: "me".into(),
        version: version.reference(),
        want: labs::Want::Job { job_type: "rebuild".into(), params: serde_json::json!({}) },
    })?;
    // Save this account-bound request privately before the first send.
    let job = labs.create_job(&request, &cancel).await?;
    let job = labs.job_ref("me", &job.id)?;
    let result = labs.wait(&job, labs::WaitOptions::default(), &cancel).await?;
    // Check result.status; an uncertain job does not mean success.
    Ok(())
}
```

Edits return `EditResult::Created`, `Stale`, or `Refused`. Other failures expose
bounded typed details through `Error::refusal`, `status`, and `retry_after`;
diagnostics omit document content. Jobs retry eligible failures with the same
saved key and exact body. Local cancellation and wait deadlines do not cancel
remote jobs; call `cancel_job` explicitly. Other writes are sent once.

Cloned Labs clients share their active account. Call `set_session` or
`clear_session` on account changes; outstanding requests, file readers, and
event readers then discard old results. Old document and job references cannot
be used for another account or deployment. Clear your private UI and caches at
the same time. The client does not store credentials or document content. JSON requests are
limited to 1 MiB and responses to 16 MiB; use smaller pages for large versions.

`events` reconnects with the last event delivered to the caller. Save `cursor()`
after applying an event and pass it to a new reader after restart. File metadata
provides short-lived signed links or an authenticated API fallback. `download`
streams full or byte-range reads without following redirects; it sends no account
credential to signed URLs. Full reads check the file hash at EOF, and range
reads check the exact requested byte count. Refresh metadata after link expiry.
Private-local uploads are refused before a request is sent. See the
[Labs API](../../docs/sdk/labs.md) for route limits and authorization.
