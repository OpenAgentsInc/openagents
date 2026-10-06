# The purchased compute balance

Status: implemented behind the retail gate. Paid availability stays off until
the owner confirms the [retail contract](retail-contract.md) and the funded
qualification passes (`NEEDS_OWNER.md`). Every test here uses fake payments.

The purchased compute balance is one customer account in the central money
ledger, [`pay_ledger::compute`](../../crates/pay-ledger/src/compute.rs),
beside the settlements it pays. No client keeps a balance or a payment
ledger of its own: the native window, the Grid workshop, the CLI, an API
key, and a phone each read the same account.

## Accounts and principals

| Record | What it holds |
| --- | --- |
| Account | An account ID and its creation time. |
| Principal | One client's binding to one account: its kind (`window`, `workshop`, `cli`, `api_key`, or `phone`), the SHA-256 digest of its credential, a generation, a read right, a spend right, and an optional revocation time. |

- **One account per principal, for good.** Binding a principal again with
  the same terms returns it unchanged; binding it to another account
  conflicts.
- **Rotation.** `rotate_principal` replaces the credential digest and moves
  the generation on. The old credential stops resolving at once.
- **Revocation is final.** A revoked principal never reads, spends, or
  rotates again; the client binds again under a new principal ID.
- **No secrets.** The ledger stores credential digests, never credentials.

## Four authorities stay separate

The balance holds exactly two rights per principal: reading and spending.
The other two retail authorities are elsewhere, and none implies another:

| Authority | Where it is granted |
| --- | --- |
| Observation of a task | The task's observe grant |
| Execution on a rented computer | A retail grant for one funded execution (`retail_cloud::authority`) |
| Disclosure of source and task to a provider | The admitted offer's disclosure terms |
| Spending the balance | The principal's spend right, checked at reservation |

Pairing a device, joining a world, holding a balance, or paying an invoice
grants none of them.

## Top-ups

A top-up buys credits over Lightning through the existing receiver wallet
([`retail_cloud::topup`](../../crates/retail-cloud/src/topup.rs) over
[`pay_ledger::compute::purchase`](../../crates/pay-ledger/src/compute/purchase.rs)).
The wallet keeps its keys; no client stores one.

1. A principal with the spend right asks for a purchase: an ID it chooses,
   and a whole number of sats up to 1,000,000.
2. The wallet issues one exact invoice that expires in 15 minutes. Its
   description hash commits to the account, the purchase ID, and the
   amount.
3. The ledger records the purchase, and only then does the client see the
   invoice. A retry with the same purchase ID returns the recorded invoice
   and never asks the wallet again; the same ID with another amount
   conflicts.
4. The wallet's callback, or a reconciliation pass that looks each open
   purchase up, reports what happened.

| Purchase state | Meaning | Credited |
| --- | --- | --- |
| `pending` | Issued and not yet paid | No |
| `paid` | The wallet received exactly the invoice's amount | Once, from source `topup:<payment hash>` |
| `expired` | Past its expiry and never paid | No |
| `unknown` | The wallet has no record, reported a different amount, or reported success without an amount | No, until a full payment is reported |

- A duplicate callback, a concurrent observer, or a replay after a crash
  finds the credit already posted: each payment hash credits once.
- A failed lookup changes nothing. A pending invoice past its expiry
  expires; a failure report before expiry leaves it payable.
- A payment reported after the invoice expired still credits, once, because
  the money arrived.
- An invoice issued before a crash and never recorded never reaches the
  customer, so it cannot be paid.
- In v1 a purchased balance is not paid back out over Lightning.
