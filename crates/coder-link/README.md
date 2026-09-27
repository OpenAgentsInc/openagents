# Coder Link

Coder Link gives every Coder client one connection owner per host. The owner
decides when to connect, when to wait, and when to stop trying, so screens
never run competing reconnect loops. It reports transport health and data
freshness as separate fields, so a screen can say "offline" and "out of date"
as different things.

The crate is a deterministic state machine. It has no dependencies and
performs no network, storage, or UI work. A host application injects the
transport through the `Connector` trait and time through the `Clock` trait.
[Issue #9707](https://github.com/OpenAgentsInc/openagents/issues/9707) defines
the scope; the [remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704)
composes it with host enrollment, reachability, and terminals.

## Model

A `Supervisor` owns one host. Each call takes the current moment, updates the
state, and returns `Command` values: open a connection, probe one, cancel an
attempt, or close a connection. A `Registry` keeps one supervisor per
`HostKey`, reads the clock, and passes each command to the `Connector`. The
connector starts the work and returns at once. Outcomes come back later
through `Registry::report`.

Every attempt has a fresh `AttemptId`. A report for a cancelled or superseded
attempt, or for a closed connection, returns `RegistryError::Stale` and
changes nothing. A slow transport can never revive state that the supervisor
has moved past.

### Transport health

| Phase | Meaning | Leaves when |
| --- | --- | --- |
| `Available` | Known and able to connect; no connection is wanted. | `Connect` or `RetryNow`. |
| `Offline` | A connection is wanted, and the device has no network. | The network returns, or `RetryNow`. Time alone never leaves it. |
| `Connecting(Establishing)` | Opening a connection where none exists. | Success, failure, or the establishment timeout. |
| `Connecting(Probing)` | Checking that an existing connection still answers. | Success, failure, or the probe timeout. |
| `Connecting(Replacing)` | Opening a new connection beside a presumed-stale one. | Success closes the old connection; failure closes both. |
| `Backoff { until }` | Waiting after a transient failure. | The deadline passes, or a signal says to try now. |
| `Connected` | A connection is up. | Loss, a probe or replacement, or a disconnect. |
| `Blocked(reason)` | Retrying cannot help. | `RetryNow`, `Connect`, or `CredentialsChanged` only. |

A blocked reason is `Authentication`, `Revoked`, `Incompatible`, or
`Configuration`. Time, network changes, and application lifecycle changes
never leave a blocked state.

### Data freshness

`Freshness` is independent of the phase:

- `Unknown`: the client has never caught up with the host.
- `Current { as_of }`: the client caught up on the current connection.
- `Stale { as_of, cause }`: the data may be behind. The cause is `Syncing`
  (connected, not yet caught up), `TransportDown`, or `SubscriptionFailed`.

`Report::SubscriptionFailed` marks data stale and leaves the phase at
`Connected`. A failed subscription never shows as reconnecting. A lost
connection keeps its last `as_of`, so a screen can show when its cached data
was last current.

### Signals

| Signal | Effect when the host is wanted and switched on |
| --- | --- |
| `Connect` | Starts an attempt from `Available`, `Backoff`, or `Blocked`. |
| `Disconnect` | Cancels and closes everything, then returns to `Available`. |
| `RetryNow` | Starts an attempt at once, even when offline or blocked. Probes a connected host. |
| `NetworkChanged` | Loss moves to `Offline` and closes the connection. Return connects from `Offline` or `Backoff` and probes a connected host. |
| `ApplicationBackground` | Records when the background began. |
| `ApplicationActive` | Probes a connection after a short background. Replaces it after a background of at least `Policy::long_background`. Ends a backoff. |
| `CredentialsChanged` | Restarts an attempt in flight, replaces a connection, and leaves `Blocked` or `Backoff` with a reset ladder. |

`Registry::signal_all` sends device-wide signals, such as network and
lifecycle changes, to every host.

### Policy

`Policy::default()` uses a 10-second establishment timeout, a 3-second probe
timeout, a ladder of 1, 2, 4, 8, 16, and 30 seconds, a 30-second stable
period, and a 5-minute long background.

- Each transient failure (`Timeout`, `Unreachable`, or `Closed`) waits the
  next ladder step. The last step repeats as the cap.
- A connection that stays up for the stable period resets the ladder.
- A probe failure reconnects at once, skipping the first wait. If that
  attempt fails, the supervisor waits the second step.
- `NetworkUnavailable` moves to `Offline`, which waits for a network signal
  instead of climbing the ladder.
- `Registry::next_deadline` returns when the host application next needs to
  call `Registry::tick`. Nothing needs a periodic timer.

## Registry

- `register` adds an idle host with a `Route` that names the credentials it
  depends on. Credential IDs name credentials; they never hold secrets.
- `switch_off` stops all transport work and keeps the host, its route, its
  credentials, and its cached data. `switch_on` resumes if the user still
  wants the connection.
- `remove` stops all transport work, forgets the host, and then calls a
  callback so the application clears the host's cached projections and
  credentials. Later reports for the host return `RegistryError::Unknown`.
- `credentials_changed` sends `CredentialsChanged` to each host whose route
  depends on that credential and returns those hosts. Other hosts are
  untouched.

## Adopt it in `coder-mobile`

This issue does not change `coder-mobile`. Its `App` currently keeps a single
`retry_after` time, an `active` flag, an 8-second observation timeout, and a
15-second or 60-second retry, and it combines connection and data status in
one `status` string. The following mapping replaces that logic:

1. Hold a `Registry<SystemClock, ObserverConnector>` in `App`. Key each paired
   computer by its host public key. Name its read grant as the route
   credential.
2. Implement `ObserverConnector` over `coder-connect`. `open` starts an
   authenticated session on the existing Tokio runtime and reports
   `Established` or `Failed`. `probe` runs a bounded observation on the
   current session. `cancel` and `close` drop the task or session.
3. Map `coder-connect` error codes to reports: `Transport` becomes
   `Unreachable` or `Timeout`; `Revoked` and `Expired` become
   `Blocked(Revoked)`; `Forbidden` becomes `Blocked(Authentication)`.
   `SourceChanged` and `Conflict` become `SubscriptionFailed`, because the
   transport still works and only the data needs a reload.
4. Translate requests into signals: `Request::Foreground { active }` sends
   `ApplicationActive` or `ApplicationBackground` through `signal_all`;
   `Request::RefreshNow` and `Intent::Refresh` send `RetryNow`;
   `Request::Connect` registers the computer and sends `Connect`;
   `Request::Disconnect` calls `remove` with a callback that erases the cache
   and the stored connection code.
5. Let the platform's foreground timer call `tick` at `next_deadline`. Run
   catalog and transcript refreshes only while the phase is `Connected`, and
   report `DataCurrent` after each successful page.
6. Project `Status` into two view fields: connection health from `phase` and
   `last_failure`, and data age from `freshness`.
7. Add a platform network signal. The iOS and Android hosts report network
   path changes as `NetworkChanged`.

## Limits

- Only the state machine exists. No connector, host application, or screen
  uses it yet.
- A server-supplied retry time, such as a rate-limit hint, has no failure
  variant. An adapter maps it to a transient failure, which follows the
  ladder instead of the server's time.
- The ladder has no random jitter, so many clients that fail together retry
  together. Jitter would need an injected random source to stay testable.
- The supervisor trusts the connector's reports. It does not verify host
  identity, grants, or routes; those checks belong to the transport and the
  host.

## Verify

```sh
cargo test -p coder-link
cargo clippy -p coder-link --all-targets -- -D warnings
cargo fmt -p coder-link -- --check
```

The [verification record](../../docs/coder/verification/2026-09-26-connection-supervisor.md)
lists what these checks cover.
