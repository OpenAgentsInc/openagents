# AC-3 broker-authorized owner-capacity receipt

Issue: [#8721](https://github.com/OpenAgentsInc/openagents/issues/8721)  
Parent: [#8547](https://github.com/OpenAgentsInc/openagents/issues/8547)

## Outcome

The owner-subscription no-charge disposition is no longer authorized by a
poster-supplied lane/provider label. After broker materialization, the Agent
Computer includes only the bounded provider-account and grant refs in its
private exact-usage request. Before inserting token truth or deciding whether
to meter, the Worker reads that grant from its own repository and requires:

- status `used` with a non-null redemption time;
- the exact owner user;
- the exact provider account; and
- the provider kind implied by the admitted Codex or Claude lane.

Missing refs, an unknown grant, issued-but-unredeemed or revoked state,
cross-owner authority, wrong account, and wrong provider all return a typed
403 before token insertion or public-counter publication. A repository read
failure returns the existing typed storage 503 and never falls through to a
charge or exemption. Hosted provider-capacity metering is unchanged.

Grant and account refs are request authority only. They are deliberately not
copied into token-event metadata or the public result.

## Verification

- Worker cloud-runtime usage route: 12 passed, 0 failed.
- Agent Computer turn runner: 53 passed, 0 failed, 185 assertions.
- Worker and Pylon TypeScript typechecks passed.
- Tests cover the full authority tuple matrix, denial before insertion/delta,
  unavailable-store failure, Codex/Claude identities, private wire refs,
  public-event redaction, ordinary hosted metering, and exact-once retry.
- `git diff --check` passed before publication.

This closes the parent audit's G1 exemption-authority gap. It does not claim
#8547's live Firecracker/mobile acceptance.
