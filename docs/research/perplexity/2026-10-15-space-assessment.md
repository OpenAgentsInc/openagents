# SPACE assessment: Perplexity's open-source sandbox platform

2026-10-15. Source: <https://github.com/perplexityai/space>. Reviewed at
commit [`de87b43`](https://github.com/perplexityai/space/commit/de87b436c35dac056564bdd5b2a113c9046e189b)
(workspace version `0.8.0`). We read a shallow clone (the last 200
commits) in a scratch directory and deleted it afterwards. We built and
ran nothing: SPACE needs a Linux x86_64 host with KVM, and this review
ran on a Mac.

## Plain summary

- **What it is.** SPACE (Sandboxed Platform for Agentic Code Execution)
  is the sandbox platform under Perplexity Computer. An HTTP API creates
  Firecracker microVMs from templates, runs commands in them, streams the
  output, and pauses, suspends (memory snapshot), wakes, forks and
  deletes them.
- **What is new.** It is not the idea; it is the production detail.
  Memory snapshots that survive a host move, warm pools, egress policy
  with credentials injected at the network gateway (secrets never enter
  the guest), content-addressed execution environments, and a long list
  of recovery and fencing rules, all open source under Apache-2.0.
- **How it relates to us.** It sits exactly where our
  [computers layer](../../cloud/computers.md) has a hole: a fast,
  memory-preserving driver. Our `oa-boat` gives each computer a whole GCE
  VM (77 s to a first turn, 52 s to resume, disk only). Our
  [Factory plan](../factory/README.md) already lists "Firecracker driver
  (L)" as item 11. SPACE could be that driver instead of us writing one.
- **What we recommend.** Spike SPACE on one nested-virtualization GCE
  host behind a `computer-driver` implementation and measure it against
  our conformance suite. Adopt its credential-injection and
  execution-identity ideas regardless of the spike. Do not take its
  Kubernetes/AWS reference stack, its E2B wrapper or its console.

## The announcement

Aravind Srinivas, Perplexity's CEO, [posted on
2026-10-15](https://x.com/AravSrinivas/status/2110785848958759165) (X
blocks fetching with HTTP 402; we read the text through X's public
syndication endpoint):

> While sandboxes are a commodity, serving millions of them under
> production-grade traffic requires a lot of detail to scale agents for
> businesses and consumers. We're open-sourcing the sandbox platform
> SPACE that powers all of Perplexity Computer to support the ecosystem.

SPACE itself was announced in July 2026 as a closed platform. Coverage
from then:

