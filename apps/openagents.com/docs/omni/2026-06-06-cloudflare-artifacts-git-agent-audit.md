# Cloudflare Artifacts Git Agent Audit

Date: 2026-06-06

Status: architecture audit and adoption plan only. This note does not change
runtime policy, create a binding, add a migration, grant repository authority,
or move production source control away from GitHub.

## Executive Decision

Cloudflare Artifacts should become OpenAgents product surface's Git-native workspace and closeout
store for agent work, but not the immediate replacement for GitHub as the
customer source, social review, issue, CI, or pull-request system.

The useful split is:

```text
GitHub
  -> upstream source, customer-owned repos, issues, PR review, CI, external
     collaboration, and optional final writeback

Cloudflare Artifacts
  -> OpenAgents product surface-owned Git workspaces, per-mission forks, patch/diff closeout,
     internal baseline repos, public-proof repos, transient runner handoff,
     and PR-less accepted outcomes

R2
  -> opaque blobs, screenshots, bundles, static site source exports, logs after
     redaction, and non-Git artifact files

D1 / Durable Objects / Queues / Workflows
  -> product truth, authority receipts, redaction, sync, events, lifecycle,
     placement, billing, and review state
```

Artifacts helps OpenAgents product surface get away from GitHub in the places where GitHub is being
used only because agents know the `git` CLI: scratch repos, forks, patches,
commits, diffs, and baseline handoff. It does not remove the need for GitHub
when the customer repository is hosted there, when a buyer expects a PR, when
external CI is the verifier, or when GitHub's issue and review graph is part of
the acceptance surface.

The first adoption target should be an internal/public-repo lane:

```text
CodingAutopilotMission
  -> create or fork Cloudflare Artifacts repo
  -> mint short-lived repo write token
  -> SHC/local/Cloudflare Container agent clones with standard git
  -> agent commits into Artifacts
  -> OpenAgents product surface records commit refs, diff refs, test refs, artifact refs, and receipts
  -> optional later GitHub PR writeback after explicit approval
```

This gives local and cloud agents a Git endpoint that is not GitHub, while
keeping OpenAgents product surface's existing approval and projection invariants intact.

## Sources Reviewed

OpenAgents product surface local files:

- `AGENTS.md`
- `INVARIANTS.md`
- `workers/api/wrangler.jsonc`
- `workers/api/src/bindings.ts`
- `workers/api/src/github-write-connections.ts`
- `workers/api/src/github-writeback-authority.ts`
- `workers/api/src/github-pr-fulfillment.ts`
- `docs/2026-06-02-cloudflare-only-openagents-sync-audit.md`
- `docs/2026-06-02-shc-agent-deployment-runbook.md`
- `docs/2026-06-04-cloudflare-containers-runner-backup-audit.md`
- `docs/2026-06-06-coding-autopilot-mission-records.md`
- `docs/2026-06-06-coding-autopilot-decision-actions.md`
- `docs/2026-06-06-coding-autopilot-continuation-decisions.md`
- `docs/2026-06-06-coding-autopilot-artifacts.md`
- `docs/2026-06-06-coding-autopilot-repo-placement.md`
- `docs/2026-06-06-coding-autopilot-repo-memory.md`
- `docs/2026-06-06-coding-autopilot-situational-awareness.md`
- `docs/pylon/2026-06-06-oa-node-managed-machine-contract.md`
- `docs/pylon/2026-06-06-oa-workroomd-sidecar-contract.md`
- `docs/omni/README.md`
- `docs/omni/2026-06-05-workroom-records-v1.md`
- `docs/omni/2026-06-05-workroom-evidence-bundles-v1.md`
- `docs/omni/2026-06-06-knowledge-source-bundle-and-span-model.md`
- `docs/omni/2026-06-06-data-package-export-rights-manifest.md`
- `docs/omni/2026-06-06-model-lab-model-artifact-contract.md`
- `docs/omni/2026-06-06-model-lab-training-run-contract.md`
- `docs/omni/2026-06-06-model-lab-public-report-projection.md`

Workspace-level Omni source material:

- `../docs/omni/coding-on-autopilot-wedge-spec.md`
- `../docs/omni/vortex-coding-agent-cockpit-synthesis.md`
- `../docs/omni/agent-cloud-edge-synthesis.md`
- `../docs/omni/vortex-to-omni-product-gap-analysis-roadmap.md`

