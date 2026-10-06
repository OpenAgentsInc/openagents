# The first retail cloud computer and task: contract v1

Status: frozen 2026-10-05 for implementation
([#10704](https://github.com/OpenAgentsInc/openagents/issues/10704)). The
owner's review of the product choices below is recorded in
`NEEDS_OWNER.md`; a change after that review is a new contract version, not
an edit to this one. Nothing here is purchasable yet: paid availability
stays off until the funded qualification in the
[paid-cloud issues](../terminal/issue-roadmap.md#later-paid-cloud) passes.

This page names exactly one computer class and one task class that a
customer can buy with prepaid OpenAgents compute credits, and what
completes and checks that task. It builds on the operator placements that
exist today ([Boat](boat-chat-work.md) and the [GCE pool](gce-pool.md)) and
on the [agentic execution router](../api/2026-10-02-agentic-execution-router.md).
It is separate from [hosted inference fallback](../coder/runtime/cloud-fallback.md).

## The classes

| Identity | Value |
| --- | --- |
| Contract | `openagents.cloud.retail.v1` |
| Computer class | `retail-boat-large-v1` |
| Task class | `retail-repo-change-v1` |

### Computer class `retail-boat-large-v1`

- **Provider: Boat.** One Boat `large` sandbox (8 vCPU, 16 GB) per task,
  started from the newest ready daily template `oa-coder-main-<date>`, on
  OpenAgents' operator Boat account.
- **Why Boat first, not GCE.** A Boat sandbox is one task's machine: it is
  created per task and deleted after it, it starts with `noEnv: true` (no
  account credential from Boat), and Boat reports its billed seconds per
  sandbox (`GET /sandboxes/{id}/usage`), which is the exact metering scope
  this class needs. The GCE pool runs two runs per host under one operator
  grant, so two customers would share a machine and a bill; it stays an
  operator placement. A GCE class is a later contract version with one host
  per task.
- **No customer access to the machine.** The customer gets no shell, SSH,
  or terminal on the sandbox in v1. The workbench shows the task's retained
  run view, not a PTY.
- **No persistence.** No sandbox is saved as a snapshot or reused for
  another task. `/tmp` holds the run's credentials and is never captured.
- **Bounds.** At most 60 minutes of wall time per task (the caller may set
  less), one sandbox per task (plus at most one replacement before the run
  starts; see below), and at most 4 retail sandboxes at once across all
  customers, under the operator plan's start limits.

### Task class `retail-repo-change-v1`

A Coder change against a repository snapshot, returned as a patch with
check results. It never publishes.

| Part | Contract |
| --- | --- |
| Source | A public HTTPS Git repository on `github.com` and an exact 40-character commit. Materialization clones it into the sandbox and refuses unless `HEAD` is that commit and the work tree is clean. No private repositories, uploads, or submodules in v1. |
| Request | The task text, at most 16 KiB, bound by digest in the admission snapshot. |
| Executor | Coder with one engine: Codex CLI as one lean `codex exec` session on the customer's own OpenAI API key, the path Coder takes under an API-key login ([#10275](https://github.com/OpenAgentsInc/openagents/issues/10275)). No other engine, no fan-out, no delegation. |
| Model payer | The customer's own key (`Payer::CallerKey { provider: "openai" }`). OpenAgents-hosted inference is not part of this class: Coder's cloud fallback (Vertex through the OpenAgents cloud) is off in a retail run, and a run that hits its provider's limit ends `limited` instead of falling over. |
| Checks | 1 to 8 check commands the customer declares with the request (for example `cargo test -p name`), each at most 1,024 bytes and 15 minutes, frozen by digest before the candidate exists. The task owner's independent check runs them read-only on the exact candidate, after the executor ends. |
| Effects | Writes inside the clone only, network for the engine's provider and package registries, no publication (no push, no pull request, no issue comment), and the Linux `bubblewrap` boundary Coder's run uses. |
| Retained artifacts | The patch against the pinned commit, each check's exit status and last 64 KiB of output, the run summary, and the scrubbed ATIF trace, kept 30 days. Nothing else leaves the sandbox. |

### Completion and checks

A task's outcome is the route-contract lifecycle projected from the task
owner, as for local runs:

| Outcome | Meaning |
| --- | --- |
| `completed` with check `verified` | The executor ended, the patch is retained, and every declared check passed on it. |
| `completed` with check `check_failed` | The patch is retained and at least one declared check failed. |
| `completed` with check `unchecked` | The executor ended with no change to check. |
| `failed` | The executor failed, hit the wall-time ceiling, or was `limited` by the customer's provider. |
| `cancelled` | The customer cancelled, or a right was revoked; the acknowledgment and final charge state are kept. |
| `needs_reconciliation` | A crash or provider loss left the outcome uncertain. The reservation stays held until reconciliation. |

"Done" means `completed`; "checked" means `verified`. The contract promises
nothing about the patch's quality beyond the declared checks.

## Lifecycle and capacity

1. **Offer.** The router quotes the class, the task, the source, the
   disclosures (the repository to the sandbox, the task text and source to
   OpenAI under the customer's key), the payer per resource, and a maximum
   charge in sats from a versioned price book (#10706). No capacity, no
   offer: when the operator plan has no Boat starts left, or 4 retail
   sandboxes run, the router refuses before any reservation.
2. **Reserve.** Confirming the unchanged offer reserves the maximum charge
   from the customer's purchased balance before anything starts (#10710).
3. **Provision.** One sandbox starts. If it is not reachable in 10 minutes
   it is deleted and replaced once; the replacement's cost is the
   operator's.
4. **Materialize and run.** Credentials are delivered (below), the source is
   cloned and verified, and the task runs.
5. **Check and retain.** The declared checks run; the retained artifacts
   upload; the sandbox is deleted.
6. **Settle.** The measured charge settles from the reservation and the rest
   is released (#10718).

Provider readiness and loss map to retail outcomes:

| Provider state | Retail outcome | Compute charge |
| --- | --- | --- |
| Start refused for plan limits | No offer, or the offer refuses at confirmation | None; nothing reserved |
| Not reachable after the replacement | `failed` (`provider_unavailable`) | None; the hold is released |
| Lost before the executor started | `failed` (`provider_lost`) | None |
| Lost after the executor started (stopped, deleted, or past its lifetime) | `needs_reconciliation`, then `failed` (`provider_lost`) once the operator confirms the sandbox is gone | Measured seconds up to the loss, capped by the quote; no new task starts without a new offer |
| Usage unreadable after deletion | `needs_reconciliation` | Held until the usage reads or the operator settles it by hand |

A reconnecting client follows the funded task; it never starts or charges a
second one.

## Credentials and grants

- **No owner or operator credential reaches a retail sandbox.** The owner's
  GitHub token, Codex ChatGPT login, xAI key, and the operator's Secret
  Manager entries that Boat and GCE operator runs read
  (`chat_boat::credentials`) are never read on a retail path. The sandbox
  receives only the customer's OpenAI API key, through the same
  `/tmp`-and-delete delivery as operator runs (#10712), and public source
  needs no token.
- **No operator grant admits a retail run.** Operator placements carry
  grants with source `operator` (`boat:<sandbox>`, `gce:<pool>`). A retail
  dispatch requires a retail grant minted for one funded execution
  identity and one sandbox; an operator grant, a pool grant, autostart, or
  pairing refuses (#10708).
- **Four authorities stay separate.** Observing the task, executing it on
  the rented computer, disclosing the source and task to OpenAI, and
  spending the customer's balance are admitted separately. A balance does
  not grant execution, and pairing does not grant spending.

## Cost identities

Each is its own line; none stands in for another.

| Identity | Who pays | In a retail task |
| --- | --- | --- |
| Purchased balance | The customer, in sats shown as credits | The rented computer's measured seconds and the OpenAgents routing charge, under the quote |
| Customer's provider key | The customer, directly to OpenAI | All model usage; OpenAgents records it as `caller_key` and never charges for it |
| Infrastructure bill | OpenAgents, to Boat | The sandboxes, including replacements, failed starts, and reconciliation |
| Operator allowance | OpenAgents | Operator runs on Boat and GCE (`chat work --on boat/gce`); never a retail task |
| Sponsored inference | OpenAgents | Hosted Vertex fallback for local Coder turns; never a retail task |

## Compatibility with the route contract and payments

- **Admission.** A retail task is one admission snapshot
  (`openagents.route.admission-snapshot.v1`) with `placement.computer` the
  retail class's sandbox, `money.funding: balance`, and a quote with basis
  `reservation`, plus one [workbench binding](../../crates/route-contract/src/binding.rs)
  naming the sandbox's generation and the funded run. A continuation of a
  retail task is a new offer: v1 has no continuation on the same sandbox.
- **New vocabulary arrives versioned.** The frozen snapshot has no
  `compute` resource, `retail` grant source, or provider-loss reason. They
  are added by #10706, #10708, and #10709 in new or additive documents
  bound by digest, never by editing a v1 schema in place.
- **Payments.** Prices stay in sats and Lightning, per the router plan's
  section 8: prepaid balance, durable reservation before provisioning,
  release of unused holds without calling it a refund, and unknown costs
  held for reconciliation. Credits are a display of that balance, not XP,
  game gold, a wallet balance, provider credits, Boat's allowance, or the
  GCE bill.
- **Out of scope for v1.** Private repositories, publication, customer
  shells, GCE placement, hosted inference, continuation on a kept sandbox,
  several engines, and per-call x402 or MPP funding.
