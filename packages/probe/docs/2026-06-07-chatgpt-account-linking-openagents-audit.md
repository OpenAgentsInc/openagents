# ChatGPT Account Linking And OpenAgents product surface-Connected Probe Auth Audit

Date: 2026-06-07

Status: architecture audit for connecting Probe to OpenAgents product surface-managed
ChatGPT/Codex accounts.

## Intended End State

Probe should be able to run coding-agent work with every relevant ChatGPT/Codex
account that a user intentionally connects to OpenAgents. OpenAgents product surface should be the
account registry, policy surface, lease authority, and grant issuer. Probe
should consume scoped runtime grants and materialize authentication only inside
the run environment that needs it.

This means Probe does not become the long-lived source of truth for ChatGPT
identity. A local Probe installation, an SHC box, or a sandbox spawned from an
openagents.com forum thread should receive references and short-lived grants
from OpenAgents product surface, not raw account credentials in launch payloads, logs, or public
metadata.

## Scope And Vocabulary

The product/runtime name is `probe`. Do not introduce implementation-history
names such as `probe_bun_effect`.

The provider name already modeled in OpenAgents product surface is `chatgpt_codex`. This is the
right provider key for ChatGPT subscription-backed Codex-style coding-agent
access. The current OpenAgents product surface code also carries a few OpenCode-shaped names in its
materialization plan. Those names should be treated as transitional and renamed
to Probe-neutral or Probe-specific contract names during the refactor.

"All relevant ChatGPT accounts" cannot mean silent discovery of every account
a person may own at OpenAI. The workable product contract is: every account the
user or operator intentionally connects through the OpenAgents/OpenAgents product surface linking
flow is recorded as a provider account, made visible in the account list, and
eligible for policy-driven Probe assignment when healthy.

## Current OpenAgents product surface Account Model

OpenAgents product surface already has a strong provider-account substrate in
`openagents/packages/provider-account-schema/src/index.ts`.

The current public provider is `chatgpt_codex`. Provider accounts carry stable
refs, user/team scope, labels, plan type, status, health, auth mode, secret
refs, and public metadata. The status model covers `pending`, `connected`,
`expired`, `denied`, `disconnected`, and `unhealthy`. Health is tracked
separately as `unknown`, `healthy`, `unhealthy`, or `requires_reauth`.

The same schema defines connection attempts, session grants, and events. Auth
modes include `chatgpt_device_code`, `codex_device_auth`, and
`manual_secret_ref`. Connection attempts are currently modeled around
`chatgpt_device_code`, with sources including `worker_device_code`,
`shc_broker`, and `manual_placeholder`.

The schema is intentionally strict about public projection hygiene. Public
secret refs may use prefixes such as `secret://`, `vault://`,
`provider-account://`, and `codex-auth://`, while raw bearer tokens, OpenAI API
keys, OAuth fields, auth JSON, JWT-looking values, and other secret markers are
rejected or redacted from public metadata.

## Current OpenAgents product surface API Surface

OpenAgents product surface currently exposes user-facing provider account routes in
`openagents/workers/api/src/provider-account-routes.ts`:

- `GET /api/provider-accounts`
- `POST /api/provider-accounts/chatgpt-codex/device-login/start`
- `GET /api/provider-accounts/chatgpt-codex/device-login/:attemptId`
- `POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/connected`
- `POST /api/provider-accounts/chatgpt-codex/device-login/:attemptId/failed`
- `POST /api/provider-accounts/:providerAccountRef/health`
- `POST /api/provider-accounts/:providerAccountRef/grants`
- `POST /api/provider-accounts/chatgpt-codex/grants/resolve`
- `POST /api/provider-accounts/:providerAccountRef/disconnect`

The service layer in
`openagents/workers/api/src/provider-account-service.ts` already supports
the important lifecycle:

- list a user's accounts and pending connection attempts
- start a ChatGPT/Codex device-login attempt
- update a pending attempt after polling
- record a connected account with a public `codex-auth://...` style secret ref
- mark failed, denied, expired, or reauth-required attempts
- issue session-scoped auth grants for connected healthy accounts
- resolve grants into a redacted materialization plan
- record health updates and disconnections

