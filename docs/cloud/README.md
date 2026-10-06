# Cloud

Running Coder and agent work on Google Cloud machines (project
`openagentsgemini`) in parallel with, and in place of, the owner's own
computers.

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
| [Cloud parallel execution audit, 2026-10-02](2026-10-02-cloud-parallel-execution-audit.md) | What existed before the reset (GCE and Firecracker lanes, the Coder run pool, Factory Droid Computers and Amp orbs), what runs in Google Cloud today, the gaps, a recommended design, and the issues to open |
| [Boat SDK plan, 2026-10-02](2026-10-02-boat-sdk-plan.md) | Boat (formerly Ascii Box): what we built against it, its current API and prices, how it compares with the GCE pool, and a plan for the Rust SDK `crates/boat` as a second placement backend |
| [`chat work --on boat`](boat-chat-work.md) | Coder issue runs on Boat sandboxes: one sandbox per issue from the daily template, the start-limit dispatcher, per-run credentials, engine logins, streaming, cost in the issue comment and the route record, teardown |
| [Retail contract v1](retail-contract.md) | The first paid cloud computer and task class, frozen for implementation: Boat per task, a repository change with declared checks, the customer's own model key, credentials, grants, cost identities, and provider-loss outcomes |
| [Retail prices](retail-prices.md) | Price book `retail-2026-10-06.1`: one credit per sat, compute and coordination rates, quotes, what a failed or cancelled task is charged, and holds versus refunds |
| [Purchased compute balance](compute-balance.md) | One customer account in the central ledger that every client resolves to: principals, credential digests, read and spend rights, top-ups, holds, and settlement |
| [Retail service](retail-service.md) | The retail flow behind the paid-availability gate: authorities, offers, holds, provisioning, credentials, dispatch, metering, teardown, cancellation, recovery, settlement, and the launch gate |
| [The GCE pool](gce-pool.md) | `openagents cloud up/down/status` and `chat work --on gce`: spot hosts from the daily image granted as one computer `gce`, two runs per host, self-delete after 10 idle minutes, credentials, measurements and cost |