Cloudflare sources reviewed on 2026-06-06:

- Cloudflare Artifacts product page:
  <https://www.cloudflare.com/products/artifacts/>
- How Artifacts works:
  <https://developers.cloudflare.com/artifacts/concepts/how-artifacts-works/>
- Workers binding:
  <https://developers.cloudflare.com/artifacts/api/workers-binding/>
- Authentication:
  <https://developers.cloudflare.com/artifacts/guides/authentication/>
- Git protocol:
  <https://developers.cloudflare.com/artifacts/api/git-protocol/>
- REST API get started:
  <https://developers.cloudflare.com/artifacts/get-started/rest-api/>
- Best practices:
  <https://developers.cloudflare.com/artifacts/concepts/best-practices/>
- Pricing:
  <https://developers.cloudflare.com/artifacts/platform/pricing/>
- Limits:
  <https://developers.cloudflare.com/artifacts/platform/limits/>

Recent commits inspected:

- `db98a97e` through `05fab5db` added the Coding on Autopilot mission,
  decision action, continuation, artifact, repo placement, repo memory, and
  situational awareness contracts on 2026-06-06.
- `4a9a159b` through `7ee4a19d` expanded late Omni, Model Lab, Benchmark Cloud,
  Artanis, hosted search, marketplace memory, and Forum/Pylon proof contracts
  on 2026-06-06.
- The current HEAD at the time of this audit was `7ee4a19d`, `Connect Artanis
  to Model Lab context`.

## Current OpenAgents product surface Shape

OpenAgents product surface already has the right product authority split for this adoption.

The Worker is the product control plane. `workers/api/wrangler.jsonc` currently
binds:

- D1 as `OPENAGENTS_DB`;
- R2 as `ARTIFACTS`;
- KV as `AUTH_STORAGE`;
- Queues as `RUNNER_EVENTS` and `ADJUTANT_ENRICHMENT_QUEUE`;
- Durable Objects as `SYNC_ROOM`;
- static web assets from `apps/web/dist`;
- dispatch namespaces for hosted Sites.

That means the name `ARTIFACTS` is already occupied by R2. If OpenAgents product surface adopts
Cloudflare Artifacts, the binding should be explicitly named something like
`GIT_ARTIFACTS`, `REPO_ARTIFACTS`, or `ARTIFACT_REPOS`. Do not overload the
existing R2 artifact binding. In OpenAgents product surface vocabulary, "artifact" already means
proof refs, screenshots, patches, reports, fulfillment receipts, and R2 blobs.
Cloudflare Artifacts is specifically Git-compatible repo storage.

OpenAgents product surface's current Coding on Autopilot contracts define:

- durable mission records with artifact refs;
- decision actions that never perform direct effects;
- continuation decisions that are evidence-only unless routed through an
  approval path;
- artifact records for diffs, patch refs, test runs, PR drafts, PR URLs,
  rollback notes, screenshots, redaction reports, fulfillment receipts, and
  customer notes;
- repo placement decisions based on repo trust tier, data classification,
  backend, workload trust, customer grants, provider grants, and operator
  approvals;
- repo memory records that explicitly forbid keyword routing and require typed
  selectors, semantic embeddings, or manual review;
- situational awareness records that combine mission, artifact, decision
  action, account failover, and repo trust visibility.

OpenAgents product surface's Omni contracts define:

- workrooms as the durable unit that links orders, outcome contracts, sources,
  artifacts, emails, receipts, status, and blockers;
- evidence bundles as typed refs for source commits, generated source, build
  logs, screenshots, deployment URLs, diffs, test reports, receipts, and
  redaction reports;
- public proof bundles and data package exports as projections, not raw file
  hosting;
- knowledge source bundles and spans for source-backed retrieval;
- data classification and trust tiers for public, customer, team, operator,
  legal-sensitive, provider-private, payment-private, and secret-bearing
  material.

The GitHub writeback code is already authority-aware. It distinguishes
customer grants, OpenAgents forks, and OpenAgents app authority. It blocks
external write actions when approval, source access, GitHub write connections,
GitHub write grants, scopes, or app configuration are missing. It records
authority receipts before pull-request fulfillment artifacts are inserted.