The device login client in
`openagents/workers/api/src/provider-account-client.ts` starts device
auth against OpenAI auth endpoints, shows the user a verification URL and user
code, polls for completion, exchanges the returned authorization code, and
extracts account labels/ids from token claims when available. In product terms,
this means OpenAgents product surface can already perform the core "connect this ChatGPT account"
ceremony without Probe owning the OAuth flow.

## Current OpenAgents product surface Persistence

OpenAgents product surface's D1 migrations already encode the durable tables needed for a multi
account fleet:

- `provider_accounts`
- `provider_account_connection_attempts`
- `provider_account_auth_grants`
- `provider_account_events`
- `runner_sessions`
- `provider_account_leases`

Later migrations add provider-account sanity checks, parallel Probe receipt
fields, lease operations, account fleet fields, and failover receipt events.

The lease/fleet columns are important for "all accounts" support. Accounts can
carry operator priority, cooldown, low-credit flags, recent failure class,
lease limits, selection timestamps, launch timestamps, reauth reasons, operator
notes, and refill notes. That is already the shape of an account fleet, not a
single-account login button.

## Current OpenAgents product surface UI State

OpenAgents product surface's web UI already treats ChatGPT accounts as a settings connection
surface. The page examples and tests under `openagents/apps/web` include
copy and states for:

- "ChatGPT accounts"
- "Add ChatGPT account"
- multiple connected accounts with primary/backup style display
- per-account reconnect and refresh flows
- a device-code prompt that sends the user to OpenAI's device page
- launch blocking when ChatGPT is not connected or requires reconnect

This is the right product direction for Probe. The first milestone should not
be a separate Probe-only login island. The user should be able to connect
accounts in OpenAgents product surface, see them there, and have Probe runs consume those accounts
through refs and grants.

## Current Run Assignment Contract

OpenAgents product surface run assignment code already carries provider auth references. In
`openagents/workers/api/src/omni-runs.ts`, run assignments can include
`providerAccountRef` and `authGrantRef`. The SHC control request passes those
refs through with runtime, repo, goal, callback, token ref, and sandbox
metadata.

The runner gateway enforces the right security boundary:
runner payloads must carry refs and grants, not raw credentials, logs, source
archives, wallet material, or customer private data.

For Probe, this should become the runtime contract:

1. OpenAgents product surface selects or receives a requested `providerAccountRef`.
2. OpenAgents product surface issues an `authGrantRef` for the run, thread, workroom, runner
   session, or lease.
3. The run assignment sent to SHC/Pylon/Probe includes only those refs.
4. Probe resolves the grant through OpenAgents product surface from the runner side.
5. The secret broker materializes auth only inside the per-run sandbox.
6. Probe scrubs the materialized auth and emits a closeout receipt.

## Current Fleet And Lease Model

OpenAgents product surface's operator provider-account routes already describe the fleet behavior
Probe needs. The operator surface can start device login, poll device login,
run sanity checks, create leases, issue lease-bound grants, handle failover,
explain leases, list active leases, touch leases, release leases, and view a
fleet dashboard.

The lease selector expires stale leases, filters for connected healthy accounts
with a secret ref, avoids low-credit and cooldown accounts, respects lease
limits, and orders accounts by active lease count, operator priority, recency,
and provider account ref. This is the correct policy authority for multi
account Probe deployment.

Probe should not independently pick from a bag of raw ChatGPT tokens. Probe
should ask OpenAgents product surface for the account or failover route, then operate with the grant
it receives.

## Deprecated Probe Auth Source Material

The archived Probe history contains useful patterns, but it should not be
restored as the authority. At commit `2d82d44`, Probe had documentation and
Rust code for subscription-backed OpenAI/Codex auth:

- `docs/54-openai-codex-subscription-auth.md`
- `crates/probe-openai-auth/src/lib.rs`

That design stored local state under `PROBE_HOME/auth/openai-codex.json`, with
multiple accounts, labels, refresh/access token material, expiry timestamps,
selected account key, and cached usage/rate-limit snapshots. It supported a
browser PKCE flow and a headless device-code flow. It also described ranking
accounts by remaining headroom, rotating on rate limit, and falling back to a
local `PROBE_OPENAI_API_KEY`.

Those are good implementation patterns to harvest:

- multi-account status reporting
- per-account refresh and health checks
- usage/headroom telemetry
- rate-limit-aware failover
- API-key fallback as a separate inference route
- headless device flow for SSH-only or worker environments

