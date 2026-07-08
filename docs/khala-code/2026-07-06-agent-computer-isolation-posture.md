# Agent Computer Isolation Posture

Date: 2026-07-06
Status: shipped public-repo enforcement for #8476. The live Firecracker proof
remains owner-gated under #8503.

This document records the enforceable isolation contract for Khala Code mobile
Agent Computers. It implements §4 of
`docs/khala-code/2026-07-06-agent-computers-strategy.md` and replaces every
prior "hosted Pylon" or exe.dev persistent-VM posture. The Pylon runtime is
software inside the image; the provisioned, metered, and isolated unit is the
Agent Computer.

## Contract

- **Unit of isolation:** one Firecracker microVM per admitted work context
  (`user + thread + repo binding`). The Worker sends `work_context_ref` plus
  optional `thread_ref` and `repo_binding_ref` in the placement request. The
  control plane must echo the same work-context ref in its placement binding;
  a missing or mismatched binding fails closed before the Worker projects an
  Agent Computer.
- **Lifecycle:** the control plane owns requested → provisioning → ready →
  active → idle → reclaiming → reclaimed, plus failed/quarantined branches.
  Public projections may expose only refs for lifecycle and resource receipts.
- **Reclaim:** a `cloud.gce.cleanup` event is not enough by itself. To project
  `reclaimed`, the event must carry both `scratchWipeReceiptRef` and
  `microvmDestroyReceiptRef` (snake-case equivalents are also accepted). Without
  those refs, the Worker rejects the placement as
  `agent_computer_reclaim_evidence_missing`.
- **Credentials:** repo access enters the microVM only through the SCM broker.
  Provider account access enters the microVM only through the provider-account
  broker with `provider_credential_policy: broker_only`. Assignment payloads,
  placement requests, public projections, issue comments, and traces carry refs
  only. A custodied ChatGPT/Codex subscription credential is keyed to one owner
  and may be injected only into that owner's own admitted work context. It is
  never pooled, never routed to another user's turn, and never used to serve
  OpenAgents org demand or `subscription_capacity_resale`. Pylon's
  `scanLongLivedScmCredentials` remains the closeout/writeback gate for
  workspaces and isolated account homes, and now rejects provider auth files as
  well as SCM credentials.
- **Writeback:** branch/PR publication uses the brokered user GitHub
  authorization and may push only scoped task branches. Force-push refspecs are
  disallowed; permission failures surface as typed refs rather than leaking Git
  output or falling back to ambient credentials.
- **Network/projection:** Agent Computers serve no inbound traffic. Progress
  flows out over authenticated OpenAgents connections. Public data may include
  placement refs, work-context refs, Agent Computer refs, lifecycle receipt refs,
  resource-usage receipt refs, and content-addressed artifact refs, never raw
  GCE topology, guest IPs, SSH material, prompts, repo content, provider master
  keys, raw OAuth tokens, wallet material, or private traces.

## Public Enforcement

- `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`
  builds an `openagents.agent_computer_isolation_policy.v1` payload for every
  live placement request. It includes the hard timeout, idle reclaim default,
  scratch-wipe and microVM-destroy requirements, SCM-broker-only credentials,
  provider-credential-broker-only credentials, owner-scoped provider grants,
  `subscription_capacity_resale: false`, credential-scanner requirement,
  no-wallet/no-master-key/no-raw-OAuth claims, no-inbound networking, and
  public-refs-only projection.
- The same route validates the placement response before projecting it:
  `agent_computer_isolation_policy_echo_missing`,
  `agent_computer_isolation_policy_echo_mismatch`,
  `agent_computer_work_context_binding_missing`,
  `agent_computer_work_context_binding_mismatch`, and
  `agent_computer_reclaim_evidence_missing` are typed fail-closed adapter
  reasons.
- `apps/pylon/src/workspace-materializer.ts` rejects unbrokered private GitHub
  repos, wrong broker scopes, credentialed URLs, embedded tokens, and unsafe
  fallback modes. Its credential scanner rejects long-lived GitHub/Forged Git
  tokens, Git authorization extraheaders, Codex auth JSON, and OpenAI API keys
  before verification/writeback.
- `apps/pylon/deploy/agent-computer/` documents the public, non-secret GCE host
  and image contract: nested virtualization, `/dev/kvm`, no external IP by
  default, no control tokens in scripts, disposable scratch, and owner-gated
  signed image receipts.

## What Is Still Owner-Gated

The public repo can enforce the seam, not prove the private host. #8503 remains
open until the owner records public-safe receipts for the nested-virt GCE host,
the signed/digest-pinned image, the armed real control plane, a real
mobile-dispatched turn inside Firecracker, lifecycle receipts, compute receipts,
and exact token receipts. The compute billing rate remains #8479 / NEEDS_OWNER.

Verification for this change:

```sh
bun run test -- src/cloud/cloud-coding-session-routes.test.ts
bun test apps/pylon/tests/gcloud-setup-script.test.ts
bun test apps/pylon/tests/workspace-materializer.test.ts
```

## Provider-ToS Position

The CX-1 amendment treats connected ChatGPT/Codex credentials as
owner-directed agentic work on a user-controlled machine: the owner explicitly
connects their own account and asks an Agent Computer to run that owner's own
repo/thread task. It does not sell, rent, pool, lend, or route the subscription
to other users or to OpenAgents org demand. The broker path vends only
short-lived material for the owner-matched work context and reclaim destroys
the scratch home; any future provider-specific ToS change must update this
document and `apps/openagents.com/INVARIANTS.md` before widening behavior.
