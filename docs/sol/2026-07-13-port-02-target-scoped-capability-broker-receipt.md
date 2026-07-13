# PORT-02 target-scoped capability broker receipt

- Issue: [#8747](https://github.com/OpenAgentsInc/openagents/issues/8747)
- Packet: PORT-02 of the remote-first portable coding-session pathway
- Requires: closed PORT-00 #8745
- Consumed by: PORT-03 #8748
- Contract: `openagents.portable_capability_broker.v1`

## Landed broker

`PortableCapabilityBroker` implements the general capability lifecycle over
PORT-00's unchanged `PortableCapabilityLease` schema. A lease binds one owner,
session, attachment generation, target, provider/SCM/tool/API capability,
optional named account/tool, least-privilege permissions, and bounded TTL.

Issue, redeem, renew, revoke, reissue, release, and wipe produce durable
refs-only outcomes and evidence. Exact lost-ACK replay returns the stored
outcome without repeating an effect; conflicting bytes fail closed. Reissue
revokes the source grant and wipes the source target before it can issue a
strictly newer destination attachment lease, and it requires a fresh
destination source-grant ref rather than copying revoked authority.

The target boundary is an injected, class-checked adapter. The acceptance
oracle runs actual material callbacks through both an `owner_local` adapter and
the accepted `openagents_managed` adapter shape, proves installation, revokes
and wipes the source, reissues to the managed target, then redeems there. This
is the bounded integration seam PORT-03 composes into the real move; it does not
claim a session or provider process moved during PORT-02.

## Secret and threat boundary

The broker stores no raw material. The injected vault owns source grants and
may expose bytes only inside one target redemption callback; the fixture zeros
the transient buffer after use. Broker snapshots exclude the opaque
source-grant ref as well as material. Evidence carries `material: excluded`.

The adversarial export gate serializes broker snapshots, Sync-like lease rows,
checkpoint metadata, prompts, logs, diagnostics, public receipts, and artifacts
and proves that neither the canary secret, the source-grant ref, nor common
credential fields appear.

## Fault acceptance

Focused Effect tests prove:

- provider, SCM writeback, MCP/tool, and bounded API least-privilege scope;
- owner-local redemption and local→managed revoke/wipe/reissue/redemption;
- exact lost-ACK replay and conflicting replay denial;
- renewal plus expiry-triggered source revoke/target wipe;
- mid-move cleanup failure leaves the source revoked and creates no
  destination lease;
- revoked/released/expired source leases cannot replay;
- target denial, unready target, adapter mismatch, broker outage, and wipe
  failure fail closed with redacted evidence;
- explicit revoke, wipe, and release outcomes reconcile independently; and
- missing account/tool scope, empty permissions, invalid refs, and excessive
  TTL fail before grant creation.

## Documentation and policy

- Root and Cloud invariant ledgers now name the broker authority and its
  production boundary.
- The operator runbook fixes lifecycle order, movement composition, failure
  recovery, scan gates, and adapter-admission requirements.
- `openagents_cloud.brokered_session_secrets.v1` advances from pending to
  `enforced` at the test-sweep tier and points at the executable oracle.

No migration or deployment was required: PORT-00 already froze the complete
lease scope and PORT-01 already persists capability lease refs on attachment
authority. PORT-02 deliberately retains transient broker lifecycle state behind
the injected vault/adapter boundary; no new command, catalog, route, Sync row,
or credential-bearing database field was introduced.

## Verification

- `bun test --cwd packages/portable-session-contract`
- `bun run --cwd packages/portable-session-contract typecheck`
- `bun test --cwd packages/behavior-contracts`
- `bun run --cwd packages/behavior-contracts typecheck`
- Sol documentation checks and `git diff --check`

## Honest boundary

PORT-02 proves the general broker lifecycle, local/managed adapter redemption,
source revocation, destination reauthorization, target wipe, replay/fault
behavior, and non-projection of raw material. PORT-03 still must move one real
session with a child between owner-local Pylon and the accepted Agent Computer
and back while preserving session/graph/cursor identity and one live attachment.