The authority should change. Long-lived account records and secret refs should
live in OpenAgents product surface. Probe should keep only short-lived per-run materialization state
unless the user explicitly links a local Probe installation for local-only
work.

## Pylon Account Linking Analogy

The OpenAgents/Pylon account-linking docs model a separate but relevant
relationship. Pylon linking proves that a local node identity is associated
with a web account. It uses a one-time token plus a signed proof, and it keeps
the payload web-safe: runtime capability snapshots are allowed, but local
credential paths, raw bridge secrets, Codex tokens, local roots, and private
material are not.

Probe should follow the same separation:

- Probe host identity linking proves which local installation, SHC box, or
  sandbox runner is allowed to talk to OpenAgents product surface.
- ChatGPT/Codex provider account linking proves which OpenAI account the user
  intentionally connected through OpenAgents product surface.
- A Probe host identity is not a ChatGPT account.
- A ChatGPT account is not a Pylon identity.
- A run grant binds the two at execution time for a narrow purpose.

That distinction matters for SHC boxes and forum-thread sandboxes. The runner
can prove it is an authorized Probe surface, then receive a scoped grant for a
specific run. It should not inherit durable user ChatGPT credentials simply
because it is linked.

## Proposed Account Linking Flow

The first working version should be OpenAgents product surface-first.

User flow:

1. The user opens OpenAgents product surface/OpenAgents settings and goes to ChatGPT accounts.
2. The user clicks "Add ChatGPT account".
3. OpenAgents product surface starts a device login attempt with `createNew=true` or with an
   explicit account ref when reconnecting.
4. The user opens the OpenAI device page, signs into the intended ChatGPT
   account, and enters the code.
5. OpenAgents product surface polls the attempt and stores the connected auth in secret storage
   behind a public secret ref such as `codex-auth://<providerAccountRef>`.
6. OpenAgents product surface lists the account with label, plan, status, health, and operator
   metadata.
7. The user repeats the same flow for every relevant ChatGPT account.

Probe run flow:

1. A forum thread, workroom, operator action, or Pylon request asks OpenAgents product surface to
   launch Probe.
2. OpenAgents product surface chooses a provider account by explicit request or lease policy.
3. OpenAgents product surface issues a session/run-scoped grant bound to the selected account and
   runner session.
4. The SHC/Pylon control request carries `providerAccountRef` and
   `authGrantRef`, not tokens.
5. Probe resolves the grant through OpenAgents product surface after proving runner identity.
6. Probe receives a redacted materialization plan and a secret ref.
7. The local secret broker materializes the actual auth into an isolated
   per-run Probe home or environment.
8. Probe runs the coding-agent task.
9. Probe reports health, low-credit, rate-limit, failure, or success receipts
   back to OpenAgents product surface.
10. Probe scrubs the materialized auth on closeout.

Local Probe management flow:

1. `probe openagents link` links a local Probe installation to a user, team, Pylon,
   or SHC identity using a Pylon-style signed account-link proof.
2. `probe auth accounts` lists OpenAgents product surface-connected account refs and health for the
   linked scope without printing raw tokens.
3. `probe auth add chatgpt` can call the same OpenAgents product surface device-login start/poll
   routes for users who prefer the CLI, but the resulting account still lives
   in OpenAgents product surface.
4. `probe auth import` may exist later for explicit migration from old local
   Probe or Codex auth files, but it must upload or broker the credential into
   OpenAgents product surface secret storage and then delete or quarantine the imported local
   material if the user chooses.

## Grant Materialization Contract

OpenAgents product surface currently returns a redacted materialization plan with OpenCode-shaped
names. Probe should refactor this into a Probe contract. The exact names can be
settled during implementation, but the contract should look like this:

- provider: `chatgpt_codex`
- provider account ref
- grant ref
- provider secret ref
- requested action
- runner session id
- expiration timestamp
- materialization target: per-run Probe home, env var, or adapter auth file
- scrub policy: scrub after closeout

Good candidate names:

- `ProbeAuthMaterializationPlan`
- `PROBE_CHATGPT_AUTH_CONTENT`
- `PROBE_PROVIDER_ACCOUNT_REF`
- `PROBE_AUTH_GRANT_REF`

If a downstream adapter requires Codex-compatible `auth.json`, that should be
an adapter-specific materialization target inside the run directory, not the
global account authority.