- [Techstrong.ai](https://techstrong.ai/articles/perplexity-unveils-space-a-secure-sandbox-platform-for-ai-agents/):
  every Computer task runs in a Firecracker microVM; sessions are
  separate from sandboxes and can pause, resume or branch; secrets never
  enter the sandbox; node services inject scoped tokens on outbound
  calls.
- [The New Stack](https://thenewstack.io/perplexity-space-agent-sandboxes/),
  [TestingCatalog](https://www.testingcatalog.com/perplexity-launches-space-runtime-for-ai-agent-tasks/),
  [AI Business](https://aibusiness.com/agentic-ai/perplexity-ai-introduces-sandbox-for-agents),
  [Karo Zieminski](https://karozieminski.substack.com/p/perplexity-space-runtime-computer-agents):
  100% of Computer traffic on SPACE since June; median create latency
  down from 185 ms to 60 ms, p90 from 447 ms to 89 ms (Perplexity's own
  production numbers, not reproduced here or in the repo).

We found no coverage of the open-source release itself yet; it is a day
old.

## The repository

| | |
| --- | --- |
| License | Apache-2.0, "Copyright 2026 Perplexity AI" ([LICENSE](https://github.com/perplexityai/space/blob/main/LICENSE)); no NOTICE file |
| Created on GitHub | 2026-10-01; 235 stars and 18 forks on 2026-10-15 |
| Version | 0.8.0, the "public baseline". 0.9 is the last planned breaking release; 1.0 freezes the HTTP API, the worker `node-api` gRPC and the in-guest `spaced-bridge` gRPC ([versioning](https://github.com/perplexityai/space/blob/main/docs/versioning.md)) |
| Languages | Rust (about 970k lines including tests, 111 crates, edition 2024), Go (about 38k: the Kubernetes runtime shim), Python (about 100k: smoke suites, benchmarks, examples), shell (about 40k), protobuf (about 8k), a Next.js console (about 11k TS/TSX) |
| Key dependencies | axum 0.8, tonic 0.14, sqlx 0.9 (Postgres), kube 4.2, Firecracker 1.15.1 with jailer, btrfs, optional Redis, S3-compatible object storage |
| Activity | 200 commits in the five days to 2026-10-15; PR numbers past #530. Most from one engineer (105 of 200), then two more with 28 and 15 |
| Maturity | Runs Perplexity's production traffic; public contract pre-1.0. Docs are thorough and precise about non-guarantees. Repo hygiene is strong: `cargo arch` ratchets on file size and forbidden patterns, cargo-deny, supply-chain vetting, gitleaks, privileged-test conventions |
| Agent-friendliness | Ships `AGENTS.md` (with `CLAUDE.md` a symlink), nine repo skills under `.agents/skills`, and a Claude Code plugin marketplace for contributors |

### How it runs

- **Host.** Linux x86_64 with `/dev/kvm`, `/dev/net/tun`, loop devices,
  btrfs and privileged Docker. Not macOS. On a cloud VM it needs nested
  virtualization ([installation](https://github.com/perplexityai/space/blob/main/docs/installation.md)).
- **Topologies.** A local Kind cluster (the documented path), a
  reference EKS deployment (Terraform, applied on a schedule), or the
  **Direct** substrate, where the API calls worker services without
  Kubernetes ([architecture](https://github.com/perplexityai/space/blob/main/docs/architecture.md)).
  A single worker can also be driven with its node CLIs
  (`space-sbxmgr`, `space-tmplmgr`).
- **Services.** API + Postgres (desired state), cluster controller,
  sandbox/node/volume/template/build/credential managers, a network
  gateway, a data plane for command streams, object storage for templates
  and snapshots, and `spaced`, the in-guest daemon that is the only thing
  in the VM that talks to the host.
- **Cloud fit.** The reference infra is AWS: STS token exchange for
  storage, Secrets Manager for credential references, EKS. Nothing in the
  docs targets Google Cloud.

## What SPACE has

| Feature | What it does | Where |
| --- | --- | --- |
| Templates and sandboxes | A template is a starting rootfs + config, referenced by tag or digest; a sandbox is a microVM made from it. Responses report the resolved digest | `docs/architecture.md` |
| Asynchronous operations | Every lifecycle change returns an operation ID to poll; an accepted request is not readiness | `docs/architecture.md` |
| Command sessions | Create a session (gets a process ID and a scoped attach token), then attach to its SSE stream, which starts the process, so no output is lost before you listen | `docs/first-sandbox.md` |
| Pause, suspend, wake | Pause keeps RAM resident; suspend writes a memory + rootfs snapshot and frees the host; wake restores. Idle suspend after six hours by default | `docs/architecture.md` |
| Fork and checkpoint | Copy a sandbox's disk (and snapshot) into a new sandbox; `:checkpoint` on demand | `crates/libs/api/src/routes/public/snapshots.rs` |
| Warm pools and base snapshots | Booted sandboxes kept ready per template release and size (Kubernetes), or restore from a saved memory snapshot (single node) | `docs/warm-pools.md`, `docs/base-snapshots.md` |
| Port, terminal, protocol and file-watch sessions | Ingress to guest services with wake-on-request, interactive terminals, file events | `crates/libs/api/src/public_sessions.rs` |
| Egress policy | Default-deny policies, named operator profiles selected per create, TLS interception with exemptions | `docs/egress-profiles.md`, `docs/tls-interception-exemptions.md` |
| Credential injection | Credential *references* stored per organization; the network gateway resolves them and injects them on outbound requests, so the guest never holds the secret | `crates/libs/credential/injection`, `crates/libs/net/gateway` |
| Scoped storage tokens | Host services get short-lived credentials for one object or prefix through an RFC 8693 token exchange; never ambient credentials | `docs/space/design/credential-token-exchange.md` |
| Execution environment identity | SHA-256 over runtime binary digest, host and guest platform, kernel, boot profile and snapshot format; decoding never equals permission to run | `docs/execution-environments.md` |
| Guest version admission | Host admits a declared range of `spaced` guest versions; old snapshots are never relabelled | `docs/versioning.md` |
| E2B compatibility wrapper | Lets the E2B Python SDK (`e2b==2.21.1`) use SPACE | `docs/ecw.md` |
| Console | Next.js admin UI for sandboxes, templates, warm pools | `ui/` |
| Benchmarks | Scripts for lifecycle, density, concurrency, cross-node and balloon memory; no published results in the repo | `scripts/benchmarks/` |

What it does not have: model routing, an agent loop, a harness, billing,
or anything above "run this process in that VM". It is infrastructure.

## How it compares to what we ship

Our side: [`oa-computers`](../../cloud/computers.md) (intent +
reconcile + Postgres), the [`computer-driver`](../../../crates/computer-driver)
contract with Core / Suspend / Snapshot / Fork profiles and
[`computer-conformance`](../../../crates/computer-conformance), and
[`oa-boat`](../../cloud/oa-boat.md), our Boat-shaped sandbox service on
GCE. The [computers-for-chats audit](../../audits/2026-10-14-computers-for-chats-audit.md)
is the current map.

| Concern | SPACE | Us today |
| --- | --- | --- |
| Isolation unit | Firecracker microVM (own kernel, minimal devices) | A whole GCE VM per computer (Shielded VM, no external IP) |
| Cold create | 60 ms median in Perplexity production (warm pools, btrfs CoW, snapshots) | 77 s to a first turn; 1 to 4.5 min for a fresh computer; no warm pool |
| Suspend | Memory + rootfs snapshot; running processes survive | Stop keeps the disk only; processes and RAM are lost (52 s resume) |
| Fork | Yes, disk and snapshot | Yes, disk through a GCE snapshot |
| Lifecycle model | Desired state in Postgres, controllers reconcile, operation IDs, fenced executions | Same shape: `POST /v1/computers` intent, leased reconciler, revision fence, idempotent operations |
| Driver contract | Native HTTP API (plus E2B wrapper) | `computer-driver` trait + Boat API over HTTP for out-of-process drivers + conformance suite |
| Secrets | Injected at the egress gateway; never in the guest | Per-turn `env` sent over SSH stdin and exported into that one process: never on disk, but the agent process can read it |
| Egress | Default-deny, named profiles, TLS interception | Cloud NAT and a firewall; no per-computer policy |
| Image identity | Content-addressed environment reference | Environment version = a GCE image name and id |
| Cloud | AWS reference; Kind locally | Google Cloud (`openagentsgemini`) |
| Above the VM | Nothing | Coder, the Coder SDK, System 1 routing, chats, projects, metering |

The previous generation of our stack had a Firecracker provisioner on a
nested-virtualization GCE host (`agent-computer-gce-1`, about 1 s boots,
production acceptance 2026-07-12), removed in the August reset; see the
[cloud parallel execution audit](../../cloud/2026-10-02-cloud-parallel-execution-audit.md).
So Firecracker-on-GCE is proven for us; the missing parts are the ones
SPACE is strong on: snapshots, pools, recovery and policy.

### The rest of our stack

SPACE runs below everything else we were asked to compare, so the
overlap is small:

- **Coder and System 1 routing** ([docs/coder](../../coder/README.md),
  [docs/inference](../../inference/README.md)): SPACE has no routing.
  Choosing *where* a turn runs (own Mac, `oa-boat`, a SPACE microVM)
  stays a typed placement decision through the
  [Decisions API](../../api/decisions.md), never a keyword match.
- **Coder SDK and hooks** ([design](../../coder/design/coder-sdk.md),
  `crates/coder-sdk`): the SDK gates tools inside a session; SPACE's
  egress policy and credential injection enforce outside it. They
  complement each other: a hook that denies a tool call plus a gateway
  that never hands the guest a key.
- **Projects** ([projects.md](../../api/projects.md)): no counterpart.
  A project could pin an environment version, which would map to a
  SPACE template digest.
- **Web chat and agents** (`crates/openagents-web`): nothing to take.
  The web's "Where it runs" picker would list a SPACE-backed computer
  like any other.
- **Rust Native and the UI plan** ([docs/ui](../../ui/README.md)): the
  Next.js console is not useful to us.
- **Agent readiness** ([readiness.md](../../agents/readiness.md)):
  SPACE serves its OpenAPI at `/openapi-m1.json` and hosted docs; we
  already publish OpenAPI, `llms.txt` and WebMCP. Nothing new.

## What we should adopt

S = a day or less, M = a few days, L = a week or more (the
[Factory plan](../factory/README.md#build-order-smallest-first) scale).

| # | Adopt | Where it lands | Size |
| --- | --- | --- | --- |
| 1 | **A SPACE driver** behind our driver contract: run SPACE (Direct substrate, or Kind on one host) on nested-virtualization GCE, implement `Driver` over its HTTP API, declare Core + Suspend + Snapshot + Fork, pass `computer-conformance`. This replaces "write our own Firecracker driver" (Factory item 11) | `crates/computer-driver/src/space.rs`; host setup under `deploy/space-host/`; selection through `OA_COMPUTERS_DRIVER=space` in `crates/oa-computers` | Spike M, driver L |
| 2 | **Credentials by reference, injected at egress.** Today a turn's Claude key and GitHub ticket are exported into the agent's process. Move to references the computer never sees: an egress proxy on the computer's network that swaps a placeholder for the real token on matching hosts. With SPACE this comes built in (AWS Secrets Manager today; we would need a Secret Manager provider) | `crates/computer-driver` (`Exec::secret` gains a reference form), `crates/oa-boat` (proxy on the VPC path), `crates/oa-computers` turns | L |
| 3 | **Memory-preserving suspend and a warm pool** as profile semantics: add "suspend keeps processes" as a declared capability, so the reconciler can prefer it for live chats, and a warm-pool row per build | `crates/computer-driver` (`Capabilities`), `crates/coder-working-computer`, `crates/oa-computers` (Factory item 4) | M |
| 4 | **Content-addressed execution environment identity**: hash runtime, kernel, guest platform and boot profile with the image digest, and refuse a create whose build has no artifact for the chosen driver | `crates/coder-environment-build` (the driver-neutral build record in Factory §5) | S to M |
| 5 | **Named egress profiles per computer**: default-deny plus additive, named allow lists chosen at create and immutable for the computer's life | `crates/oa-boat` (firewall/proxy), `crates/oa-computers` create request | M |
| 6 | **Create-then-attach command sessions**: attaching starts the process, so output before the listener exists cannot be lost; scoped per-session attach tokens instead of the service bearer | `crates/oa-boat` commands API, `crates/computer-driver` `exec` | M |
| 7 | **Guest version admission**: the host declares the guest runtime range it admits and refuses others with a definite error, never a timeout; never relabel old images | `docs/cloud/oa-boat.md`, `oa-coder-host` image metadata | S |

Order: 1 first (it answers whether 2, 3 and 5 come for free), 4 and 7
alongside, 2 before any computer runs on someone else's money or
someone else's repository.

## What to skip

| Skip | Why |
| --- | --- |
| The EKS / Kubernetes reference stack in production | AWS-shaped (STS, Secrets Manager, Terraform for EKS) and heavy: Kind, Postgres, optional Redis, S3 gateway, a dozen services. We run Google Cloud and keep one binary with one Postgres per product. SPACE stays an out-of-process driver, never part of `oa-computers` |
| The E2B compatibility wrapper | We standardized on the Boat API shape for out-of-process drivers; a second compatibility surface buys nothing |
| The Next.js console | Our product UI is Rust Native; an admin page over `oa-computers` covers all drivers |
| Vendoring SPACE code into our tree | About a million lines that change dozens of times a day. Run their release as a service; copy ideas, and at most small, attributed pieces |
| Windows or macOS guests | Named in their type system, not implemented |
| Their production latency numbers as a promise | Not reproduced; the repo has benchmark scripts but no published results. Measure on our host (first step below) |

## License: code or ideas?

SPACE is Apache-2.0, the same license as this repository. We may run
it, modify it and copy code from it, including into our crates, provided
we keep the license text and copyright notice with copied files and mark
changed files; Apache-2.0 also carries a patent grant. It has no NOTICE
file to carry forward.

In practice: run it unmodified as a separate service (no obligations
beyond the license staying with it), take ideas freely, and if we copy a
module (the egress-profile parser or the environment-identity types are
the plausible candidates), keep the header and say where it came from in
the commit.

## Workspace rules this respects

- **No keyword routing.** Placement onto a SPACE computer is a typed
  decision (Decisions API, Clef first), not a match on the request text.
- **Google first.** SPACE runs on our GCE project; models for any agent
  inside it stay Vertex Gemini first. Storage would go to GCS through
  its S3-compatible interface, which is untested with SPACE.
- **Bitcoin-only payments.** SPACE has no billing. Computer seconds keep
  flowing to our meter, the `pay-ledger` compute account and plan hours,
  on Bitcoin rails.

## A small first step: SPACE on one GCE host, measured

One spike, no product change. Size M.

1. Create one `n2-standard-8` in `openagentsgemini` with nested
   virtualization on, a local SSD formatted btrfs, no external address,
   label `openagents-managed=space-spike`.
2. Install SPACE `0.8.0` with its Kind installer and run
   `examples/first_sandbox.py`.
3. Build a SPACE template from our `oa-coder-runtime` image's root
   filesystem.
4. Write `SpaceDriver` behind a feature flag in `crates/computer-driver`,
   with a live conformance run gated on `SPACE_API_URL`.
5. Record results in `docs/cloud/evidence/2026-10-space-spike.json` and a
   short note in `docs/cloud/`.

Acceptance checks:

- [ ] `first_sandbox.py` prints `Hello from SPACE` and exits 0 on the
      GCE host.
- [ ] Twenty creates from our template, p50 and p90 recorded, cold and
      with a base snapshot. Pass bar: p50 under 2 s with a base snapshot
      (against 77 s on `oa-boat` today).
- [ ] `coder --version` and `claude --version` run inside a sandbox.
- [ ] A process holding 1 GB in memory survives suspend and wake with
      its state intact; time to wake recorded.
- [ ] `computer-conformance` passes Core and Suspend against
      `SpaceDriver`; Snapshot and Fork pass or are recorded as not yet.
- [ ] An egress policy that allows only `api.github.com` blocks a request
      to another host from inside the sandbox.
- [ ] Teardown leaves no `space-spike` instance or disk; the spike's
      cost is recorded.

If the bar is met, item 1 becomes an issue for a production driver; if
not, we keep `oa-boat`, still do items 2, 4 and 7, and revisit at
SPACE 0.9.

## Sources

- SPACE: <https://github.com/perplexityai/space>, guide
  <https://space.docs.perplexity.ai/guide/index.html>, API reference
  <https://space.docs.perplexity.ai/api/index.html>.
- Announcement: <https://x.com/AravSrinivas/status/2110785848958759165>.
- July 2026 coverage: linked under [The announcement](#the-announcement).
- Ours: [computers](../../cloud/computers.md),
  [oa-boat](../../cloud/oa-boat.md),
  [managed computers](../../cloud/managed-computers.md),
  [computers-for-chats audit](../../audits/2026-10-14-computers-for-chats-audit.md),
  [Factory plan](../factory/README.md),
  [cloud parallel execution audit](../../cloud/2026-10-02-cloud-parallel-execution-audit.md),
  [Coder SDK](../../coder/design/coder-sdk.md),
  [Decisions API](../../api/decisions.md).
