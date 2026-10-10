# Dogfood and operations

Audit baseline: `07805e6a7c3513a057d226b488cb2d40fd974a64`, October 10, 2026. This chapter covers using OpenAgents to improve OpenAgents: execution placement, capacity, claims, integration, credentials, approval, recovery, observability, deployment, and operational measurement. The retained dogfood report and issue comments are historical evidence; this audit did not independently repeat their live runs.

There is a real cloud-development proof and substantial local automation. There is not yet a demonstrated, unattended loop from a web or phone request through cloud editing, protected evaluation, global integration, optional Mac work, deployment approval, observed usefulness, and learning. The remaining work is both operational and product-facing. An owner who still needs the Mac's shell, private credentials, and several separate logs to complete the loop is operating useful infrastructure, not yet using the complete self-improving-codebase product.

## The current dogfood evidence

The [dogfood report](../../cloud/dogfood-dev-on-prod.md) records an issue completed from GCE environment `oa-dev-env-1`: [#11224](https://github.com/OpenAgentsInc/openagents/issues/11224), landed as `6652765b03`. It records a build of roughly 333 seconds and a later run of roughly 257 seconds, cache-hit/miss counts, and 63 passing `coder-cloud` tests. It also records practical constraints: large source/build disks, a stale image, credential setup, production IAM limitations, and remaining dependence on the Mac for platform-specific work.

This establishes that the source can be built and changed on that configured environment. It does not establish a representative time-to-accepted-issue distribution, a repeatable customer onboarding flow, cloud-only orchestration, complete cost per accepted change, or a validated learning update. One operational success should remain one operational success in product copy.

The open follow-ups are unusually concrete:

| Issue | Missing operational link | Proof it asks for |
| --- | --- | --- |
| [#11226](https://github.com/OpenAgentsInc/openagents/issues/11226) | Repository-scoped GitHub access from web environments | Web-started run pushes a branch and opens a PR without persisting or printing the token |
| [#11227](https://github.com/OpenAgentsInc/openagents/issues/11227) | A cloud integrator and idle stop | Three branches land serially without the Mac; idle VM stops |
| [#11228](https://github.com/OpenAgentsInc/openagents/issues/11228) | Cloud issue-run visibility in fleet/phone | Live row, stop, and message from both surfaces |
| [#11223](https://github.com/OpenAgentsInc/openagents/issues/11223) | Mac-only work from cloud | Exact-ref build/test/upload/capture, local signing custody, returned artifacts, required approval |
| [#11225](https://github.com/OpenAgentsInc/openagents/issues/11225) | Decisions served by our own endpoint and providers | Production routing/judging without Jev credentials, with Pylon failover demonstrated |
| [#11220](https://github.com/OpenAgentsInc/openagents/issues/11220), [#11219](https://github.com/OpenAgentsInc/openagents/issues/11219) | House-key model/embedding reliability and economics | Vertex-first paths using funded capacity; fallback behavior and deployment |

These issues should stay linked to the product proof. Closing an individual implementation issue proves its delivered scope, not the whole operational claim.

## OPS-01 — There is no single durable owner for the complete improvement attempt

**Priority: P1. Confirmed integration gap.** The new issue-run has an event log and summary, the established task flow has a durable issue-flow record, cloud jobs retain provider and session state, the trace script has replay/admission records, and deployment has its own approval and result. Each is useful. The audited path does not carry one mandatory attempt identity through all of them.

The consequence is more than a missing dashboard. After a crash between push and PR creation, a restarted operator needs to know whether to inspect an existing branch or create another. After a verifier finishes but the summary write fails, learning must not infer success from a surviving UI transcript. After the owner approves a deployment, a later candidate must not inherit that approval. Separate files and command outputs can support manual reconstruction, but autonomous recovery requires typed joins and explicit unknown states.

Use existing owners rather than another scheduler. A top-level attempt can reference issue claim, source/base, environment, task/run, patch, required check plan, replay, integration, deployment, and observed outcome. Each owner remains authoritative for its transition. Required evidence must be durable before the dependent side effect. If an append or save fails, the attempt should report incomplete evidence and stop the dependent promotion; it must not silently continue while the UI still says the run completed.

**Acceptance experiment:** kill the coordinator after dispatch, patch capture, check completion, push, PR creation, merge, and deployment request. Restart twice. Each recovery must find the original effect or keep it unknown, never blindly create a duplicate. Prove the resulting record can explain a failed attempt as clearly as a successful one. Include disk-full and corrupted-record cases, not only process restarts.

## OPS-02 — Claims and landing coordination are local; the cloud integrator remains open

**Priority: P1. Confirmed scope boundary.** The established issue flow uses `coder_lease::claims`, GitHub claim markers, assignment and board helpers, and stale-claim handling. The [lease documentation](../../coder/runtime/leases.md) and closed [#10764](https://github.com/OpenAgentsInc/openagents/issues/10764) define session ownership. This is real coordination infrastructure.

The new issue-run setup reads the issue title/body/state and prepares a worktree without integrating the same claim admission. It can therefore be started on work another task owns unless its caller performs the coordination. That is a product integration gap even if a careful operator follows `AGENTS.md` manually.

For landing, `coder::task::issue_run` has a process mutex and local file lock. It checks, repairs, rebases, and handles push refusal through the landing owner. A local lock does not serialize separate cloud hosts; [#11227](https://github.com/OpenAgentsInc/openagents/issues/11227) explicitly tracks that gap. Push conflict handling prevents some bad writes, but it is not equivalent to one queue with clear ownership, fairness, recovery, and a cost budget.

**Required behavior:** a run must claim before doing issue work and release or transfer the claim when it stops. The integrator should consume immutable candidate references and check evidence, rebase deliberately, and run the required affected checks after relevant base movement. Preserve the distinction between a claim, a branch lease, a build lease, and publication authority. A claim grants neither permission to deploy nor permission to disclose code to another provider.

**Acceptance experiment:** two sessions on one host and one session on another race for the same issue. Exactly one is admitted under the selected coordination policy. Then three independent branches land in a single cloud queue while one needs a conflict repair and one fails checks. Restart the integrator during a push and verify the same result is recovered, the failed branch remains inspectable, and board/issue state agrees with the actual landed commit. Do not require the full release gate for ordinary integration.

## OPS-03 — Capacity controls exist, but the new learning paths do not uniformly use them

**Priority: P1. Confirmed source and operational gap.** The repository already has counted build leases, quiet leases, placement classes, target-slot accounting, disk reclaim, browser isolation, durable scratch, and usage-limit coordination. [#10768](https://github.com/OpenAgentsInc/openagents/issues/10768) records the broader infrastructure effort; [#10756](https://github.com/OpenAgentsInc/openagents/issues/10756) covers build shims and nested-lease behavior. These should be the common execution substrate.

The experimental issue-run launches checks directly. The A/B harness serializes through its own host lock and target directory. Local trace replay creates temporary check resources and has its own disk floor. These paths can be safe in a carefully arranged experiment, but they do not establish one enforced capacity model for concurrent agent work, replay, training, inference, and release activity. The quiet lease is especially relevant to measuring decision-model or file-finder latency: competing builds can make a model update appear faster or slower without any model change.

Disk pressure is already an observed limitation. The [#11218 closing report](https://github.com/OpenAgentsInc/openagents/issues/11218#issuecomment-6097838645) says the Mac had 0–16 GB free, below the replay path's 30 GB floor; replay was done on the build host instead. Historical [#10251](https://github.com/OpenAgentsInc/openagents/issues/10251) records a supposedly warm Boat fork taking 701 seconds for a no-op build because restored paths and artifact access defeated the expected warmth. An image with a large target directory is not evidence of a warm customer task.

**Required measurement:** record queue wait, provision, hydrate, clone/fetch, briefing, active agent, compile/test, independent replay, integration, and idle time separately. Record host load and cache state with performance evidence. Report cold and warm distributions rather than one best run. Include failed attempts, retries, and cleanup cost in cost per accepted change.

**Acceptance experiment:** run a bounded cohort under the existing capacity owner, with explicit cold/warm conditions and one concurrent background load case. Prove builds queue, quiet work excludes conflicting loads, disk admission refuses safely, and cancellation releases resources. Keep one long-lived external target per slot and durable scratch; do not “fix” the problem by making every attempt a cold build or deleting another agent's cache.

## OPS-04 — The product must separate developer credentials from customer execution

**Priority: P1 for external repository use. Confirmed dogfood-to-product gap.** The dogfood setup uses deliberately configured credentials and host access for the owner. That is useful for developing the system, but it is not an automatically reusable customer security model. [#11226](https://github.com/OpenAgentsInc/openagents/issues/11226) states that web environment runs currently cannot push or open PRs because they lack the customer's GitHub credential, and asks for short-lived repository-scoped access from the signed-in person's connection.

The repository's authority design is stronger than a generic “agent has access” flag. Closed [#10708](https://github.com/OpenAgentsInc/openagents/issues/10708) separates observation, execution, disclosure, and spending; [#10712](https://github.com/OpenAgentsInc/openagents/issues/10712) confines admitted source/provider credentials. Closed [#10848](https://github.com/OpenAgentsInc/openagents/issues/10848) requires current team policy before each enabled route. Self-improving codebases must adopt these existing distinctions.

A repository grant needs exact repository and branch scope, permitted operations, duration, revocation, recipient/provider, and payer. A verifier generally needs no push credential. A training job needs only authorized corpus access. An integrator may need permission to update a branch; production deployment needs separately granted authority. Provider fallback must not silently disclose private source to a provider outside the user's policy.

The earlier health audit reported cloud transport and secret-lifetime risks. One remains source-visible: [`coder-cloud::pool`](../../../crates/coder-cloud/src/pool.rs) uses `StrictHostKeyChecking=no` and `UserKnownHostsFile=/dev/null`. This does not prove interception, but the selected route needs an authenticated remote identity before it is described as suitable for sensitive customer code. Broader cloud credential cleanup findings require a focused re-audit of every error transition; they should not be assumed fixed because the happy-path teardown clears credentials.

**Acceptance experiment:** use two synthetic customers, two repositories, and a revoked token. The second customer must never inherit the first's checkout, credential, cache content, or training permission. A worker cannot push main; a verifier cannot push any branch; a denied provider fallback sends no source. Credentials must be absent from image snapshots, logs, trace artifacts, command lines, and the learning corpus. Rotate/revoke during a paused task and recheck before its next effect.

## OPS-05 — Production approval is implemented, but recursive changes need release-level binding

**Priority: P1 integration requirement; do not mislabel existing approvals as absent.** [`openagents deploy`](../../../crates/openagents-cli/src/deploy.rs) supports staging and exact-image production promotion. Production validates a SHA-256 image identity, consumes approval for that digest, creates a no-traffic revision, runs its smoke, and moves traffic only after success. It returns the prior revision and rollback command. This is a useful boundary for self-improvement.

A learning update can affect more than a web image: file-ranking weights, calibration, prompts, model routing, provider selection, thresholds, evaluator definitions, or a tool manifest. The release identity must bind the set of artifacts that changes behavior. An approval for one image should not authorize an unpinned external model artifact or a later mutable configuration update. A successful benchmark also does not authorize a production change; evaluation and authority remain different.

The product should reuse the approval owner for the exact proposed release and display a concise result: what changed, the expected benefit, evidence scope, known regressions, and rollback. It should preserve the owner's ability to approve a narrow standing policy for low-risk updates, while keeping policy changes and broader authority separate. Do not insert unnecessary approval prompts into already authorized routine code work; the requirement is correct scope, not maximum friction.

**Acceptance experiment:** approve release A, substitute one behavioral artifact or target, and require refusal. Promote a valid candidate with a failing smoke and confirm no traffic shift. Interrupt after creating the no-traffic revision and recover it without a second promotion. Roll back both behavior and configuration, and confirm subsequent traces name the actual served release. Treat uncertain deployment outcome as unknown until the serving revision is observed.

## OPS-06 — Mac handoff and cross-surface control remain qualification gaps

**Priority: P1 for the claimed cloud-first, phone-supervised workflow.** [#11223](https://github.com/OpenAgentsInc/openagents/issues/11223) describes the right Mac handoff: capability advertisement, a typed job tied to a repository ref, a local worktree, streamed logs and returned artifacts, local signing keys, and approval for store uploads. The audit did not find a completed proof of that whole path in the retained dogfood evidence.

This matters for our own repository because Rust source can affect iOS, macOS, Android, native rendering, thin platform glue, release packaging, and linked-host behavior. A Linux build is valuable evidence for its selected targets, not evidence that a mobile release works. Conversely, platform-specific qualification should not block unrelated ordinary changes when repository policy does not require it.

The web fleet is implemented in a narrower scope. [#11164's closing comment](https://github.com/OpenAgentsInc/openagents/issues/11164#issuecomment-6093496239) records the Agents panel, Stop/Message, results, cost, and paused-until fields with targeted tests; staging acceptance was still listed for owner qualification. [#11228](https://github.com/OpenAgentsInc/openagents/issues/11228) separately identifies cloud issue-runs missing from web/phone fleet rows. Closed UI work therefore does not prove every backend run appears there.

**Acceptance experiment:** launch one issue from the web, observe it from the phone, disconnect both, reconnect, steer the original run, and cancel it. Show requested versus acknowledged cancellation. Then execute a harmless Mac-only verification on an exact ref with synthetic signing material or no signing. Verify identity, artifacts, and cleanup before qualifying any real store-upload route. Smoke-created tasks must be archived and fixture runs must use scratch hosts, not pollute the owner's chat lists.

## OPS-07 — Decision-provider changes can break both availability and measurement

**Priority: P1. Current open work.** [#11225](https://github.com/OpenAgentsInc/openagents/issues/11225) changes the default architecture: our endpoint dispatches typed decisions to connected Pylons running Clef, then hosted Clef, then Vertex Gemini, with Jev optional and off by default. Earlier open issues [#11189](https://github.com/OpenAgentsInc/openagents/issues/11189), [#11191](https://github.com/OpenAgentsInc/openagents/issues/11191), and [#11192](https://github.com/OpenAgentsInc/openagents/issues/11192) describe older local/fallback/payment stages. Treat them as overlapping history, not four independent completed architectures.

Availability and learning are coupled here. A briefing improvement measured on one decision model may regress after fallback to another. Probabilities from one door cannot be treated as calibrated probabilities from another merely because the response shape is valid. Model admission limits, question count, option count, token budget, refusal handling, and deadlines can change the selected path. A fallback that silently broadens data disclosure is also an authority defect.

The open Vertex-first work is driven by an actual operational constraint: #11219 records exhausted/degraded house-key routes. That evidence makes budget and provider status part of product reliability, not an optional finance dashboard. Quota fairness also matters: [#11190](https://github.com/OpenAgentsInc/openagents/issues/11190) reports shared tenant-level door quota and concurrency across signed-up personal workspaces.

**Acceptance experiment:** remove the Jev credential, stop the first Pylon during an in-flight request, exhaust a configured provider budget, and send an over-limit decision. Require bounded, policy-valid failover or a typed refusal. Every answer and downstream trace must name the served model/artifact/provider and decision contract. Compare quality by actual served route, including fallbacks, rather than pooling them into one apparent model improvement. Keep the model's quality gate separate from routing availability and customer authorization.

## OPS-08 — The operational success metric is not yet an accepted improvement metric

**Priority: P1 for product claims and optimization.** The codebase can count tasks, runs, tokens, costs, traces, checks, and commits. A recursive improvement system needs a joined denominator: all authorized attempts in a fixed cohort, including cancellations, unavailable providers, failed checks, integration failures, reversions, and rejected changes. Otherwise it can appear to improve by dropping difficult work or moving expense into setup, verification, or the owner.

For our first product cohort, report:

- Accepted useful changes per attempted issue, with the task class and acceptance authority stated.
- Time to accepted change, separating machine time, queue time, and human review time.
- Total attributable cost per accepted change, including briefing, fallback calls, failed attempts, checking, replay, integration, idle compute, and storage.
- First-pass acceptance, repair rounds, escaped defects, reversions, and follow-up issue rate over a stated observation window.
- Coverage and uncertainty: what was checked, what required a real device or owner action, and what remains unknown.
- Reuse benefit: whether a learned change improves fresh tasks against a frozen baseline without widening authority or weakening checks.

These are a recommended measurement contract, not invented current performance numbers. [#11131](https://github.com/OpenAgentsInc/openagents/issues/11131) already calls for reproducible benchmark claims and second-machine reproduction. A benchmark page should expose evidence scope and failures rather than extrapolate an eight-task study or three-issue replay cohort to all customer codebases.

The unit of operational adoption should also stay explicit. [#10672](https://github.com/OpenAgentsInc/openagents/issues/10672) distinguishes authored, published, installed, invoked, externally validated, adopted, credited, and settled. A new briefing template being invoked is not proof it helped. A trace entering a training corpus is not proof a model improved. A model winning a benchmark is not proof it was deployed or helped a customer.

## How this audit updates the previous health findings

The [health remediation record](../2026-10-10-codebase-health-audit/remediation.md) reports fixes for several severe findings. Do not repeat the original health report as though nothing changed. Current source corroborates the narrowed read-only MCP command admission in [`openagents-cli::mcp`](../../../crates/openagents-cli/src/mcp.rs), including refusal of money/key/remote-shell groups and dangerous switches. Current delegate code delegates credential-name decisions to the shared screen. Those changes reduce real risks.

The remediation record also reports payment-journal locking, registry/ledger locking, relay authorization/rate-limit fixes, receipt-writer changes, shared-key access serialization, generated-path hygiene, and improved verification selection. This chapter does not claim to have re-executed every fix's regression test. They remain reported remediations with the scope documented there. The new runner's direct subprocesses, cache, worktree reuse, and unguarded PR path are separate current findings; a fix in a different owner does not automatically cover them.

Remaining structural risks from the assigned health chapters include blocking subprocess/provider work in async owners, boundedness of event/output handling, process-local coordination, duplicated protocol adapters, and large owners that are hard to audit. They matter when they cause a concrete missing deadline, stale authority, duplicated effect, or lost evidence. Refactoring every large file is not a prerequisite to useful self-improvement; fixing the affected transitions and qualifying the selected path is.

## A bounded own-codebase qualification sequence

First, repair evaluator truthfulness and candidate identity from the execution chapter. Then route the briefing experiment through the established task, claim, lease, and check-plan owners. Retain a complete end-to-end attempt record. Select one narrow issue class and freeze the comparison before running it.

Next, qualify the cloud-only landing path with several independent branches and failure injection. Add the customer's repository-scoped token path and accurate fleet visibility. Qualify Mac handoff only for tasks that need it. Preserve exact production approval and rollback. Measure a baseline that includes setup and human effort before attributing an improvement to the learned layer.

Finally, run one full learning cycle whose evidence chain can be followed in both directions: new issue → attempt → independently checked outcome → authorized example → trained/versioned decision artifact → held-out evaluation → approved release → fresh-issue benefit. Keep a simultaneous rollback candidate and verify that reverting the learned artifact restores the prior behavior. This is the first operational proof of recursive improvement; a larger agent fleet is a scaling decision after that proof.

All of these qualifications can follow the repository's current velocity rules. Use targeted tests and existing non-GitHub infrastructure. Documentation-only audit work does not require Cargo. Close implementation issues when code, scoped checks, merge, and required host deployment are complete; record owner-only credentials, real-money activity, device checks, or store releases in `NEEDS_OWNER.md`. The product evidence should continue to show those qualification limits without keeping code-complete issues artificially open.