## Account Selection And Failover

OpenAgents product surface should own cross-account selection. Probe should own local execution and
reporting.

Selection policy should support:

- explicit account ref chosen by the user or operator
- automatic lease selection across all healthy connected accounts
- team-scoped and user-scoped accounts
- labels and operator notes
- operator priority
- lease limits
- cooldown windows
- low-credit flags
- recent failure class
- last selected timestamp
- account health and reauth status

Probe should report signals back to OpenAgents product surface:

- access token failed
- refresh failed
- account requires reauth
- account appears low credit or out of usage headroom
- rate limited
- provider unavailable
- run succeeded
- run failed for non-auth reasons
- auth was scrubbed at closeout

When a run hits account-level failure, Probe should request failover through
OpenAgents product surface's lease/failover route. It should not locally iterate across every raw
account secret.

## Security Requirements

The account linking process must preserve these boundaries:

- No raw ChatGPT/OAuth tokens in run assignments.
- No raw ChatGPT/OAuth tokens in public metadata.
- No raw ChatGPT/OAuth tokens in logs, forum threads, receipts, or gateway
  payloads.
- Long-lived provider account secret material is stored behind OpenAgents product surface secret
  refs.
- Grants are short-lived, one-time or narrowly scoped, and runner-session
  bound.
- Disconnecting or revoking an account invalidates active and future grants.
- Expired or unhealthy accounts require reconnect before selection.
- SHC boxes and Pylon nodes prove runner identity separately from provider
  account ownership.
- Per-run materialized auth lives in a sandbox-local path or env surface and
  is scrubbed after closeout.
- Probe can use local/swarm/API-key inference strategies, but those are
  different routes from ChatGPT subscription-backed provider accounts.

## Required Refactors

OpenAgents product surface:

- Rename OpenCode-shaped materialization types and env hints to Probe-neutral
  names.
- Ensure `Add ChatGPT account` always creates a distinct provider account when
  requested, instead of silently reusing an existing account.
- Make account labels editable enough for users/operators to distinguish every
  relevant ChatGPT account.
- Confirm the grant resolve route is available to authorized Probe runners and
  not only browser sessions.
- Bind grant resolution to runner identity, assignment id, lease ref, or
  runner session id.
- Add explicit account-import endpoints only if migration from old local Probe
  or Codex auth is needed.
- Promote usage/headroom/sanity results into account health and fleet fields.
- Add receipts for materialized, scrubbed, failed-refresh, rate-limited,
  low-credit, and reauth-required events.

Probe:

- Implement an OpenAgents product surface account client.
- Implement a grant resolver that accepts `providerAccountRef` and
  `authGrantRef` from the run assignment.
- Implement a Probe auth materializer that writes only to the per-run
  sandbox/home.
- Implement scrub-on-closeout and receipt emission.
- Implement account status commands that read OpenAgents product surface public account projections.
- Implement optional CLI-driven account add/reconnect by calling OpenAgents product surface's
  device-code routes.
- Do not recreate the old long-lived multi-account store as the primary
  authority.

Pylon/SHC:

- Link Probe runner identity to OpenAgents product surface using a signed account-link process.
- Pass run assignments with account refs and grants.
- Provide a local broker for secret-ref materialization.
- Keep raw provider auth out of Pylon link payloads.
- Emit closeout and scrub receipts back to OpenAgents product surface.

## Path To Parity With Deprecated Probe Auth

Parity should be measured by user-visible capability, not by restoring old file
formats.

Milestone 0: contracts and fixtures.

- Document the Probe account/grant assignment shape.
- Add fake grant fixtures with no real tokens.
- Add redaction tests around every public projection and receipt.

Milestone 1: OpenAgents product surface multi-account connect/list.

- Use the existing OpenAgents product surface device-code flow.
- Verify repeated "Add ChatGPT account" creates multiple visible accounts.
- Verify reconnect updates only the selected account.
- Verify disconnect prevents grant issuance.

Milestone 2: Probe grant consumption.

- Build the Probe-side client that resolves a fake OpenAgents product surface grant.
- Materialize fake auth into a temporary per-run home.
- Run a no-provider smoke using the materialized path.
- Scrub and prove the file/env is gone after closeout.

Milestone 3: SHC/Pylon launch.

