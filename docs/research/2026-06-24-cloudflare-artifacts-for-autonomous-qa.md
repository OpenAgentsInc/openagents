# Cloudflare Artifacts for the autonomous-QA agent — fit assessment (2026-06-24)

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


> Goal: minimize the QA agent's dependence on GitHub. This note assesses whether
> **Cloudflare Artifacts** ([docs](https://developers.cloudflare.com/artifacts/)) helps.
> Builds on the prior repo audit
> [`apps/openagents.com/docs/omni/2026-06-06-cloudflare-artifacts-git-agent-audit.md`](../../apps/openagents.com/docs/omni/2026-06-06-cloudflare-artifacts-git-agent-audit.md).
> Not a runtime change; no binding/migration created here.

## What Cloudflare Artifacts actually is

From the live docs (2026-06-24): **a Git-compatible *repository* store** — *"Artifacts
stores versioned file trees behind a Git-compatible interface,"* addressable from
*"Workers, the REST API, and Git clients."* It is for **version-controlled file
structures (git repos / code / trees)**, designed for programmatic repo creation +
parallel-execution isolation for agents. It is **currently closed beta** (access by
form). It is **NOT** an object store: the docs make no mention of serving videos,
images, or arbitrary binaries over a URL. That's R2's job. This matches our prior
2026-06-06 audit's split (Artifacts = git workspaces/closeouts; R2 = opaque blobs).

## What the QA agent actually produces, and where each piece belongs

| QA artifact | Nature | Right home | GitHub today? |
|---|---|---|---|
| Session **video** (mp4/webm) | binary blob | **R2** + served on our `/trace/{uuid}` | gh-attach (being removed) |
| **Screenshots** | binary blobs | **R2** + `/trace` | — |
| **ATIF trace** (JSON projection) | structured doc | **D1 + R2 offload** + `/trace` | never |
| **Distilled `*.e2e.test.ts`** | code (git-tracked) | the repo under test | GitHub (customer) / could be Artifacts (owned) |
| **Diff / patch / closeout** | code (git) | the repo under test | GitHub (customer) / could be Artifacts (owned) |
| **Verdict / receipt** | structured doc | D1 + receipts | never |

## Verdict — does Artifacts help the QA agent? Narrowly, and not for the part that matters most.

**1. It does NOT help the artifacts that drive our GitHub dependence today** — the
**video, screenshots, and trace**. Those are binaries/JSON, not git trees; Artifacts
can't store/serve them. The real "get QA media off GitHub" win is **R2 + our own
`/trace/{uuid}` surface** — already in flight (#6223: upload blob bytes to R2 +
`GET /api/traces/{uuid}/blob/{r2Key}`, visibility-gated). Once that lands, the video
plays from our domain and the PR can stop using GitHub-hosted attachments entirely.
Artifacts adds nothing here.

**2. It DOES help one real slice: a non-GitHub *git* home for the agent's code
artifacts** — the distilled e2e tests, diffs, and baseline/public-proof repos — **when
the target is an OWNED repo** (internal regression suites, our own product, public-proof
repos, PR-less accepted outcomes). This is exactly the 2026-06-06 audit's thesis: agents
reach for GitHub only because they know `git`; Artifacts gives them a git endpoint that
isn't GitHub (clone/commit/diff/branch), so internal QA closeouts need no GitHub repo,
PR, or CI.

**3. It does NOT remove GitHub from the customer-facing flow.** The product use case —
QA *a customer's* repo and open a **PR they review** (e.g. an `executor`-style eval) — is
irreducibly GitHub: that's where the customer's code, review graph, and "show it working"
PR live. Artifacts is for *our* repos, not theirs.

## Recommendation

1. **Media/trace off GitHub → R2 + `/trace` (already the plan, #6223).** This is the
   high-value dependency cut for QA. Finish + deploy it; then drop gh-attach from the
   demo PR and point at the trace's own player.
2. **Request Cloudflare Artifacts closed-beta access** and adopt it for the **owned-repo
   git-closeout lane** (internal/public-proof QA repos, distilled-test baselines, PR-less
   accepted outcomes) per the 2026-06-06 plan — a complementary, lower-priority cut. Gate:
   beta access + a short-lived repo-write-token flow; keep our approval/projection
   invariants.
3. **Do not expect Artifacts to GitHub-free the customer QA→PR flow.** Keep GitHub for
   customer repos + the reviewable PR; that's the product, not a dependency to remove.

**Net:** Cloudflare Artifacts is genuinely useful to OpenAgents (a non-GitHub git store
for agent work), but for the **autonomous-QA agent specifically** its benefit is
**secondary** — the QA artifacts that drive GitHub dependence are *blobs/traces*, which
belong in **R2 + our own trace surface** (in flight), not in a git store. Adopt Artifacts
for owned-repo code closeouts; don't route QA media through it.
