# exe.dev Cloud Delegation Audit for Khala Code

Date: 2026-07-06

> **SUPERSEDED (owner decision, 2026-07-06, same day):** this evaluation's
> recommendation is **not adopted**. The cloud execution substrate is
> **Agent Computers** — Firecracker microVMs on OpenAgents' own GCP
> infrastructure via the already-built `cloud/`-repo provisioner and the
> flag-gated `cloud-coding-session-routes.ts` seam — per
> `docs/khala-code/2026-07-06-agent-computers-strategy.md` (issue #8503).
> Decisive factors: the Firecracker/GCE path already exists and is ours to
> arm rather than build; per-run microVM isolation beats persistent pooled
> third-party VMs on the exact surface that matters (arbitrary user repo
> code + scoped credentials + credit charging); and exe.dev's security
> posture is too sparse for that trust boundary. This doc's
> authority-model conclusions (admission/accounting/sync stay ours) and
> its don't-boot-per-message warm-context insight carry over and are
> folded into the strategy doc. Retained as a historical evaluation only.

## Summary

exe.dev looks like a strong candidate for a simpler first version of the
Khala Code org-cloud executor pool. It does not replace our admission,
accounting, sync, or owner-scope rules, but it may replace a meaningful amount
of GCE-hosted-Pylon provisioning work for the first mobile MVP.

The practical shape would be: run a small fleet of OpenAgents-owned exe.dev
VMs, each bootstrapped as a hosted Pylon in
`OPENAGENTS_RUNTIME_EXECUTOR_MODE=org_cloud`, and let those hosted Pylons
consume `khala_runtime_control_intent.v1` from Khala Sync exactly like the
#8473 spine expects. Mobile still talks to OpenAgents; the Worker still owns
admission; Pylon still emits runtime events and exact usage receipts. exe.dev
would be the VM substrate, not the product authority.

Recommendation: run an immediate exe.dev hosted-Pylon spike before building
more bespoke GCE control-plane code. If the spike proves stable for one real
mobile thread end to end, exe.dev can be the first MVP org-cloud pool behind
#8474's admission gate. Keep the GCE/private cloud path as fallback for
stronger isolation, reserved capacity, and long-term control-plane needs.

## Source Review

Primary sources reviewed:

- https://exe.dev/llms.txt
- https://exe.dev/pricing
- https://exe.dev/docs/what-is-exe.md
- https://exe.dev/docs/api.md
- https://exe.dev/docs/https-api.md
- https://exe.dev/docs/cli-new.md
- https://exe.dev/docs/cli-rm.md
- https://exe.dev/docs/cli-stat.md
- https://exe.dev/docs/customization.md
- https://exe.dev/docs/private-image.md
- https://exe.dev/docs/integrations-github.md
- https://exe.dev/docs/integrations-attach.md
- https://exe.dev/docs/teams/vms.md
- https://exe.dev/docs/https-tokens-for-vms.md
- https://exe.dev/docs/regions.md
- https://exe.dev/docs/use-case-agent.md
- https://exe.dev/docs/faq/how-exedev-works.md
- https://exe.dev/docs/boxes.md
- https://exe.dev/security

## What exe.dev Provides

exe.dev is persistent Linux VMs with fast creation and an SSH-first control
plane. The public docs describe VM creation as container-image based with a
block device attached, usually starting quickly, and no direct public IP per
VM. HTTP/TLS is terminated by exe.dev and proxied to VM web servers; SSH routes
through `vmname.exe.xyz`.

The control plane is intentionally simple:

- `ssh exe.dev ls --json` lists VMs.
- `ssh exe.dev new --json` creates VMs.
- `ssh exe.dev rm <vm>` deletes VMs.
- `ssh exe.dev stat <vm> --json` returns CPU, memory, disk, and IO metrics.
- `POST https://exe.dev/exec` wraps the same SSH command API behind a bearer
  token.

VM creation accepts knobs we would need for a hosted-Pylon pool:

- `--name`
- `--cpu`
- `--memory`
- `--disk`
- `--image`
- `--setup-script`
- `--env`
- `--tag`
- `--integration`

The agent use-case page says `claude`, `codex`, and `pi` are preinstalled on
new VMs, and the customization docs support either setup scripts or custom
Docker images. That is a good match for a hosted-Pylon image or bootstrap
script.

Pricing as of this audit: Personal is $20/month and lists 50 VMs, 100 GB pooled
disk, and 200 GB data transfer. The same page says VMs share resources allocated
to the user pool, so the 50 VM number should be treated as a fleet-size quota,
not 50 dedicated 2-vCPU/8-GB workers. Team is $25/user/month with 50 VMs per
user and team management/SSO. Reserved Cloud Pool is the higher-scale option.

## Proposed Khala Code Architecture on exe.dev

The simplest architecture is a hosted-Pylon pool, not per-turn VM creation.

1. OpenAgents owns an exe.dev account or team.
2. An operator or pool controller creates a bounded set of VMs tagged
   `khala-org-pylon`.
3. Each VM is created from either an OpenAgents image or exeuntu plus a setup
   script.
4. Bootstrap installs or updates the repo/Pylon package, configures only
   public-safe org-cloud env, and starts a long-lived hosted Pylon supervisor.
5. The hosted Pylon runs with:
   - `OPENAGENTS_RUNTIME_EXECUTOR_MODE=org_cloud`
   - `OPENAGENTS_RUNTIME_USAGE_RECEIPTS_ENABLED=1`
   - the OpenAgents base URL
   - an org-scoped agent token or an equivalent least-privilege runner token
6. The Worker admits mobile turns through #8474 policy: valid mobile session,
   positive credit balance, per-user concurrency/rate gates, and org capacity
   availability.
7. Runtime intents land in Khala Sync.
8. exe.dev-hosted Pylons consume those intents, execute `hosted_khala`,
   `codex_app_server`, or `claude_pylon` lanes as configured, and emit the same
   sync entities the mobile app already renders.
9. Pylon posts exact usage to
   `POST /api/khala/cloud/runtime-turn-usage`.
10. #8479 charges credits from those exact receipts.

This avoids building a custom GCE leasing/provisioning service before MVP. It
also avoids booting a VM per message, which would be wasteful and would make
queue latency and setup failures part of the chat turn path.

## Could We Use the 50 Personal VMs?

Yes, for a first MVP pool or dogfood spike, with caveats.

The Personal plan's 50 VMs are useful because they let us model "one hosted
Pylon process per VM" and keep individual workspaces/processes separated at the
VM boundary. But the pricing page frames resources as pooled. That means we
should not plan on 50 simultaneous heavy coding-agent runs on the Personal
plan. A safer first operating model is:

- 1 to 3 VMs for the first proof.
- 5 to 10 warm VMs for initial mobile dogfood if the proof is stable.
- one active coding turn per VM until we have evidence that concurrent lanes on
  one VM are safe.
- `hosted_khala`/Gemini turns preferred for cheap/default execution.
- Codex/Claude org-account lanes enabled only where the VM has the intended
  account home, health checks, and cleanup.

If launch demand requires real parallel compute, graduate to Team or Reserved
Cloud Pool rather than pretending the Personal pool is 50 full machines.

## What Gets Simpler

Provisioning:

- We can create named machines with a single SSH command or HTTPS `/exec`
  call.
- Setup scripts or custom images can install the hosted Pylon once.
- Tags can identify fleet members.
- `stat` and `ls` can feed a basic pool-health reconciler.

Private repo access:

- exe.dev has a GitHub Integration that can clone private repos without putting
  raw GitHub tokens on the VM.
- Integrations can attach to specific VMs, all VMs, or tagged VMs.
- The integration can optionally act as the GitHub user for attribution in
  non-team contexts.

Agent bootstrap:

- exeuntu already includes common coding agents according to the docs.
- Custom images can pin our preferred Pylon/Codex/Claude versions.
- The "put your agent in a VM" posture fits the hosted-Pylon model.

Networking:

- We do not need public IP allocation for every executor.
- VM HTTPS tokens can expose a small private runner-health endpoint through
  exe.dev's proxy if we need pull-based health checks.

## What Does Not Get Simpler

Admission policy still belongs to OpenAgents:

- A valid mobile session must be required.
- Credit balance must be positive before org-cloud admission.
- Per-user concurrency and rate limits must live at our Worker boundary.
- The app needs typed refusals: `insufficient_credit`, `rate_limited`, and
  `org_capacity_unavailable`.

Accounting still belongs to OpenAgents:

- Usage must come from exact receipts.
- The client must not supply billable amounts.
- #8479 still needs to charge from `token_usage_events`, not from exe.dev
  metrics or wall-clock estimates.

Authority boundaries still belong to OpenAgents:

- exe.dev-hosted Pylons are org capacity only.
- This must not widen access to any user's own Pylon or another user's
  capacity.
- The old owner-self Pylon invariant remains true for user-owned Pylons.

Isolation is not "solved":

- exe.dev gives VM boundaries, but the docs do not make our per-run cleanup,
  secret lifecycle, repo-token scoping, raw-event retention, or malicious
  checkout policy decisions for us.
- Long-lived hosted VMs need workspace cleanup, credential scanning, and
  process supervision just like GCE hosted Pylons.

## Security and Trust Notes

exe.dev's public security page is sparse. The architecture FAQ says VMs run on
bare-metal hosts using Cloud Hypervisor, with container-image boot and attached
block devices. That is a credible lightweight VM substrate, but it is not the
same as us owning a dedicated Firecracker control plane with policy-specific
attestation and lifecycle hooks.

For MVP, that is acceptable only if we state the trust model honestly:

- exe.dev is third-party org-cloud infrastructure.
- The execution lane is OpenAgents-owned capacity, not user-owned local
  capacity.
- Do not place raw user provider keys, wallet material, or broad GitHub tokens
  on these VMs.
- Prefer short-lived runner credentials.
- Keep private repo access brokered and scoped.
- Assume VM compromise can expose that VM's workspace and local process env;
  design the runner token accordingly.

## Interaction With #8474-#8479

#8474 becomes easier, not unnecessary. Its `org_capacity_unavailable` check can
initially read a small exe.dev pool ledger:

- expected VM count by tag
- last heartbeat from each hosted Pylon
- recent `stat` / runner-health status
- current active turn count
- breaker state for failed VMs

#8475 may become easier for clone operations if we use exe.dev GitHub
Integrations, but we must not silently replace our user GitHub OAuth contract.
For MVP we can choose one of two paths:

- OpenAgents-brokered SCM auth remains canonical; exe.dev only supplies the VM.
- exe.dev GitHub Integration is used as an interim clone broker and documented
  as a launch trust decision.

#8476 must explicitly cover exe.dev's isolation posture:

- persistent VM per hosted Pylon
- one active run per VM initially
- workspace cleanup between turns
- credential scanner before closeout/writeback
- no pooled third-party user capacity
- no raw prompts/tokens in public traces

#8477 can potentially use exe.dev's GitHub Integration for push/PR attribution,
especially `--act-as-user`, but the docs say user attribution is not available
on team integrations. That is a real constraint if the production pool needs
team-owned administration.

#8479 is unchanged except that exe.dev `stat` data may be useful operational
telemetry. It must not become billing truth. Credits charge from our exact
runtime usage receipts.

## Minimal Spike Plan

1. Owner creates or confirms an exe.dev account/team and produces a short-lived
   API token with only the needed commands.
2. Create one VM:

   ```sh
   ssh exe.dev new --name khala-org-pylon-001 --tag khala-org-pylon --json
   ```

3. Bootstrap with a setup script or custom image that installs the hosted Pylon
   supervisor.
4. Start Pylon in org-cloud mode with one active run slot.
5. Confirm the hosted Pylon can consume a synthetic runtime intent and emit
   sync events.
6. Run one real mobile thread turn through `hosted_khala`.
7. Verify exact usage lands in `token_usage_events` through
   `POST /api/khala/cloud/runtime-turn-usage`.
8. Kill/restart the VM and verify the pool surfaces
   `org_capacity_unavailable` while it is down and recovers after restart.
9. Repeat with a private-repo fixture only after #8475's SCM decision is made.

## Open Questions

- Can OpenAgents get an exe.dev Team account quickly enough, or is the first
  proof on a Personal account?
- Are the Personal pool's shared 2 vCPU / 8 GB resources enough for multiple
  Codex/Claude runs, or only for a few `hosted_khala`/Gemini turns?
- What is the supported way to run a persistent background process on exeuntu:
  systemd, supervisord, shell loop, or Shelley-managed command?
- Can we create least-privilege exe.dev API tokens that allow `ls`, `new`,
  `rm`, and `stat` without broader account operations?
- Can GitHub Integration support our required user attribution path in the
  account/team configuration we would actually use?
- What is exe.dev's retention/backups story for persistent disks, and how fast
  can we guarantee workspace cleanup or VM replacement after a failed run?
- What happens under prolonged CPU/RAM pressure in the pooled Personal plan,
  and how visible is throttling to `stat` or health checks?

## Recommendation

Use exe.dev as the first hosted-Pylon substrate if the spike passes. It is a
better MVP fit than building a bespoke per-turn cloud computer system right now:
fewer moving parts, persistent VMs, simple SSH/HTTPS control APIs, custom
images/setup scripts, and GitHub integration hooks.

Do not describe it as "50 cloud computers for $20/month" in product or
planning copy. Describe it as "up to 50 persistent VM slots over a shared
Personal resource pool." For serious launch concurrency, plan either Team or
Reserved Cloud Pool capacity, or keep the GCE/private control-plane path ready.

Most importantly: exe.dev can simplify provisioning, but it does not change
the authority model. OpenAgents still owns admission, exact accounting,
credit charging, sync projection, user-visible refusals, and the invariant
that org-cloud execution never routes through another user's Pylon.