That is the important baseline: Cloudflare Artifacts must plug into the
workspace and artifact loop. It must not bypass the GitHub writeback authority
ledger or turn a repo token into implicit approval to mutate customer source.

## What Cloudflare Artifacts Is

Cloudflare presents Artifacts as versioned storage that speaks Git. The product
page emphasizes three properties that matter for OpenAgents product surface:

- every repo can be used by normal Git clients;
- repos can be created, imported, forked, diffed, searched, and managed
  programmatically;
- the service is intended to scale to very large numbers of repos, including
  one repo per agent, branch, or user.

The developer docs make the operating model more precise:

- Artifacts creates Git repos on demand.
- Each repo is isolated and has its own Git history, refs, remote URL, tokens,
  lifecycle, and durable state.
- Namespaces are the top-level container for repos. Namespace and repo name
  form the stable address.
- Forking creates a new repo that starts from an existing repo's history and
  then diverges with its own tokens and lifecycle.
- Access is repo-scoped. Tokens can be `read` or `write`.
- The Worker binding can create, get, list, import, delete, tokenize, revoke,
  and fork repos.
- The REST API can create and manage repos outside Worker code.
- The Git endpoint is standard HTTPS Git. Agents can use normal `git clone`,
  `git fetch`, `git pull`, and `git push`.

The auth split is clean:

| Interface | Credential | Use |
| --- | --- | --- |
| Workers binding | configured `artifacts` binding | Worker code calling `env.GIT_ARTIFACTS` |
| REST API | Cloudflare API token with Artifacts read/edit permissions | control-plane calls from non-Worker systems |
| Git protocol | repo-scoped Artifacts token | `git clone`, `fetch`, `pull`, and `push` |

The Git protocol docs recommend passing the token through `http.extraHeader`
for local workflows:

```sh
git -c http.extraHeader="Authorization: Bearer $ARTIFACTS_TOKEN" \
  clone "$ARTIFACTS_REMOTE" artifacts-clone
```

They also support embedding the token secret in an HTTPS Basic-auth remote for
short-lived, self-contained commands, but OpenAgents product surface should avoid storing or
projecting that form because it puts secret material directly in the URL.

Current published limits and pricing that matter:

| Area | Current doc value |
| --- | --- |
| Control-plane request rate | 2,000 requests per 10 seconds per namespace |
| Git request rate | 2,000 requests per 10 seconds per artifact |
| Maximum storage per repo | 10 GB |
| Maximum account storage | 1 TB, raisable on request |
| Number of repos | Unlimited |
| Number of namespaces | Unlimited |
| Paid operations | first 10,000 per month included, then $0.15 per 1,000 operations |
| Paid storage | first 1 GB per month included, then $0.50 per GB-month |

The Git protocol support detail matters for agent design: clone and fetch
support protocol v1 and v2, push uses standard v1 receive-pack, and push over
protocol v2 receive-pack is not supported. Standard Git clients should still
work, but the runner should not assume every optional protocol capability is
available.

## Why It Helps OpenAgents product surface Get Away From GitHub

Artifacts gives OpenAgents product surface a way to stop using GitHub as the default scratchpad.

Today, if an agent needs to make Git-backed progress, GitHub is the familiar
remote, even when the work is not ready for PR review and even when the repo is
an internal OpenAgents experiment. That creates avoidable dependencies:

- every scratch branch becomes coupled to GitHub availability and auth;
- every agent wants a GitHub token even for private intermediate work;
- PRs can become the accidental artifact before the product has reviewed
  redaction, tests, authority, and customer readiness;
- local and cloud agents need different setup paths if one has GitHub access
  and another does not;
- product-led review state is split between OpenAgents product surface and GitHub too early.

Artifacts changes that because OpenAgents product surface can hand an agent a normal Git remote
without handing it a GitHub repo or GitHub token.

The first benefits are concrete:

1. Per-mission Git workspace repos.

   A Coding on Autopilot mission can get a dedicated Artifacts repo. The repo
   name can include stable mission, workroom, run, and baseline identifiers.
   The agent commits there. Cleanup, archival, export, and deletion are tied to
   the mission lifecycle instead of buried in a branch on a shared GitHub repo.

