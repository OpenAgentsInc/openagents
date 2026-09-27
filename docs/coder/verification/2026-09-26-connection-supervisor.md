# Connection supervisor verification — September 26, 2026

[Issue #9707](https://github.com/OpenAgentsInc/openagents/issues/9707) adds
`crates/coder-link`, a per-host connection supervisor for Coder clients. The
[crate README](../../../crates/coder-link/README.md) describes the phases,
signals, policy, registry, and the planned `coder-mobile` adoption.

## Delivered behavior

- A deterministic `Supervisor` per host with the phases `Available`,
  `Offline`, `Connecting(stage)`, `Backoff`, `Connected`, and
  `Blocked(reason)`. It returns transport work as commands and performs no
  input or output.
- Signals for connect, disconnect, retry now, network changes, application
  background and activation, and credential changes. A short background
  probes the connection; a long background replaces it.
- An establishment timeout, a probe timeout, and a capped backoff ladder that
  resets after a stable connection. Blocked failures never retry until
  `RetryNow`, `Connect`, or `CredentialsChanged`. `Offline` waits for a
  network signal. A probe failure reconnects without the first wait.
- Transport health (`Phase`) and data freshness (`Freshness`) as separate
  fields. A failed subscription marks data stale and leaves the phase at
  `Connected`.
- A `Registry` keyed by host key with register, switch off, switch on, remove
  with a forget callback, and a credential-change sweep limited to routes
  that name the credential.
- A `Connector` trait for transports and a `Clock` trait with `ManualClock`
  for tests and `SystemClock` for applications.

The crate has no dependencies beyond the standard library.

## Evidence

All evidence is synthetic unit testing on macOS with Rust 1.97.1. No network,
simulator, emulator, or physical device was involved, and no application uses
the crate yet.

| Check | Result |
| --- | --- |
| `cargo test -p coder-link` | 12 tests passed. |
| `cargo clippy -p coder-link --all-targets -- -D warnings` | Passed with no warnings. |
| `cargo fmt -p coder-link -- --check` | Passed. |

The `every_transition_in_the_table` test runs 56 table cases against a
supervisor and checks its internal invariants after every step. Each case
lists its inputs, the expected phase, the exact commands from the last step,
and, where relevant, the ladder position, the data freshness, and whether the
last report was refused as stale. The cases include the following acceptance
paths:

- The ladder climbs, caps at its last step, resets after a stable connection,
  and keeps climbing after a loss before the stable period.
- A probe timeout cancels the probe, closes the connection, and reconnects at
  once; a further failure waits the second ladder step.
- A long background replaces the connection, and success closes the old one.
  Losing the old connection during a replacement keeps the attempt.
- A blocked host ignores time, network changes, and application activation,
  then leaves the blocked state on `RetryNow`.
- Credentials changed during an attempt cancel it and start a new one. The
  superseded attempt's later success is refused as stale.
- A failed subscription leaves the phase at `Connected` and marks data
  stale.

Registry tests cover removal during a backoff (the forget callback runs, no
later tick retries, and later reports return `Unknown`), removal while
connected (the connection closes before the callback), the credential sweep,
switching a host off and on, device-wide signals through the injected clock,
and refusals. Other tests cover `next_deadline`, policy validation, identifier
bounds, and the manual clock.

One expectation was changed deliberately to confirm that the table fails on a
wrong phase; the run failed with the case name, and the expectation was
restored.

## Limits

- No connector, host application, or screen uses the supervisor yet. The
  `coder-mobile` adoption plan is in the crate README.
- A server-supplied retry time has no failure variant and follows the ladder.
- The ladder has no jitter.
- The supervisor trusts connector reports; identity and grant checks belong
  to the transport and the host.
