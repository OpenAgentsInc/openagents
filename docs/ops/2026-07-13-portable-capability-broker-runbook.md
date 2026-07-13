# Portable capability broker operator runbook

- Contract: `openagents.portable_capability_broker.v1`
- Implementation:
  `packages/portable-session-contract/src/capability-broker.ts`
- Issue: [#8747](https://github.com/OpenAgentsInc/openagents/issues/8747)
- Scope: PORT-02 broker lifecycle; PORT-03 owns the first real host move

## Operating boundary

The broker is a target-scoped capability state machine, not a secret store or
generic tunnel. A caller supplies:

- the existing PORT-00 lease scope: owner, session, attachment, generation,
  target, capability, optional account/tool, and expiry;
- one nonempty least-privilege permission set;
- an opaque source-grant ref owned by an injected vault; and
- a target binding whose class matches a registered adapter.

The broker stores no material. The vault may expose material only for the
duration of `withSourceGrantMaterial`; the adapter installs it into its bounded
target scratch boundary, and the vault zeros or destroys the transient buffer
after the callback. Neither the broker snapshot nor its evidence sink receives
the source-grant ref or material.

Supported capabilities are provider, SCM read, SCM writeback, MCP/tool, and a
bounded named API. `tool` and `api` require an exact `toolRef`; provider requires
an exact named `accountRef`. Empty permissions, missing scope, unready targets,
adapter-class mismatch, expired leases, and TTL above the configured maximum
are refused.

## Lifecycle

1. **Issue:** validate the exact target and least-privilege scope, then create
   an `issued` ref-only lease.
2. **Redeem:** revalidate target readiness and expiry, ask the vault for the
   exact source grant, install through the class-matched adapter, and move to
   `redeemed` only after installation succeeds.
3. **Renew:** issue a new bounded expiry on the same active scope. Renewal does
   not change target, attachment generation, permissions, account, or tool.
4. **Revoke:** mark the lease locally non-active before asking the vault to
   revoke the source grant. Broker outage retains fail-closed local denial and
   a redacted failed outcome.
5. **Wipe:** call the exact target adapter with lease/target/attachment refs and
   its opaque installation ref. Cleanup failure remains visible and blocks
   reissue.
6. **Reissue:** revoke source, wipe source target, require a freshly authorized
   destination source-grant ref, require a strictly newer attachment
   generation, then issue the new destination lease. It does not redeem the
   new lease implicitly.
7. **Release:** revoke, wipe, then record terminal `released`.

Every externally retried operation uses one stable `operationRef`. Repeating
the exact bytes returns the stored result without repeating the effect. Reusing
the ref with different bytes is `conflicting_replay` and performs no mutation.

## Move procedure

PORT-03 movement composition must use this order:

1. Quiesce the complete source graph through portable-session authority.
2. Seal and verify the secret-free checkpoint.
3. Revoke every source attachment lease.
4. Wipe every source target installation and retain its evidence ref.
5. Ask the owning provider/SCM/tool/API authority for fresh destination grant
   refs. Never reuse the revoked source grant ref.
6. Reissue leases for the new attachment generation and explicit destination.
7. Redeem them at the destination adapter.
8. Activate the destination attachment only after all required redemptions
   complete.

Any failure before step 8 leaves the destination unable to accept work. Never
copy an auth home, token cache, `.env`, process environment, provider-native
session, credential helper output, socket, or live process state.

## Failure and reconciliation

| Condition | Required result |
| --- | --- |
| Lost acknowledgement | Retry the identical operation ref; receive `replayed`; do not repeat the effect |
| Conflicting replay | Reject `conflicting_replay`; retain the original result |
| Expiry | Mark non-active, revoke, wipe, and refuse redemption |
| Mid-move revocation | Source remains non-active; destination is not issued until source wipe passes |
| Target denial/mismatch | Refuse redemption; keep lease unredeemed |
| Vault/broker outage | Fail closed with `broker_unavailable`; do not install material |
| Wipe failure | Record `cleanup_failed`; do not mint destination authority |

Operator recovery is retry-by-exact-operation-ref for an unknown response,
followed by inspection of refs-only evidence. Do not inspect, paste, or log raw
material. A cleanup failure requires target-specific remediation plus a new
wipe operation ref; it never justifies activating the destination.

## Threat model and scan gate

Threats covered by the executable oracle include cross-target reuse, stale
generation replay, excessive TTL, ambient/default account selection, target
class substitution, lost ACK, revoked-source replay, broker outage, target
denial, failed wipe, and leakage through serialized broker state.

Before accepting a new adapter, serialize these surfaces and scan them with a
known canary secret: broker snapshot, Sync lease rows, checkpoint metadata,
prompts, normal logs, diagnostics, public receipts, and artifacts. The canary,
source-grant refs, and credential-shaped fields must be absent. Evidence must
carry `material: excluded`.

Run the gate:

```sh
bun test --cwd packages/portable-session-contract
bun run --cwd packages/portable-session-contract typecheck
bun test --cwd packages/behavior-contracts
bun run --cwd packages/behavior-contracts typecheck
```

The fixture adapters prove the real callback boundary for `owner_local` and
accepted `openagents_managed` target classes. They do not claim a provider
process or coding session moved. That later proof must retain the same source
revoke, destination reauthorization, wipe, and evidence sequence.