2. Baseline fork repos.

   For internal OpenAgents work, OpenAgents product surface can maintain stable baseline repos in
   Artifacts and fork them per mission. The Cloudflare best-practices docs
   recommend one repo per agent/session/application and forking from stable
   baselines. That matches OpenAgents product surface's workroom model better than one crowded repo
   with many autonomous branches.

3. Public-source import without ongoing GitHub write authority.

   Artifacts can import public HTTPS remotes. For public GitHub repos, OpenAgents product surface
   can import or refresh a baseline, fork it, and let the agent operate in
   Cloudflare-owned Git storage. The final handoff can be a patch, diff, or
   optional GitHub PR after approval.

4. PR-less accepted outcomes.

   Some accepted work does not need a GitHub PR. A customer or internal user
   may need a patch bundle, generated source, site source export, model-lab
   package, or proof repo. Artifacts can hold the Git history and commit refs;
   OpenAgents product surface can project only safe refs and store opaque/non-Git files in R2.

5. One Git handoff path for local and cloud agents.

   SHC, GCP, Cloudflare Containers, local Codex, local OpenCode, or a future
   Probe runner can all use standard Git over HTTPS. The Worker or REST API
   mints a short-lived write token, the runner clones, the runner pushes, and
   OpenAgents product surface records the resulting refs. The agent does not need to know whether
   the backing store is GitHub, Artifacts, or something else once the
   assignment gives it a remote and token resolution path.

6. Better product authority sequencing.

   Artifacts lets OpenAgents product surface delay GitHub PR creation until the workroom has:

   - evidence bundle refs;
   - diff or patch refs;
   - test/build refs;
   - redaction report refs;
   - authority receipt refs;
   - customer or operator approval refs;
   - accepted/rejected/revision state.

   That fits the existing "Autopilot prepares the work; humans accept the
   outcome; external writes require approval" posture.

7. Fewer long-lived repo credentials.

   Cloudflare Artifacts repo tokens are repo-scoped and can be short-lived.
   OpenAgents product surface should mint per-run write tokens and per-review read tokens. This is
   narrower than a customer GitHub token with broad `repo` and `workflow`
   scopes, especially for internal scratch work.

## Where It Does Not Help

Artifacts does not replace GitHub's whole product surface.

It does not replace:

- GitHub issues as an input source;
- GitHub PR review, comments, checks, branch protection, CODEOWNERS, or merge
  policy;
- GitHub Actions or existing CI integrations;
- social proof from a public GitHub PR;
- customer-owned source hosting;
- private GitHub repo access unless the customer still grants source access;
- GitHub App/OAuth installation flows for customer repos;
- the current `github_writeback_authority` receipt model for branch, commit,
  or PR creation on GitHub.

It also does not replace R2. Git is the wrong storage model for large
screenshots, videos, tarballs, raw logs, model checkpoints, or generated site
bundles that are better stored as blobs with digest refs and retention policy.
R2 remains the blob store. Artifacts becomes one kind of source/artifact ref:
versioned Git state.

Finally, Artifacts is not an execution sandbox. It gives agents a versioned
filesystem remote. It does not decide whether a runner may execute code, access
the internet, resolve provider credentials, use customer source, or publish
results. Those decisions remain in OpenAgents product surface's runner placement, grants, source
authority, workroom, evidence, and approval layers.

## Recommended OpenAgents product surface Model

Add a new Git workspace layer instead of bolting Artifacts onto the GitHub
writeback code.

Suggested service names:

- `GitWorkspaceService`
- `ArtifactsGitWorkspaceService`
- `RepoWorkspaceService`

Suggested binding name:

```jsonc
"artifacts": [
  {
    "binding": "GIT_ARTIFACTS",
    "namespace": "openagents-prod"
  }
]
```

The existing R2 binding should stay:

```jsonc
"r2_buckets": [
  {
    "binding": "ARTIFACTS",
    "bucket_name": "openagents-autopilot-artifacts"
  }
]
```

Suggested D1 tables:

