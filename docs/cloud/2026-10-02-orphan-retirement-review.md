# Orphan retirement review

Issue: [#10223](https://github.com/OpenAgentsInc/openagents/issues/10223).

The [fresh inventory](evidence/2026-10-02-orphan-inventory.json) confirms that
the following candidates are still running. Labels identify components, not
current human owners or activity. No instance was stopped or deleted, and no
monthly saving has been verified.

| Instance | Zone | Machine type | Ownership evidence |
| --- | --- | --- | --- |
| `coder-pool-w2v4` | `us-central1-b` | `c3-standard-8` | No labels |
| `coder-box-pool-8b1t` | `us-central1-b` | `c3-standard-8` | No labels |
| `agent-computer-gce-1` | `us-central1-a` | `n2-standard-4` | No labels |
| `oa-codex-control-1` | `us-central1-a` | `e2-small` | Codex control |
| `oa-managed-sandbox-control-1` | `us-central1-a` | `e2-small` | Codex control |
| `oa-managed-sandbox-control-sbx09-canary-1` | `us-central1-a` | `e2-small` | Codex control |
| `oa-managed-sandbox-control-staging-1` | `us-central1-a` | `e2-small` | Codex control |
| `oa-issue7-final-p-a` | `us-central1-a` | `n2-standard-16` | No labels |
| `one-eval-cli-builder` | `us-central1-a` | `n2-standard-32` | No labels |

The issue requires an owner decision before retirement. Confirm the current
owner and workload of each candidate, then choose retain or stop. Prefer a
reversible stop before deletion. Inspect managed instance groups before stopping
pool members: a manager can recreate them. Preserve disks and snapshots until
the data-retention decision is made. Stopped instances can still incur storage
and reserved-address charges.

Production-labeled GKE nodes, payment infrastructure, relays, and the newly
created host-measurement instance are not retirement candidates in this review.
The absence of labels is not proof that a machine is idle.
