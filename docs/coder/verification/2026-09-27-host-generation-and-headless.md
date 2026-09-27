# Host generation and headless approval verification — September 27, 2026

This record covers two items of
[issue #9719](https://github.com/OpenAgentsInc/openagents/issues/9719):
**One host generation** and **Headless approval end to end**.

## Evidence class

All evidence here is synthetic, from one macOS computer. The tests use
temporary directories, fixture host bundles that are shell scripts, a local
NIP-42 test relay, and loopback sockets. No launchd or systemd unit was
installed, and no production relay, second computer, or physical device was
involved.

## One host generation

### What changed

Before this change, a standalone `coder host serve` kept its own counter in
`~/.openagents/host/generation`, with a clock floor, while the
`coder-service` launcher counted from zero in `launcher.json`. A standalone
run followed by a service run could therefore show clients a much lower
generation, which NIP-REACH clients refuse.

Now `coder_service::generation` owns one counter per host root, the
`generation` record, and is the only code that computes a next generation:

- The launcher reserves each generation from the counter, with its own
  `launcher.json` value as a floor, and passes the value in
  `OPENAGENTS_HOST_GENERATION` and its root in
  `OPENAGENTS_HOST_GENERATION_ROOT`.
- `coder host serve` claims a given generation (from the launcher, or
  `--generation N`) in that root before it serves, or advances and claims
  the counter itself when none is given. A claim below the counter, or of a
  value already used, refuses before the host binds anything.
- The next value is one more than the largest of the recorded value, the
  caller's floor, and the Unix time in seconds.
- Each change holds an exclusive `flock` on `generation.lock` and replaces
  the record atomically with a synced rename. A damaged record refuses; a
  bare number from the earlier standalone counter reads as a used value.

### Checks

| Check | Result |
| --- | --- |
| `cargo test -p coder-service --lib` | 36 passed. |
| `cargo test -p coder-host --lib` | 11 passed, after rebasing onto the WebSocket direct-channel change. |
| `cargo clippy -p coder-service -p coder-host --all-targets` | No warnings. |

The tests that establish the item:

| Test | Establishes |
| --- | --- |
| `generation::tests::standalone_and_service_starts_never_decrease` | Twelve interleaved standalone and service starts strictly increase. |
| `launcher::tests::standalone_and_service_hosts_share_one_generation_counter` | Standalone, then the real launcher's host, then a trial update, then standalone, then the launcher again: every generation exceeds the one before, and the descriptor reports the launcher's value. |
| `generation::tests::concurrent_starts_serialize_to_distinct_values` | Eight threads making 128 concurrent advances and reservations get distinct values, and the record ends at the highest. |
| `generation::tests::a_crash_between_reserve_and_use_skips_the_value` | A reservation never claimed is skipped by the next reservation or standalone start, and a host holding the lost value is refused. |
| `launcher::tests::a_launcher_crash_before_its_host_claims_skips_the_generation` | The same through a launcher that crashes after starting its host. |
| `generation::tests::a_crash_during_a_write_leaves_the_previous_record` | An interrupted write leaves the last complete record, and its temporary file is removed on the next change. |
| `generation::tests::a_claim_refuses_a_lower_or_used_generation` | `--generation N` below the counter, or repeated, refuses; a higher one is admitted once and becomes the floor. |
| `generation::tests::a_generation_follows_the_clock_floor_and_a_caller_floor` | The clock and a launcher floor both bound the next value from below. |
| `generation::tests::a_legacy_number_is_read_and_a_damaged_record_refuses` | The earlier bare-number file carries forward; a damaged record refuses and is left untouched. |
| `coder_host::generation::tests::a_standalone_start_advances_and_a_given_generation_is_claimed_once` | The `coder host serve` path: advance, refuse a lower or repeated value, and claim a launcher reservation once. |

## Headless approval end to end

### What changed

`coder host` had no reverse-enrollment command. The only path was
`coder-access request`, which runs its own relay loop with the host key, so
it competed with a running `coder host serve` for the same requests. The new
`coder host request` command, backed by `coder_host::enroll`, records and
publishes the request, prints the code, and reads the outcome from the
access store while the resident host answers the approval on the relay.

### Checks

| Check | Result |
| --- | --- |
| `cargo test -p coder-host --test headless` | 5 passed, three consecutive runs. |
| `cargo test -p coder-host` | All targets passed, including `end_to_end`, `serve`, and `websocket`. |

Each test starts a local relay and a real resident host through
`coder_host::start`, with an administrator device enrolled by invitation
with every right.

| Test | Establishes |
| --- | --- |
| `an_administrator_approves_a_headless_host_and_the_new_device_opens_a_terminal` | The request is sealed to the owner and the administrator and carries no code. The administrator types the code in lowercase without the dash; the host grants standard rights to the new device; the waiting host reads `approved` with that device and grant. A repeat returns the same grant; another device is `conflict`. The new device reads presence at the host's generation, connects directly to the TCP hint, opens a terminal, and runs a command whose output it reads. |
| `a_denied_request_admits_nothing` | A denial ends the wait as `denied`; a later approval with the right code is `denied`; no device was added. |
| `five_wrong_codes_close_the_request` | Each of five wrong codes is `wrong_code` and counts; the request reads `closed` after the fifth; the right code is then `rate_limited`; no device was added. |
| `a_device_without_access_admin_cannot_approve` | A standard-rights device receives no copy of the request. Holding the exact request and the right code, its approval and denial are refused as `missing_right` naming `access_admin`, and the request stays pending. The owner then approves it. |
| `an_expired_request_refuses` | A request issued 330 seconds ago reads `expired`; the client no longer opens it; an approval and a denial built while it was current are both `expired`; no device was added. |

## Limits

- The approver side is exercised through the `coder_access` client, not
  through an application screen.
- An expired request is retained for one minute after it expires. After
  that, a decision refuses as `forbidden`, because the host no longer holds
  the request, rather than `expired`.
- `coder host request` reads the outcome from the access store, so it
  reports `approved` only while a `coder host serve` answers the request's
  relay. It does not start a relay loop of its own.
- The `coder` crate's `tests/host_cli.rs`, which starts `coder host serve`
  with `OPENAGENTS_HOST_GENERATION`, was not run for this record because the
  disk was near full; `coder_host::generation` covers the same path.