```text
git_workspace_repos
  id
  namespace
  repo_name
  provider              -- cloudflare_artifacts | github | local_mirror later
  remote_ref            -- public-safe ref, not auth URL
  remote_host_ref       -- account/namespace host ref if safe
  source_kind           -- empty | imported_public_https | forked_artifact | pushed_by_runner | customer_source_copy
  source_repo_ref
  source_commit_ref
  mission_id
  workroom_id
  run_id
  repo_trust_tier
  data_classification
  visibility
  status                -- creating | ready | importing | forking | archived | deleted | failed
  public_safe
  created_by_ref
  created_at
  updated_at
  archived_at

git_workspace_token_receipts
  id
  repo_id
  token_ref             -- opaque receipt/ref only; never plaintext token
  scope                 -- read | write
  issued_for_ref        -- runner session, reviewer, workroom, export job
  expires_at
  issued_by_ref
  used_at
  revoked_at
  status                -- issued | used | expired | revoked | failed
  created_at
  updated_at

git_workspace_events
  id
  repo_id
  mission_id
  workroom_id
  run_id
  event_kind            -- created | imported | forked | token_issued | cloned | pushed | diff_recorded | archived | deleted | failed
  commit_ref
  branch_ref
  safe_summary_ref
  receipt_ref
  metadata_json
  created_at
```

Suggested public-safe ref shapes:

```text
git-workspace-repo:REPO_ID
git-workspace-commit:REPO_ID:COMMIT_SHA
git-workspace-branch:REPO_ID:BRANCH_NAME_HASH_OR_SAFE_SLUG
git-workspace-diff:DIFF_ID
git-workspace-token-receipt:TOKEN_RECEIPT_ID
```

Do not project:

- plaintext Artifacts tokens;
- Basic-auth remotes with token secrets in URLs;
- raw private Git remote URLs;
- local filesystem paths;
- raw source archives;
- raw runner logs;
- raw patches before redaction checks;
- provider credentials;
- customer emails;
- wallet/payment material;
- raw timestamps.

The projection split should follow the existing Coding on Autopilot and Omni
contracts:

| Audience | Show |
| --- | --- |
| Public | public-safe repo refs, public commit refs, public diff refs, public proof refs |
| Customer | authorized customer-safe repo refs, artifact refs, diff summaries, test refs |
| Team | team-safe repo/workroom/run refs and route/evidence summaries |
| Operator | safe operational refs, token receipt records without plaintext tokens |

## Agent Workflows

### Flow 1: Internal Public Repo Mission

Use when the source repo is public or OpenAgents-owned and public-safe.

```text
1. User creates CodingAutopilotMission.
2. RepoPlacementPolicy classifies repo as public.
3. Worker imports public GitHub remote or gets existing Artifacts baseline.
4. Worker forks baseline into mission repo.
5. Worker records git_workspace_repos row.
6. Worker mints short-lived write token for runner session.
7. Runner clones Artifacts remote with git extraHeader.
8. Runner runs Codex/OpenCode/Probe.
9. Runner commits and pushes to Artifacts.
10. Runner emits closeout manifest with commit refs, diff refs, tests, and caveats.
11. Worker records evidence bundle, coding artifact records, decision action, and receipt refs.
12. Customer/operator reviews in OpenAgents product surface.
13. Optional GitHub PR writeback uses existing authority gate.
```

This should be the first implementation lane. It reduces GitHub dependence
without touching private customer source.

### Flow 2: Local Agent Git Work

Use when a desktop or Tailnet/local Codex agent needs a repo but no GitHub
remote should be created.

```text
1. Operator or product route creates Artifacts repo through Worker or REST API.
2. OpenAgents product surface issues a short-lived write token receipt.
3. Local agent runs git clone with http.extraHeader.
4. Agent works locally.
5. Agent pushes branch/commit back to Artifacts.
6. OpenAgents product surface reads safe commit refs and records evidence/artifacts.
```

This is a clean way to have local agents do Git work without pushing branches
to GitHub during exploration.

### Flow 3: Cloudflare Container Backup Runner

Use when OpenAgents product surface later adds the `cloudflare_container` runner lane described in
the Containers backup audit.

```text
Worker API
  -> RunnerGatewayService
  -> CloudflareContainerRunnerAdapter
  -> assignment carries git workspace repo ref and token receipt ref
  -> container resolves short-lived token
  -> container clones Artifacts repo
  -> container commits/pushes
  -> callback includes commit refs and manifest refs
  -> Worker records evidence, artifacts, receipts, and sync projections
```

