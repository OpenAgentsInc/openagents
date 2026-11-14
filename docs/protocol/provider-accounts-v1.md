# Computer-local provider accounts and usage

The phone's Settings → Usage displays the provider accounts of its enrolled
computers. The first implementation is Claude Code. Codex uses a future
adapter with the same account/view structure; it is explicitly unsupported
until implemented. This composes the existing account-owned environment
catalog and NIP-HOST broker, not a second computer-connection system.

The contract starts in
`proto/openagents/environment/accounts/v1/accounts.proto`. Its JSON request
and view identities are separate from the published environment runtime
v1alpha1 contract. HOST's existing `app.rpc` carriage, signatures, encryption,
grant/epoch admission and correlated replies remain unchanged. Old hosts
refuse this profile. The authenticated gateway route is
`/api/chat/environment/accounts/v1?host=HOST`; the caller cannot choose a
filesystem root, executable, provider URL, recipient, credential or account
owner. The existing account/host connection selects the exact broker grant.

Account management requires an additional **local operator opt-in**, bound to
that exact device and grant. Reads require observe; login and selection
require operate as well as the opt-in. The owner's request authorizes enabling
this profile on their enrolled Mac and CoderOS computers. Revocation, expiry,
disabled configuration or another device/grant deny every operation, including
polling a login. Authentication is not a general remote terminal.

Each computer exposes its existing CLI login as `system`, without copying or
overwriting it. Added accounts have independent, 0700 `CLAUDE_CONFIG_DIR`
directories below the shared OpenAgents host directory, named only by generated
32-hex IDs. Claude Code itself performs `auth login` and writes its credentials
to its own store (Keychain on Mac, private credential file on Linux). Browser
opening is suppressed on the computer. The phone opens the CLI's allowlisted
HTTPS Claude authorization URL on an explicit tap; when Claude gives a
paste code, the phone returns it to that same running CLI. No model request
is used to sign in or measure usage. The provider's browser may require a
human sign-in; the app never claims this interaction is completed automatically.

No access/refresh token travels to the gateway or phone. A single-use paste
code is a secret in transit: TLS and existing encrypted HOST carriage protect
it against outside observers; the explicitly trusted broker can observe it.
It is not logged or retained in receipts, and request buffers/files are
cleared/deleted after use. The CLI owns PKCE/state verification and token
refresh. Usage probing never refreshes or swaps its token, avoiding the
single-use refresh races Zeron guards against.

Usage follows Zeron's observed provider quota endpoint, read locally with the
CLI's credential: `https://api.anthropic.com/api/oauth/usage`, OAuth beta
header `oauth-2025-04-20`. This is a provider-internal endpoint, not a promise
of a supported public API. Missing/malformed windows and 401/403/429/network
failures remain explicit. Keep the last successful observation labeled stale;
never turn failure into 100% remaining. Five-hour and weekly windows, including
reported model-specific weekly windows, remain separate. Remaining is
`10000 - used_basis_points`; these are plan quotas, not token/currency totals.
Display reset time and observation freshness. Normal probes cache five minutes;
forced probes are throttled to thirty seconds and rate-limit backoff is honored.

The eight-account bound includes system. A new login allocates its own profile
and ten-minute flow. It does not replace another account or automatically
select the new one. An explicit SelectAccount sets the default for **new**
managed chats on this grant. Existing live conversations retain their exact
profile environment across restarts of their CLI child. Re-authentication of
a profile in use is refused. Selection never moves credentials between machines
or relabels an existing conversation as another account. Usage is shown per
computer binding; duplicated accounts' quotas are never added together.

Every mutation retains its command ID and five-minute intent deadline. A
durable receipt precedes process creation or sending a code. Retrying the same
intent observes its existing flow; an altered request with the same ID refuses.
Receipts retain only a digest, not a code. After a service restart a retained
but unobserved effect is unknown and must not launch/replay automatically.
Read/poll operations never create a new flow. Expiration/cancellation releases
the owned login process. Closing Usage detaches; explicit Cancel cancels.

Source comparison: Zeron checkout `fbf5199a`,
`crates/engine/src/agent_accounts.rs`, `crates/proto/src/plan_usage.rs` and
`crates/ui/src/account_usage.rs`. We reuse its quota interpretation and token
refresh caution. We use independent CLI profile directories in place of its
credential swapping, and Claude Code's own login instead of cloning OAuth.
The official [CLI authentication reference](https://code.claude.com/docs/en/cli-reference)
and [configuration-directory contract](https://code.claude.com/docs/en/env-vars)
describe the installed CLI commands and account isolation.

The Usage surface uses the existing installed `environment.wait` service for
polling; native frames have no animation clock. An account action can queue
behind an observation but never replaces a pending mutation. Sign-in has a
focused native sheet; Back to Usage detaches, Continue reopens it, and Cancel
ends the computer's owned login process. The terminal Coder surface already
uses the shared Claude quota/credential codec; consolidating its older probe
effects is tracked in [#13598](https://github.com/OpenAgentsInc/openagents/issues/13598).
