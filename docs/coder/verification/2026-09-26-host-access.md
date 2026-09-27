# Host access verification — September 26, 2026

[Issue #9705](https://github.com/OpenAgentsInc/openagents/issues/9705) adds the
[NIP-HOST draft](../../../nips/openagents/NIP-HOST.md) and
[`crates/coder-access`](../../../crates/coder-access/README.md). A Coder host
admits a device with host-wide scoped rights by host invitation or by reverse
enrollment with a short code. It is part of the remote access program,
[#9704](https://github.com/OpenAgentsInc/openagents/issues/9704).

## Delivered behavior

- A closed right set: `observe`, `operate`, `terminal`, `review`,
  `access_read`, and `access_admin`. No right implies another. The standard
  device grant excludes both access rights.
- An owner established only by a local `init` command. A different owner is
  refused without a new store.
- Host invitations in the observer's binary layout under the `coder-host:`
  prefix. The host stores a capability digest before display. Redemption is
  single use, same-device retries return the same grant, and another device
  is refused.
- Reverse enrollment. The host persists a request, seals one copy to the
  owner and one to each current administrator, and shows an eight-character
  code it never publishes. An approval must match the signer, request digest,
  code, and expiry. Five wrong codes close the request.
- Host-signed grants bound to the device key, rights, expiry, and a
  per-device revocation epoch. Every operation checks the current grant.
  Revocation advances the epoch and discards retained replies.
- Delegation: invitations and approvals from an administrator are bounded by
  its rights and grant expiry, and a delegated invitation dies with its
  issuer's grant.
- Operations `enroll.redeem`, `enroll.approve`, `enroll.deny`,
  `invite.create`, `invite.cancel`, `device.list`, `device.revoke`,
  `task.create`, and `terminal.open`, each with one required right.
  Refusals for a missing right name it.
- Exact retries return retained signed replies; a reused request ID with
  different bytes conflicts. Consumption, grant, and reply commit in one
  atomic save.
- A client-only build (`default-features = false`) with no host store or QR
  dependency, and a CLI with `init`, `public-key`, `invite`, `cancel`,
  `request`, `approve`, `list`, `revoke`, and `serve-once`.

`coder-connect` changed only by widening visibility and adding
prefix-parameterized helpers: `Invitation::parse_prefixed`,
`encode_prefixed`, `issue`, `capability`, `terminal_qr_prefixed`, and
`Store::open_named`. Its observer behavior and file names are unchanged.

## Checks that ran

All commands ran with a worktree-local `CARGO_TARGET_DIR` on macOS with the
pinned toolchain.

| Command | Result |
| --- | --- |
| `cargo test -p coder-access` | 14 library tests and 2 CLI process tests passed. The library suite ran three times without a failure. |
| `cargo clippy -p coder-access --all-targets -- -D warnings` | Passed. |
| `cargo clippy -p coder-access --no-default-features --lib -- -D warnings` | Passed. |
| `cargo fmt -p coder-access -p coder-connect --check` | Passed. |
| `cargo test -p coder-connect` | 21 library tests and 2 CLI tests passed; 3 ignored production or export tests did not run. |
| `cargo clippy -p coder-connect --all-targets -- -D warnings` and `--no-default-features --lib` | Passed. |
| `cargo check -p gym-bridge -p coder-mobile --all-targets` | Passed; both consume `coder-connect`. |

The fixtures over the synthetic relay cover each acceptance case:

| Case | Test |
| --- | --- |
| Redeem, same-device retry after restart, other-device reuse | `redeem_then_same_device_retry_after_restart_and_other_device_refused` |
| Expiry and cancelled invitation | `expired_and_cancelled_invitations_refuse` |
| Reverse enrollment approve and deny | `reverse_enrollment_approve_and_deny`, `cli_reverse_enrollment_is_approved_by_the_owner` |
| Wrong code and a forged request digest | `wrong_codes_close_the_request_and_a_forged_digest_refuses` |
| Delegation beyond held rights | `delegation_cannot_exceed_held_rights`, `delegated_approval_is_bounded_by_the_approver` |
| Revocation during a pending request | `revocation_during_a_pending_request_refuses` |
| Stale epoch and a copied grant | `stale_epoch_and_copied_grants_refuse` |
| Crash between consumption and reply | `crash_between_consumption_and_reply_returns_the_retained_reply` |
| Per-right refusal | `observe_only_device_cannot_create_a_task_or_open_a_terminal` |
| Exact retry and request-ID conflict | `exact_retry_is_idempotent_and_a_reused_request_id_conflicts` |
| Listing and revocation by an administrator | `administrator_lists_and_revokes_devices` |
| Local owner, private store, prefix separation | `owner_is_established_locally_and_the_store_is_private` |
| Separate CLI processes | `cli_invites_serves_lists_and_revokes` |

The relay is the in-process WebSocket fixture from `coder-control`. It
requires NIP-42 authentication, accepts `3188` only from its authenticated
author, and delivers private artifacts only to author or recipient. Every
request in the relay fixtures opens a new `Host` over the private store, so
each crosses a process-state restart. The expiry fixture runs the host clock
ahead of the invitation window; the crash fixture commits a reply and drops
it before publication, then recovers it with the exact retried event.

## What these checks don't prove

- **No production relay.** No request reached `relay.openagents.com` or any
  deployed relay. Relay retention, access policy, and latency there are
  untested.
- **No physical or emulated device.** No phone, simulator, or emulator ran.
  QR rendering was not decoded by a camera.
- **No resident host.** `serve-once` and `request` are bounded commands.
  Long-running service, reconnection under real network loss, and sleep are
  untested; the resident host belongs to later program issues.
- **No task or terminal effects.** `task.create` and `terminal.open` are
  admitted and handed to a `Dispatch` implementation. Fixtures use an
  in-memory recorder; the CLI refuses both as `unavailable`. Nothing starts a
  task or a terminal.
- **Direct artifact binding only.** The NIP's CAP/CJ binding is specified but
  not implemented.
- **Persistence failure injection.** A failed save is not injected in these
  fixtures. The store's poison-on-failure behavior is inherited from
  `coder-connect` and covered by its tests.
- **No separate owner device.** The owner key signs directly in fixtures. A
  NIP-46 remote signer and owner-key custody on a phone are not exercised.