This keeps the container image free of baked GitHub credentials and avoids
requiring a GitHub write connection for internal scratch work.

### Flow 4: Customer Private GitHub Repo

Use a slower path here. Artifacts can help, but it must not erase the customer
grant boundary.

```text
1. Customer grants source access through the existing GitHub/provider path.
2. RepoPlacementPolicy classifies the repo as private/sensitive/infra/etc.
3. Runner clones customer source with the customer-approved grant.
4. Runner may push sanitized patch commits to an OpenAgents product surface Artifacts repo only if
   the workroom's data classification and customer grant allow it.
5. OpenAgents product surface records the Artifacts repo as a private/customer-visible workspace ref.
6. GitHub PR writeback still requires the existing writeback authority gate.
```

Do not assume Artifacts can import private GitHub repos through the documented
public import API. Treat private-source copies into Artifacts as a separate
customer-authorized data movement with rights, retention, redaction, and
deletion policy.

### Flow 5: PR-less Accepted Outcome

Use when a customer or internal operator accepts a patch or generated source
without asking for a GitHub PR.

```text
Artifacts commit/diff
  -> CodingAutopilotArtifactRecord(kind = patch_ref or diff_summary)
  -> OmniEvidenceBundle(entry kind = diff/source_commit/test_report)
  -> MissionBriefing
  -> WorkroomLifecycleDecision(accepted or revision_requested)
  -> DataPackageExport, if portable package is needed
```

This is the real "away from GitHub" path. GitHub becomes one optional external
write target, not the definition of a finished coding artifact.

## Integration With Existing Contracts

### Coding On Autopilot Artifacts

Add Cloudflare Artifacts as the backing store for these existing artifact
kinds:

- `patch_ref`
- `diff_summary`
- `test_run`
- `build_log_summary`
- `pr_draft`
- `rollback_note`
- `redaction_report`
- `fulfillment_receipt`

Do not add raw patch text to public/customer projections. The artifact record
should hold safe refs and summaries; the private repo can hold commits and
branches.

### Repo Placement

Extend `RunnerBackend` only when the runner side changes. Artifacts is not a
runner backend. It is a repo workspace provider.

Add a separate workspace provider enum:

```text
GitWorkspaceProvider =
  | "cloudflare_artifacts"
  | "github"
  | "local_checkout"
```

Placement should answer two questions separately:

```text
Where may this code execute?
Where may this Git workspace be stored?
```

A public repo may execute on SHC or Cloudflare Containers and store workspace
state in Artifacts. A regulated or legal-sensitive repo may execute only on
approved backends and may be blocked from Artifacts until the data policy says
otherwise.

### Repo Memory

Repo memory should store conventions and command facts by semantic selectors
or typed selectors. It should not use Artifacts repo names, commit messages, or
branch names as keyword routing signals.

Allowed:

```text
repo memory says: this repo uses bun test
source evidence: git-workspace-commit:...
retrieval: typed selector or semantic embedding
```

Disallowed:

```text
route to a runner because repo name contains "cloudflare"
select files because commit message contains "test"
```

### Evidence Bundles

Add an evidence entry source authority for Artifacts Git refs:

```text
sourceAuthority = "cloudflare_artifacts_git"
entryKind = "source_commit" | "diff" | "test_report" | "generated_source"
```

The evidence bundle should continue to validate refs as public-safe refs. It
should not fetch the repo or expose the remote URL during projection.

### Data Package Exports

Data package exports can reference Artifacts commits and diff refs as digest
inputs, but they should not make a download URL by default. The current data
package contract says it records digest refs and does not host files or create
download URLs. Keep that boundary.

If OpenAgents product surface later offers a downloadable Git bundle, that needs a separate
approval-gated export route with rights manifest, retention, and redaction
checks.

### GitHub Writeback

Do not merge Artifacts adoption into `github-writeback-authority.ts`.

Instead, use Artifacts before GitHub writeback:

```text
Artifacts repo has reviewed commit
  -> customer/operator requests GitHub PR
  -> GitHubWritebackAuthorityRequest
  -> receipt
  -> GitHub executor creates branch/PR
  -> PR fulfillment artifact recorded
```

