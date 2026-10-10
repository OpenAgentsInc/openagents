# Cloud

Running Coder and agent work on Google Cloud machines (project
`openagentsgemini`) in parallel with, and in place of, the owner's own
computers.

[Developing OpenAgents on our own production environment](dogfood-dev-on-prod.md)
is the gap list for moving agent work off the owner's Mac, with an issue taken
end to end on a cloud environment.

The [Coder Cloud and openagents.com specification](coder-cloud.md) defines the
proposed Rust web workspace, connected Verse and delegation views, and customer,
team, billing, and sales interfaces over the existing domain owners.

The proposed [repository environment onboarding process](example-cursor-cloud-agent-onboarding/environment-onboarding.md)
ports the observed Cursor setup into a native setup, build, fresh verification,
and Save lifecycle. Start with the [Cursor workflow analysis](example-cursor-cloud-agent-onboarding/cursor-environment-onboarding-analysis.md)
and [ordered tool ledger](example-cursor-cloud-agent-onboarding/cursor-environment-onboarding-tool-sequence.md) for the
evidence behind the design.

The [managed computer plan](managed-computers.md) joins that onboarding contract
with the [Orbs research](../research/orbs.md), current native Cloud jobs, and the
web composer. Its terms and implementation boundaries match the
[glossary](../glossary.md#cloud-computers-and-repository-environments).

The [OpenInspect source study](../research/openinspect.md) compares durable
session ownership, provider checkpoints, tool capture, and collaboration with
these existing Rust boundaries and the remaining environment work.

## Resident operator bridge

The following command loads a separately admitted operator policy:

```sh
openagents host serve --state /absolute/private/access --root /absolute/private/host \
  --cloud-operator /absolute/private/operator.json
```

The file has mode `0600`, its
containing directory has mode `0700`, and neither path contains symlinks.
The policy maps a native device to workspace, project, and executor profile
aliases. Host `observe` and `operate` grants still apply to each request.

The policy uses this schema; paths name explicitly selected native files:

```json
{
  "schema": "openagents.coder.cloud-operator.v1",
  "operators": [{"device": "<64 hex public key>", "workspace": "checkout", "project": "engineering", "profiles": ["review"]}],
  "profiles": {
    "review": {
      "workspace": "checkout", "project": "engineering", "cwd": "/absolute/repository",
      "source_revision": "<40 hex Git commit>", "source_digest": "sha256:<64 hex>",
      "paths": [], "include": [], "pool": "review-pool", "placement": "boat", "mode": "coder",
      "executor": "codex", "model": null, "reasoning": null, "max_timeout_seconds": 600,
      "size": "small", "template": null, "credentials": {}, "adapter": {"kind": "unavailable"}
    }
  }
}
```

Compute `source_digest` with `coder_cloud::workspace::source_identity`. A Boat
adapter requires `kind: "boat"`, an explicit `origin`, and a private `token_file`.
A GCE adapter requires `kind: "gce"`, `pool_file`, `gcloud_binary`,
`config_directory`, `credential_file`, and the admitted `hosts`. It refuses
integrated mode. Credential names map to private files in `credentials`.
Real adapters currently qualify only executor `codex` in mode `coder`, with a
nonempty, explicitly selected `OPENAI_API_KEY` or `OA_CODEX_AUTH` file. The latter
must contain a JSON object; configuring it does not verify a provider login.
`GH_TOKEN` and `GITHUB_TOKEN` are the only additional admitted credential
names. GCE commands start with a clean remote environment, and the Coder
runtime uses a separate `CODEX_HOME` for each job. Other real executor modes
remain unavailable until their remote login isolation is qualified. An injected
synthetic backend has separate admission and never contacts a provider.
Changed source, policy, pool, or credential pins refuse stale work; changed
adapter profiles require a host reload. This bridge never selects ambient
provider logins or establishes retail spending rights.

Reads preserve canonical jobs and original byte chunks. Submission, continuation,
cancellation, and reconciliation require a reviewed native request. Repeating
that request returns its retained result, and reconciliation follows the
original provider task without submitting a replacement. Provider and cleanup
uncertainty remain visible until the native owner records a result.

### Environment owners

`--environment-owners /absolute/private/environment.json` (only beside
`--cloud-operator`) runs the repository environment setup, build, and verify
owners next to the operator as one packaged service
([`coder-environment-operator`](../../crates/coder-environment-operator/src/lib.rs),
ENV-08). They keep their state under `<root>/cloud-operator` with the
environment records the operator reads, and their loop starts and stops with
the operator. The configuration file holds no secret:

```json
{
  "schema": "openagents.environment.owners.v1",
  "provider": "boat",
  "workdir": "/workspace/repo",
  "template": null,
  "credential_names": ["GH_TOKEN"],
  "tick_seconds": 15
}
```

The Boat client reads `BOAT_API_KEY` (or Secret Manager `boat-api-key`) and
`BOAT_API_BASE`. Credential values are read by name from the host process
environment at start and are also the redaction set. Setup and builder
machines may receive them; verifier machines never do. The
[environment onboarding contract](example-cursor-cloud-agent-onboarding/environment-onboarding.md)
lists the state layout and restart recovery. A configured package is not
deployed-origin availability. That requires the owner-run Boat qualification
recorded in `NEEDS_OWNER.md`.

The optional dedicated GCE adapter (ENV-09,
[`coder_working_computer::gce`](../../crates/coder-working-computer/src/gce.rs))
replaces Boat with one isolated GCE instance per setup, builder, and verifier
machine. It never uses the shared Coder pool:

```json
{
  "schema": "openagents.environment.owners.v1",
  "provider": "gce",
  "workdir": "/home/coder/repo",
  "gce": {
    "project": "openagentsgemini",
    "zone": "us-central1-a",
    "machine": "c3-standard-8",
    "disk_gb": 200,
    "base": {"project": "openagentsgemini", "name": "oa-coder-host-20261001", "id": "1234567890123456789"}
  },
  "credential_names": ["GH_TOKEN"],
  "tick_seconds": 15
}
```

`base` is one exact image name and its numeric GCE ID (`gcloud compute images
describe NAME --format='value(id)'`), never a family. A recipe for this
adapter pins the same base (`GceImage::pin`). The host runs `gcloud` with its
current account and reaches instances over the pool's SSH key through IAP.
Its account needs instance, disk, and image create/delete rights in the
project. Instances get no service account and no API scopes. The janitor
writes `<root>/cloud-operator/environment-gce/reconciliation.json` with every
owned instance and image, their sizes and run times, and the orphan instances
it deleted. It never deletes an image.

## Existing cloud lanes

The [terminal workbench roadmap](../terminal/workbench-roadmap.md#paid-openagents-cloud-computers-and-credits)
adds a proposed paid OpenAgents cloud-computer option inside Verse and the
other clients. Purchased credits require shared funding, quotes,
reservations, metering, and recovery through the agentic execution router.
Boat/GCE operator placement and hosted model fallback already exist; they
do not establish a customer credit balance or a paid remote-computer product.
The [paid-cloud issue directory](../terminal/issue-roadmap.md#later-paid-cloud)
tracks the retail contract, account, funding, execution, recovery, and launch
slices on the [Terminal and Workbench project](https://github.com/orgs/OpenAgentsInc/projects/20).
Later payment and market profiles have separate conditional issues.

On `main` today: the Boat SDK (`crates/boat`), the daily Boat template
(`crates/boat-template`, `scripts/cloud/coder-host-setup.sh`), builds on Boat
(`openagents boat run`, `scripts/boat-run.sh`), Coder issue runs on Boat
(`openagents chat work --on boat`), the daily `oa-coder-host` GCE image, and
the GCE spot pool granted as one computer (`openagents cloud up/down/status`,
`openagents chat work --on gce`). The earlier Cloud crates and documents (`oa-codex-control`, `oa-node`, `oa-workroomd`, the old
`docs/cloud/`) were removed in commit `dabc08102f` on 2026-09-18; read them at
[`8f84d05896`](https://github.com/OpenAgentsInc/openagents/tree/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/cloud).

| Document | What it covers |
| --- | --- |
| [Cloud computers and repository environments](managed-computers.md) | Working web selections over admitted native jobs, and the remaining prepared environment, machine, service, preview, and retail owners |
| [Repository environment onboarding](example-cursor-cloud-agent-onboarding/environment-onboarding.md) | Proposed native environment owner, existing infrastructure and issue dependencies, recipe/build/verifier/Save contracts, complete evidence, and implementation slices |
| [Cursor onboarding analysis](example-cursor-cloud-agent-onboarding/cursor-environment-onboarding-analysis.md) | Exact observed setup stages, changes, backend build order, fresh verification, limitations, and decisions for the OpenAgents port |
| [Cursor onboarding tool sequence](example-cursor-cloud-agent-onboarding/cursor-environment-onboarding-tool-sequence.md) | All 121 parent call IDs in submission order, exact shell commands, seven child calls, and links to original responses |
| [Cloud parallel execution audit, 2026-10-02](2026-10-02-cloud-parallel-execution-audit.md) | What existed before the reset (GCE and Firecracker lanes, the Coder run pool, Factory Droid Computers and Amp orbs), what runs in Google Cloud today, the gaps, a recommended design, and the issues to open |
| [Boat SDK plan, 2026-10-02](2026-10-02-boat-sdk-plan.md) | Boat (formerly Ascii Box): what we built against it, its current API and prices, how it compares with the GCE pool, and a plan for the Rust SDK `crates/boat` as a second placement backend |
| [`chat work --on boat`](boat-chat-work.md) | Coder issue runs on Boat sandboxes: one sandbox per issue from the daily template, the start-limit dispatcher, per-run credentials, engine logins, streaming, cost in the issue comment and the route record, teardown |
| [Saved environments contract v1](retail-environment-contract.md) | Proposed, closed until owner review and funded qualification: a customer buys setup, build, check, and a saved image kept for prepaid days, with its price book, holds, renewal, lapse, and recovery (ENV-10) |
| [Retail contract v1](retail-contract.md) | The first paid cloud computer and task class, frozen for implementation: Boat per task, a repository change with declared checks, the customer's own model key, credentials, grants, cost identities, and provider-loss outcomes |
| [Bring your own Claude](claude-code-byo.md) | Running unmodified Claude Code in Cloud computers on the user's own plan or key: sign-in inside the computer, no credential collection, plan concurrency, and API keys for fleets |
| [Retail prices](retail-prices.md) | Price book `retail-2026-10-06.1`: one credit per sat, compute and coordination rates, quotes, what a failed or cancelled task is charged, and holds versus refunds |
| [Purchased compute balance](compute-balance.md) | One customer account in the central ledger that every client resolves to: principals, credential digests, read and spend rights, top-ups, holds, and settlement |
| [Retail service](retail-service.md) | The retail flow behind the paid-availability gate: authorities, offers, holds, provisioning, credentials, dispatch, metering, teardown, cancellation, recovery, settlement, and the launch gate |
| [Metered Lightning sessions](mpp-sessions.md) | The `openagents.mpp.lightning-session.v1` profile: deposit, frozen rate, ceiling, admitted debits, closure, and remainder refunds as liabilities |
| [Retail qualification](retail-qualification.md) | The fake-payment acceptance run and its retained receipt, the funded-qualification runner and owner runbook, and the fail-closed launch gate |
| [Retail operations](retail-operations.md) | The fail-closed launch gate, deployment and state compatibility, monitoring alerts, reconciling stuck funded requests, and incidents |
| [The GCE pool](gce-pool.md) | `openagents cloud up/down/status` and `chat work --on gce`: spot hosts from the daily image granted as one computer `gce`, two runs per host, self-delete after 10 idle minutes, credentials, measurements and cost |