- Send `providerAccountRef` and `authGrantRef` in a Probe run assignment.
- Require runner identity proof before grant resolution.
- Verify sandbox execution cannot see unrelated account refs or credentials.

Milestone 4: fleet selection and failover.

- Use OpenAgents product surface leases to select among all connected accounts.
- Feed Probe auth failures back into account health.
- Request failover through OpenAgents product surface instead of local token iteration.
- Add account-level cooldown, low-credit, and reauth-required receipts.

Milestone 5: local Probe account management.

- Add `probe openagents link`.
- Add `probe auth accounts`.
- Add optional `probe auth add chatgpt` as a CLI wrapper around OpenAgents product surface's
  device-code flow.
- Add explicit import from deprecated local Probe/Codex auth files only after
  the OpenAgents product surface secret-storage path is ready.

Milestone 6: inference strategy integration.

- Treat ChatGPT/Codex account grants as one inference strategy.
- Keep API-key providers, local inference, swarm inference, and Codex-style
  adapters in the same Probe surface.
- Route each job through OpenAgents product surface policy so account-backed, API-key-backed,
  local, and swarm execution are explicit and auditable.

## Test And Audit Checklist

OpenAgents product surface tests:

- multiple ChatGPT accounts can be connected for the same user/team
- reconnect updates the intended account
- grant issuance fails for disconnected, unhealthy, expired, or secretless
  accounts
- grant resolution fails after expiration or runner-session mismatch
- public projections reject raw token material
- account health changes affect lease selection
- failover skips cooldown, low-credit, and exhausted accounts

Probe tests:

- run assignment parser accepts `providerAccountRef` and `authGrantRef`
- grant resolver calls OpenAgents product surface and rejects mismatched account refs
- materializer writes fake auth only inside the per-run directory
- cleanup removes materialized auth
- logs and receipts redact auth content
- health reporter sends rate-limit, low-credit, refresh-failed, and
  reauth-required signals

Pylon/SHC tests:

- linked runner identity can resolve an assigned grant
- unlinked runner identity cannot resolve a grant
- link payloads do not include provider credentials
- sandbox closeout emits scrub receipts

## Immediate Recommendation

Implement the first Probe milestone around OpenAgents product surface grant consumption, not a new
standalone Probe login store. The first working build should accept an OpenAgents product surface
assignment containing `providerAccountRef` and `authGrantRef`, resolve it,
materialize a fake auth fixture in a per-run directory, scrub it, and report a
receipt. In parallel, OpenAgents product surface should verify that its existing settings flow can
connect more than one ChatGPT account and issue a grant for the selected
account.

After that works end to end, add CLI convenience commands that call OpenAgents product surface's
same APIs. That gives users a Probe-native surface without splitting account
authority away from the openagents.com/OpenAgents product surface product.

## Source Files Reviewed

- `openagents/packages/provider-account-schema/src/index.ts`
- `openagents/workers/api/src/provider-account-routes.ts`
- `openagents/workers/api/src/provider-account-service.ts`
- `openagents/workers/api/src/provider-account-domain.ts`
- `openagents/workers/api/src/provider-account-client.ts`
- `openagents/workers/api/src/operator-provider-account-routes.ts`
- `openagents/workers/api/src/omni-runs.ts`
- `openagents/workers/api/src/runner-gateway.ts`
- `openagents/workers/api/src/provider-launch.ts`
- `openagents/workers/api/migrations/0009_provider_accounts.sql`
- `openagents/workers/api/migrations/0045_provider_account_parallel_probe_receipts.sql`
- `openagents/workers/api/migrations/0046_provider_account_leases.sql`
- `openagents/workers/api/migrations/0048_provider_account_fleet_fields.sql`
- `openagents/workers/api/migrations/0049_provider_account_lease_operations.sql`
- `openagents/workers/api/migrations/0050_provider_account_failover_receipt_events.sql`
- `openagents/docs/pylon/PYLON_ACCOUNT_LINKING_NIP98.md`
- `openagents/docs/pylon/PYLON_ACCOUNT_LINKING_PROCESS.md`
- deprecated Probe history at commit `2d82d44`:
  `docs/54-openai-codex-subscription-auth.md`
- deprecated Probe history at commit `2d82d44`:
  `crates/probe-openai-auth/src/lib.rs`