This preserves the existing rule that PR creation is an external write that
needs its own authority trail.

## Namespace Strategy

Cloudflare recommends partitioning namespaces by environment, team, and high
rate workload. OpenAgents product surface should not put all repos under `default`.

Recommended namespace shape:

| Namespace | Use |
| --- | --- |
| `openagents-prod-public` | public/internal public-source agent workspaces |
| `openagents-prod-customer` | customer-authorized private workspaces after policy is ready |
| `openagents-prod-model-lab` | Model Lab public-safe eval/training artifact repos |
| `openagents-prod-sites` | Sites source/version repos if Git-backed site export is needed |
| `openagents-staging` | staging and test runs |
| `openagents-dev` | local/dev experiments |

Repo names should include stable identifiers:

```text
mission-{missionId}-{shortRepoSlug}
workroom-{workroomId}-{shortRepoSlug}
run-{runId}-{shortRepoSlug}
model-lab-{artifactId}
site-{siteId}-revision-{revisionId}
```

Avoid short shared names like `repo`, `scratch`, or `starter`. Cleanup and
audit will be much easier if names carry mission/workroom/run identity.

## Security And Authority Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Plaintext token leaks into logs, docs, issue comments, or D1 | Repo mutation until expiry | Store token receipts only; never store token plaintext; prefer `http.extraHeader`; scanner rejects `art_v1_` patterns |
| Basic-auth remote with token gets projected | Secret-bearing URL leak | Ban auth remotes in projections and metadata; runner may use only transient process env |
| Artifact repo becomes private source archive | Customer data migration without approval | Private-source copies require explicit customer grant, rights manifest, retention, deletion policy, and data classification |
| PR-less artifact bypasses code review | User accepts unreviewed work | Mission Briefing must link diff, tests, redaction, rollback, and human acceptance decision |
| Artifacts repo confused with R2 `ARTIFACTS` binding | Implementation mistakes and wrong storage assumptions | Use `GIT_ARTIFACTS` binding name and `GitWorkspace` domain terms |
| Agents use repo name or branch text for routing | Violates semantic routing invariant | Route through existing typed placement, semantic selectors, and structured planner |
| Workspace exceeds 10 GB repo limit | Failed pushes or incomplete closeout | Preflight repo size and reject large binary repos; put blobs in R2 |
| Namespace hot spot | Rate-limit failures | Partition namespaces by environment, workload, and team |
| Cloudflare service beta/product changes | API drift | Keep generated Wrangler types as source of truth; isolate calls in one service |
| GitHub PR state diverges from Artifacts source | Confusing review trail | Record cross-refs: Artifacts commit, GitHub branch, PR URL, authority receipt |

## Implementation Plan

### Phase 0: Spike And Vocabulary Lock

No production mutation yet.

- Add a doc-backed issue plan.
- Choose binding name `GIT_ARTIFACTS`.
- Choose namespace names.
- Define ref prefixes.
- Add `art_v1_` token-shape detection to unsafe material scanners.
- Decide whether the first service name is `GitWorkspaceService` or
  `ArtifactsGitWorkspaceService`.

Acceptance:

- no runtime behavior changes;
- no secrets in docs;
- clear distinction between R2 artifacts and Cloudflare Artifacts repos.

### Phase 1: Internal Public Repo Proof

Add a narrow Worker service around the Artifacts binding.

Capabilities:

- create repo;
- get repo;
- fork repo;
- import public HTTPS repo;
- create read/write token receipt;
- revoke token;
- record D1 rows.

Do not expose broad public API routes yet. Drive it from tests and an
operator-only route or smoke script.

Acceptance:

- test creates a repo/fork in a test namespace;
- test mints a read token and redacts plaintext after handoff;
- D1 stores only refs/receipts;
- projection tests reject plaintext token and auth remote URL.

### Phase 2: Mission Workspace For Internal Runs

Connect to Coding on Autopilot mission records.

Capabilities:

- mission creates or selects Artifacts workspace;
- runner assignment includes workspace repo ref and token receipt ref;
- runner can clone/push via standard Git;
- closeout records commit/diff/test refs as coding artifacts.

Acceptance:

