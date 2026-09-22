# Relay, worker, and program interoperability

Issue: [#9526](https://github.com/OpenAgentsInc/openagents/issues/9526).

The earlier close of this issue relied on
`crates/coder/tests/interoperability.rs`, an in-process composition, and was
reopened because a kind-and-tag ledger plus a single-process proof does not
show that the parts interoperate. This record covers the suite that replaced
that evidence: `crates/coder/tests/interop_processes.rs`, which runs the
`nostr-relay`, `coder-worker`, and `coder` binaries as real child processes
over a disposable PostgreSQL database, with scripted WebSocket clients on
authenticated NIP-42 connections.

## How to run it

The suite refuses to prove nothing quietly: with no database it reports each
test skipped. Run it the way the continuous harness does:

```sh
./scripts/test-postgres.sh
```

or against any disposable PostgreSQL database:

```sh
NOSTR_RELAY_TEST_DATABASE_URL="host=/path/to/socket user=$USER dbname=interop" \
NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
cargo test --locked -p coder --test interop_processes
```

`cargo test` builds `coder-worker` and `coder`; the `nostr-relay` binary must
sit beside them in the target directory, which `scripts/test-postgres.sh`
arranges. Queries are scoped by unique `h` tags, so the suite is stable on a
database that retains earlier runs.

## What it proves, by acceptance item

`a_package_publishes_discovers_installs_and_runs_over_the_wire`:

- A capability (`kind:30180`), a program definition (`kind:30182`), an
  extension release, a listing, and two NIP-94 artifact locators are signed
  and published as real relay events; each receives a relay `OK`.
- The relay refuses a malformed program definition and an unsupported program
  version at admission, and refuses an unauthorized extension operation.
- A second client discovers the program through `#t` step tags, resolves the
  listing and release, and reads the locators back.
- The manifest the release pins parses, each locator matches the artifact it
  names (`ext::locator_matches`), the byte closure verifies
  (`ext::verify_closure` with the release's own manifest as a dependency),
  and the install transition stages the exact lock.
- The wire-fetched definition binds against the host and `Runtime::run`
  executes both steps: a `query` step reading the request source, and a
  `module` step running the located Wasm under a `fuel` bound. The authority
  call records as `program_authority`; the guest step's output reports
  `status: ok`.
- Revocation travels the wire: the package author publishes `kind:3185`
  naming the release, the reader discovers it through the `e` tag,
  `ext::absorb` folds it into revocation knowledge, `ext::fresh` refuses
  strict admission when no checkpoint answers offline unless the pin was
  explicit, and `preserve_active_pin` keeps a revoked head from moving an
  active run's pin.
- Archive expansion for an unsupported media type refuses with
  `unsupported_feature` rather than failing silently.

`a_lost_job_recovers_by_journal_and_never_replays_unknown`:

- A customer encrypts a NIP-CJ job request (`kind:25900`, NIP-44) addressed
  to the worker's public key; the real `coder-worker` process answers with a
  `kind:26900` result the customer decrypts and verifies.
- The relay's own log shows both the request and the result admitted.
- A non-admitted customer receives a typed `not_admitted` refusal, malformed
  ciphertext a `malformed` refusal, and an unsupported payload version an
  `unsupported_version` refusal.
- The controller journal claims and dispatches a run; a durable private run
  record publishes.
- Both the relay and the worker are killed after the request is dispatched
  but before its answer is observed. Journal recovery reports the dispatched
  run `unknown`; the controller does not accept it as complete, does not
  replay it offline, and refuses a retry the authority cannot enforce.
- After restart, the durable record remains queryable, no historical copy of
  the ephemeral answer exists, a fresh job succeeds, and the worker does not
  process the lost request again. A second journal owner's claim on the same
  run is refused `claimed`.

`filters_privacy_retention_and_forks_hold_on_the_wire`:

- Tag and author filters work on live sockets; an expiring event disappears
  after its `expiration`, an ephemeral event is delivered live but absent
  from history, and a durable event survives.
- Private run records (`kind:3187`) are visible to the author and the named
  `p` recipient and to no one else; a stranger's forged record does not
  widen that set, and unauthenticated private reads are denied.
- A private capability policy is visible only to its author.
- Fencing refuses a second claim and a stale-generation handoff.

`advertised_roles_match_configuration_and_turns_record`:

- The relay's startup log reports the configured database, URL, auth
  requirement, and OpenAgents profiles.
- The worker's log reports its identity, relay, door, job concurrency, and
  the customer allow-list — the advertised claims match the configured
  behavior the other tests exercise.
- A `coder -p` child process reaches the worker over the relay, and an
  in-process `RelayDoor` turn does the same; each turn records an ATIF trace
  with the prompt and reply inspectable afterward.

## Product change the proof required

`runtime::enforced` advertised `fuel`, `memory_bytes`, `output_bytes`, and
`read_bytes` as the bounds a module step may declare, but `admit_bound` had
no validation arms for them, so every declared module bound refused with
`bound_unenforceable`. `admit_bound` now admits them consistently with
`plugin::Limits`, which is what enforcement actually reads.

## Verification

Rust 1.97.1, separate target directory per worktree, disposable PostgreSQL:

```sh
cargo test --locked -p coder --test interop_processes   # 4 passed
cargo test --locked -p coder runtime                    # 70 passed
cargo test --locked -p coder --test interoperability    # passed
cargo clippy --locked -p coder --all-targets            # clean
cargo fmt --check                                       # clean
```

The suite was run twice against the same dirty database; both runs passed.
Three unrelated timing-sensitive unit tests
(`a_timed_out_delegation_ends_its_descendants`,
`the_fan_out_is_concurrent_under_its_bound`,
`simultaneous_additions_and_removals_keep_all_answers`) flaked under parallel
load and passed in isolation; they do not touch the changed code.

## Limits

- The scripted clients exercise the wire protocol directly; they are test
  harnesses, not a second shipped implementation.
- The job-loss window is driven by process kills, not a fault-injecting
  network; packet-level loss is approximated, not simulated.
- `fresh` is exercised through its inputs; a live checkpoint service is a
  separate surface.
