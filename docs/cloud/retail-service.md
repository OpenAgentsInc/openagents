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

## Offers

[`retail_cloud::offer`](../../crates/retail-cloud/src/offer.rs)

`make_offer` quotes one request for one account and returns a
[`route_contract::Offer`](../../crates/route-contract/src/offer.rs) whose
terms carry the request digest, the retail admission's digest, the computer
class, the effects, the material recipients, the maximum charge, and the
source revision, together with the price book's quote and the proposed
admission. The quote lists every charge line with its payer: compute and
coordination from the purchased balance, and the model on the customer's
own OpenAI key at zero. Sponsored hosted inference is a separate field that
is always `off` for a retail run.

No offer is made, and nothing is reserved, for a typed outcome:

| Outcome | When |
| --- | --- |
| `unsupported` | The source is not a public `github.com` repository at a 40-digit commit, the task text is empty or over 16 KiB, there are not 1 to 8 checks of at most 1,024 bytes, or the wall time is not 1 to 3,600 seconds |
| `unavailable` | Four retail sandboxes already run (`retail_limit`), or the operator plan has no Boat starts left (`plan_limit`) |
| `price` | The price book refused: an unknown class, seconds out of bounds, or a maximum above the customer's ceiling |

An offer stays confirmable for 10 minutes. `confirm` accepts only the offer's
own control; shell-command approval and input routing refuse
(`not_an_offer_control`). It re-quotes the request from the current book and
rebuilds the admission, and refuses an expired offer (`expired`), another
digest (`mismatch`), or any changed price, source, recipient, or provider
(`changed`). The first confirmation records exactly one funded request with
its execution identity in the journal; a repeated confirmation, even after a
restart, returns that same funded request and reserves nothing more.