- one internal OpenAgents repo mission closes with an Artifacts commit ref;
- Mission Briefing shows safe diff/test refs;
- no GitHub PR is required for the work to be reviewable.

### Phase 3: Diff Viewer And Evidence Bundle

Add product review surfaces.

Capabilities:

- compare baseline commit to result commit;
- record changed-file summaries;
- link test/build refs;
- generate redaction report;
- attach evidence bundle entries.

Acceptance:

- customer/operator can inspect a diff summary in OpenAgents product surface;
- evidence bundle can cite Artifacts `source_commit` and `diff` refs;
- public projection shows only public-safe refs.

### Phase 4: Optional GitHub PR Bridge

Use existing GitHub writeback authority after Artifacts review.

Capabilities:

- selected Artifacts commit becomes input to PR draft;
- existing GitHub authority gate decides whether PR can be created;
- PR fulfillment artifact stores both Artifacts commit and GitHub PR refs.

Acceptance:

- blocked PR attempts still create safe blocked artifacts/receipts;
- allowed PR attempts include authority receipt;
- GitHub token/grant material never enters Artifacts records.

### Phase 5: Private Customer Repos

Only after data classification, retention, and deletion policy are explicit.

Capabilities:

- customer-grant-gated private workspace storage;
- customer-safe delete/archive/export route;
- rights manifest for portable exports;
- stricter operator audit.

Acceptance:

- private repo Artifacts storage blocked unless customer grant and data policy
  allow it;
- private repo refs are hidden from public/agent projections;
- deletion and archive receipts preserve audit evidence without retaining raw
  private source beyond policy.

## Test And Verification Requirements

Unit tests:

- Artifacts token shape rejected by projection scanners.
- Basic-auth remote URLs rejected by projection scanners.
- Git workspace repo records reject private repo URLs in public projection.
- Public repo workspace can become public proof only if repo trust and data
  classification are public.
- Private/sensitive/legal/payment/regulated repos require grants and/or block.
- Repo memory still requires typed selectors, semantic embeddings, or manual
  review.
- GitHub PR bridge cannot run without `github_writeback_authority` receipt.

Integration smoke:

```text
create test namespace repo
mint write token
git clone with http.extraHeader
create commit
git push
mint read token
git clone/fetch result
record commit ref
revoke token
verify token receipt redaction
```

Product smoke:

```text
create internal CodingAutopilotMission
create Artifacts workspace
dispatch fake or low-risk runner
push commit
record coding artifact
record evidence bundle
render mission briefing
verify public/customer/operator projection splits
```

Formal/model note:

- Model the rule "a Git workspace token grants Git access only; it never grants
  GitHub writeback, customer acceptance, public claim upgrade, payment, or
  deployment authority."
- Convert any counterexample into projection or authority-gate tests.

## Recommendation

Adopt Cloudflare Artifacts now, but start in the narrow lane where it is
strongest:

```text
OpenAgents-owned or public-source Coding on Autopilot workspaces.
```

Do not start with private customer repositories. Do not try to replace GitHub
PRs in the same patch. Do not rename the existing R2 `ARTIFACTS` binding. Do
not treat Artifacts as a runner or sandbox.

The right first production outcome is:

```text
An OpenAgents product surface mission produces a reviewable commit, diff, test summary, redaction
report, and closeout receipt from a Cloudflare Artifacts Git repo, with no
GitHub write token involved.
```

After that works, GitHub becomes a bridge target instead of the place every
agent must work by default.

## Open Questions

- Does the current Cloudflare account have Artifacts access enabled for the
  Workers Paid plan used by `openagents-autopilot`?
- Should the first namespace be `openagents-prod-public` or a staging-only
  namespace until the smoke is stable?
- Should local agents receive tokens through an operator-only API route or an
  out-of-band CLI smoke first?
- Should Artifacts repos be retained per accepted outcome, archived after a
  retention window, or converted to R2 Git bundles for long-term cold storage?
- What is the exact customer-facing language for a PR-less accepted coding
  outcome: "patch", "workspace", "reviewable commit", or "source handoff"?
- Should Model Lab use the same Git workspace service for small eval harnesses,
  or keep model artifact refs separate until raw weight/checkpoint policy is
  clearer?
