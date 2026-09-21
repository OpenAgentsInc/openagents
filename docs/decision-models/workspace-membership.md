# Workspace membership

`tenancy::accounts` is the membership half of the serving side. The
registry binds a tenant to its doors; the account store binds people to
the tenant they act for. It answers three questions: which workspaces an
account belongs to, what its role there permits, and whether a
membership that authorized a session still stands.

The store is `accounts.json`, beside `registry.json`, `keys.json`, and
`quota-ledger.jsonl` in the directory the gateway already reads.

## The model

Three record kinds make up the store:

- **Account** — a stable identity, `acct_<hex>`. An account carries
  principal *references* — `key:<id>` for an `oak_` key, `nostr:<hex>`
  for a relay principal — never a secret. Authentication stays with
  `tenancy::keys`; the account store resolves what an authenticated
  principal belongs to.
- **Workspace** — `ws_<hex>`, personal or organization. The id never
  changes: a rename, a seat change, and every membership write leave it
  standing. `tenant` names the manifest tenant the workspace's quota and
  billing bind to, and membership changes never touch it.
- **Membership** — the join: an account, a role, a status, and an epoch.
  A revoked membership is a record, not an absence — authorization can
  name `revoked` rather than answering `unknown`.

Invitations are the fourth record: `inv_<id>.<secret>` tokens, stored as
the secret's SHA-256 digest only — the same discipline `keys.rs` keeps
for API keys. An invitation expires at `expires_unix`, grants `admin` or
`member` (never `owner`), and is single-use: the first acceptance
consumes it, and every later presentation is a replay that refuses with
`invitation-closed`.

## The permission matrix

Every mutation names the acting account and is authorized against its
active membership before anything is read or written.

| Action | Owner | Admin | Member |
| --- | --- | --- | --- |
| Call `authorize` — belong to the workspace | ✓ | ✓ | ✓ |
| Invite or revoke an invitation | ✓ | ✓ | — |
| Remove a member | Any other member | Members only | Self only |
| Set a role between `admin` and `member` | ✓ | — | — |
| Transfer ownership | ✓ | — | — |
| Set seats, rename, rebind the tenant | ✓ | — | — |

Two rules sit outside the table because they are invariants, not
permissions:

- **Exactly one owner.** A workspace stands under exactly one active
  owner. Ownership moves only through `transfer_ownership`, which makes
  the target the owner and the caller an admin in one committed
  revision — the workspace never passes through zero or two owners.
  Removing the owner, demoting the owner, and granting `owner` by
  invitation or role write all refuse.
- **Personal workspaces hold their owner alone.** Invitations, role
  changes, transfers, and seat writes refuse on a personal workspace.

Seats bound members plus live invitations: a pending, unexpired
invitation holds its seat until it is accepted, revoked, or expires.
`set_seats` refuses a bound below the active membership rather than
orphaning members the next read would refuse to load.

## The call flow a future HTTP adapter runs

No HTTP exists today. When an adapter lands in front of this store, a
request runs the same sequence the gateway already runs for decision
calls:

1. **Authenticate.** `Authorization: Bearer oak_<id>.<secret>` resolves
   through `tenancy::keys::authenticate` to a key id. The adapter forms
   the principal `key:<id>` and resolves it through
   `Accounts::account_of_principal` to an account. A request with no
   resolvable principal is unauthenticated — there is no anonymous
   membership.
2. **Authorize the actor.** For a read, `Accounts::authorize` or
   `Accounts::authorize_principal` answers the [`MemberRef`] — role,
   membership epoch, workspace epoch — or a typed refusal:
   `not-member`, `revoked`, `unknown-workspace`. For a mutation, the
   mutation calls (`invite`, `accept`, `set_role`, `remove_member`,
   `transfer_ownership`, `set_seats`, `rename`, `rebind_tenant`) run the
   same check inside the write and refuse before anything changes.
3. **Serialize the write.** `Accounts::mutate` takes `accounts.lock`,
   re-reads the store inside it, applies the change, bumps `sequence`,
   chains `supersedes`, reseals the digest, revalidates, archives the
   revision under `accounts-history/<digest>.json`, and renames the new
   store into place. The second of two competing writers decides against
   the winner's committed state, never a stale read. A writer that
   cannot take the lock inside the retry bound gets
   `Trouble::Locked` — the adapter maps it to a retryable response.
4. **Answer.** A mutation returns the changed record; the adapter maps
   `Refusal` variants to status codes the way the gateway maps its own —
   `forbidden`, `seat-limit`, `last-owner`, `invitation-expired`, and
   the rest are already distinct answers.

A session or cached authorization carries the `MemberRef` epochs it was
issued under. Comparing `members_epoch` on the next `authorize` tells
the caller its view moved; the per-membership `epoch` moves when that
membership is granted, re-roled, or revoked. Because every query
re-reads the committed store, a revocation is visible to the very next
authorization on any handle — nothing needs to expire.

## What the store guarantees

- **Versioned.** `v` names the schema (`openagents.tenancy.accounts.v1`),
  `sequence` counts revisions, `supersedes` chains each revision to the
  digest it replaced, and every sealed revision stays archived by
  digest.
- **Fail closed.** A missing file, a torn write, an unknown schema, a
  digest that does not recompute, a membership pointing at no account, a
  workspace without exactly one owner — each refuses the whole store
  rather than serving the part that parsed.
- **No secrets at rest.** Invitation tokens and API keys persist as
  digests. The token leaves the store once, in the response that issued
  it.

## Limitations

- **There is no onboarding.** No sign-in, sign-up, password, or recovery
  flow exists, and none is claimed. `create_account` and
  `create_workspace` are operator calls; invitation delivery is the
  inviter's problem, out of band.
- **The tenant binding is a reference.** `Workspace::tenant` records
  which manifest tenant quota and billing bind to; whether that tenant
  exists or what it may reach is the registry's check, not this store's.
- **One file, one lock.** The store is a single document serialized on a
  single advisory lock. That is the right shape for the gateway's
  registry directory; it is not a multi-region membership service.
- **Principals are managed, not derived.** Binding a freshly issued
  `oak_` key to an account is a separate operator step —
  `update_principals` — so a key's lifecycle and an account's membership
  stay independent: rotating a key does not move the membership, and
  revoking a membership does not touch the key.
- **Acceptance trusts the bearer.** Whoever presents a live invitation
  token joins. The store cannot tell a forwarded token from a delivered
  one; short TTLs and single-use semantics are the mitigation.

[`MemberRef`]: ../../crates/tenancy/src/accounts.rs
