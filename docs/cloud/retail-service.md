# The retail cloud service

Status: implemented behind the paid-availability gate and tested only with
fake payments, fake providers, and fake task owners. Nothing here is
purchasable until the owner confirms the [retail contract](retail-contract.md)
and the funded qualification passes (`NEEDS_OWNER.md`).

The [`retail-cloud`](../../crates/retail-cloud) crate composes the central
money ledger ([compute balance](compute-balance.md)), the router contract
(`crates/route-contract`), and the receiver wallet's contract into the
retail flow. Each section names the module that implements it.

## Authorities

[`retail_cloud::authority`](../../crates/retail-cloud/src/authority.rs)

A retail admission (`openagents.cloud.retail-admission.v1`) binds the
account, the funded execution, the source and request digest, the computer
and task classes, the retail grant generation, the effects, the material
recipients and disclosed content, the model payer, the price book version
and digest, and the maximum charge. It is a new document bound by digest;
the frozen admission snapshot is unchanged.

Before every side effect, `check` reads the current rights and requires
every authority the step needs:

| Step | Observe | Execute | Disclose | Spend |
| --- | --- | --- | --- | --- |
| Reserve funds | | | | Yes |
| Provision a computer | | Yes | | Yes |
| Dispatch the executor | | Yes | Yes | Yes |
| Read progress, artifacts, or receipts | Yes | | | |
| Cancel or steer | Yes | Yes | | |
| Upload source and the customer's key | | Yes | Yes | |

- **Execute** needs a retail grant for this execution at the admitted
  generation. An operator, pool, auto-start, or pairing grant refuses
  (`not_retail`).
- **Disclose** needs the consent the customer gave for this exact admission
  digest. Consent for an earlier admission does not cover a changed one.
- **Spend** is the principal's spend right, read from the ledger at the
  moment of the step, so a rotated or revoked credential stops spending at
  once.
- **Observe** is a grant for this account and execution.

Pairing, world membership, a balance, and a paid invoice supply none of
these.

A changed recipient, source, computer, effect, payer, or quote is a new
admission: `RetailAdmission::changes` names what moved, and the caller
issues a new offer.

Revocation stops new work and never erases an obligation:

| Stage at revocation | What follows |
| --- | --- |
| Reserved only | Release the hold; no charge |
| Computer provisioned, executor not started | Tear down; no charge |
| Executor started | Request a stop, wait for its acknowledgment, tear down, and settle the measured seconds |
| Unknown | Reconcile before reporting |
