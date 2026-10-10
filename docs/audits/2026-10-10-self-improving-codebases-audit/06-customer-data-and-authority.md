# Customer data and authority

This chapter reviews the boundary between own-repository learning and a service that can learn from customer work. It is based on source revision `07805e6a7c3513a057d226b488cb2d40fd974a64`, the requested sales and health-audit documents, and the issue snapshot described in the audit methodology. It reports code behavior and product requirements; it does not certify legal compliance or establish that any customer data was disclosed.

The repository has useful controls to reuse: workspace-bound provider keys, separate host grants, private trace upload, explicit sharing, sales suppression and tracked-copy cleanup, protected training partitions, and exact evidence references. These controls belong to different products and stores. The central gap is the absence of a demonstrated, single chain from a customer's permitted purpose through every copy, training build, served artifact, and later withdrawal.

For own-repository dogfood, public code and owner-run traces make a narrow research workflow practical. They do not make all local files, issue bodies, prompts, provider outputs, contributors' information, or future customer repositories freely reusable. A replay receipt proves something about execution; it is not a data-use grant.

## The permissions that must remain distinct

| Permission | What it can authorize | What it cannot imply |
| --- | --- | --- |
| Repository access | Read the named repository under the granted identity | Training, sending to every model provider, or publishing changes |
| Task execution | Perform the bounded task with specified tools and budget | Merge, deploy, transfer secrets, or increase the budget |
| Provider disclosure | Send selected inputs to the named provider or permitted provider set | Training a shared OpenAgents model |
| Private account sync | Store and observe the user's selected history on their account | Public sharing or corpus admission |
| Trace sharing | Publish the explicitly selected trace scope | Live terminal access, host control, training, or unrestricted descendants |
| Tenant learning | Fit the named tenant's model for the agreed purpose | Shared-model use, benchmarks, marketing examples, or sale of data |
| Shared learning | Use the specifically permitted material across customers | Relax the original retention or redistribution limits |
| Delivery acceptance | Accept an exact candidate under agreed checks | New spending, further tasks, publication, or training consent |
| Payment | Settle the named invoice or obligation | Model deployment or authority over another workspace |

These permissions need not all be separate dialogs. A durable, bounded agreement or owner policy can authorize a sequence of actions. The implementation must preserve the scope and recheck it at the effect that matters. A learner's confidence, a copied memory, or the existence of credentials must not expand that scope.

## Findings

### DATA-01 — General chat policy and the codebase product have different training defaults

**Priority: high before customer enrollment.** Closed [#11044](https://github.com/OpenAgentsInc/openagents/issues/11044) records an explicit owner decision: chats may be used for training, with paid-plan opt-outs by arrangement. The current [Privacy Policy](../../../crates/openagents-web/content/legal/privacy.md), [Terms](../../../crates/openagents-web/content/legal/terms.md), [privacy documentation](../../../crates/openagents-web/content/docs/privacy-and-security.md), and [chat knowledge note](../../../knowledge/openagents/openagents.chat-privacy.md) reflect that decision. This audit must not revive the older claim that the whole platform never trains on chats.

The [self-improving-codebases proposal](../../product/self-improving-codebases.md) instead requires customer opt-in and tenant-local training, with separate written permission for shared models. [Pilot v1](../../sales/README.md) excludes training, benchmarks, examples, and marketing reuse without separate permission. Those are narrower product commitments. A conversation about a customer repository can cross the chat, task, trace, and corpus surfaces without changing its underlying sensitivity.

The reviewed documents do not establish a machine-enforced precedence rule for that crossing. The risk is not the existence of either policy in isolation; it is admitting a codebase customer's material under a general chat default after promising a narrower pilot contract.

**Acceptance:** record the product, purpose, agreement version, account/workspace, repository, material scope, recipient set, training scope, expiry, and withdrawal state at collection. The narrower customer agreement must follow derived copies. An agreed chat opt-out and a codebase-pilot exclusion must both block inadmissible corpus use, including after an export/import or account migration. Have the owner resolve conflicting product copy before enrollment; do not silently amend an already accepted agreement.

### DATA-02 — Corpus provenance is documentation, not an enforced consent service

**Priority: high before multi-customer training.** [Tenancy training](../../../crates/tenancy/src/training.rs) is a useful local artifact book. `CorpusItem` binds source, license, permission, label, group, partition, and optional teacher information into its item digest. It rejects empty provenance fields, unconfirmed model labels, cross-partition groups, and detected cross-partition duplicates.

`Corpus::validate`, however, checks `permission` as a nonempty string. It does not resolve an active grant, match the caller to the stated workspace, or verify allowed purpose and recipients. `Book::open` creates one local training directory, and `register_corpus` addresses corpora by name within that directory. `Corpus::digest_of` hashes the item list, not the top-level workspace or retention fields. `Retention` records days, access, and artifact terms; the reviewed validation does not enforce an expiry schedule or access-control policy from those fields.

This is a boundary limitation, not evidence of an exposed cross-tenant endpoint. The local operator is presently part of the trust model. It becomes a product defect if a customer service treats a validated corpus as proof of consent or assumes the library alone isolates workspaces.

**Acceptance:** add a service admission boundary that resolves every permission reference to current authority for the exact workspace, repository, purpose, and artifact class. Bind that authority and retention policy into a versioned manifest identity. Keep data partitions separate from ownership boundaries: a correct train/test split does not establish tenant isolation. Reject revoked, expired, wrong-workspace, wrong-purpose, and fabricated permission references. Qualification must include two workspaces with identical corpus names and overlapping issue numbers.

### DATA-03 — Corpus deletion leaves the optional teacher payload intact

**Priority: high before customer training.** In [training.rs](../../../crates/tenancy/src/training.rs), `Book::delete_corpus` replaces `state` with null, removes `question` and `label_rule`, clears `label`, and resets annotations. It preserves `teacher: Option<Value>`. That field is arbitrary JSON intended to carry a judge or teacher answer and can contain copied source text or customer output. Deleting the corpus therefore does not remove all item content permitted by the schema.

The routine also leaves source/license/permission strings and item/group identifiers. Some of these are needed for audit references, but they are unrestricted strings rather than a proven content-free tombstone schema. The current implementation prevents reopening a tombstoned corpus for normal use, which is valuable, but unreadability through one API is different from erasure of the stored content. No customer disclosure was observed in this audit. The [learning chapter](03-learning-and-evaluation.md) covers the same defect as LEARN-08; this chapter owns its customer-data consequences.

**Acceptance:** define a minimal tombstone type with only approved non-content identifiers, digests, deletion time, and reason. Remove or minimize every content-bearing field, including teacher data. Test a distinctive synthetic phrase in each field and nested JSON value, delete, reopen the raw store, and confirm that only the approved metadata remains. Repeat after schema migration and failed-write recovery. Do not describe this local tombstone as deletion of raw trace blobs, caches, exports, backups, or already trained weights; those need the lifecycle described in DATA-08.

### DATA-04 — Finder state does not yet provide repository and customer isolation

**Priority: high for S5 and customer use.** In [filefind.py](../../../scripts/filefind/filefind.py), `default_cache` derives the cache directory from the basename of the common Git checkout. Two repositories with the same basename can select the same cache directory. Worktree sharing is useful; basename sharing across unrelated repositories is not a sufficient product identity.

`cmd_feedback` defaults to scanning `~/.openagents/coder-new/issue-runs`. Its retained rows contain an issue number, path, kind, and source label. They do not bind a stable repository, workspace, accepted data-use grant, or deletion state. The legacy issue-run path accepts a check-passing summary and patch without requiring the independent replay path; the explicit `--traces` path checks a replay verdict. The [post-merge hook](../../../scripts/filefind/post-merge-hook.sh) invokes feedback with the defaults.

This is tolerable only within the current single-repository operator convention. Copying the finder to another repository or using one machine for several customers can mix feedback and cache identities. A correct path that exists in both repositories would be especially difficult to notice by inspection alone.

**Acceptance:** key stores by stable repository identity plus workspace and policy scope, with an explicit mapping that preserves intended worktree sharing. Bind every run and feedback row to that identity and reject mismatches before reading content. Namespace issue IDs by repository. Distinguish measured replay labels from unverified feedback. Test same-name repositories, forked repositories, changed remotes, identical issue numbers, restored caches, and withdrawal before the next feedback build. Training and index refresh must exclude revoked inputs, not merely stop adding new ones.

### DATA-05 — Trace admission currently assumes the public own-repository research context

**Priority: high before accepting customer traces.** [traces.py](../../../scripts/bench/traces/traces.py) stores patches and briefing bytes directly by digest through `put_blob`. `base_record` includes the source path, issue number, and base commit but no stable repository/workspace or purpose identity. `corpus_items` automatically assigns the OpenAgents repository source, Apache-2.0 license, and an owner-run public-repository permission string. Replay verifies execution facts; it does not verify those rights assertions.

The reviewed capture/admit path does not call the shared secret screen or resolve customer permission before storing these bytes. File creation follows the process's filesystem defaults rather than explicitly setting the private modes used in several sales stores. A caller can override `--repo` and `--store`, but those flags do not change the hardcoded rights statement into an authorized customer contract.

The web upload path is different and has real controls: [openagents-web traces](../../../crates/openagents-web/src/traces.rs) validates ATIF, rejects recognized credential shapes, scopes IDs to the account, defaults to private storage, and bounds traces and agent trees. Those controls must not be assumed to cover a local Python path that bypasses that API.

**Acceptance:** retain an explicit own-public-repository mode, and refuse other inputs until the customer admission contract exists. For the customer path, screen before raw retention and before each disclosure, bind repository/workspace/purpose, require current rights, enforce private file modes, and retain a bounded rejection record without the rejected secret. The secret screen is a rule-based filter, not a completeness guarantee. Test patch, briefing, tool output, nested JSON, path metadata, and malformed input; a verified replay must not bypass data admission.

### DATA-06 — Provider fallback must preserve the customer's recipient and privacy choices

**Priority: high for private data; medium for own public-code experiments.** Closed [#11040](https://github.com/OpenAgentsInc/openagents/issues/11040) added no-retention/no-training request controls to the chat model paths. The current [inference router](../../../crates/inference/src/router.rs) filters privacy, payer, capabilities, price, and capacity. [BYOK](../../../crates/gateway/src/inference_byok.rs) is workspace-bound, and `pay: "mine"` must not fall back to company keys. These are useful existing boundaries.

They do not establish one universal promise across every decision, embedding, trainer, SDK, and fallback call. The [chat knowledge note](../../../knowledge/openagents/openagents.chat-privacy.md) explicitly distinguishes chat-model requests from decision and embedding calls. The finder selects available embedding credentials, including Vertex or OpenRouter, and [post-merge refresh](../../../scripts/filefind/post-merge-hook.sh) can send source-derived inputs without a per-repository disclosure gate. Open [#11220](https://github.com/OpenAgentsInc/openagents/issues/11220) and [#11225](https://github.com/OpenAgentsInc/openagents/issues/11225) change provider routing again; a default-provider migration is also a data-recipient change.

**Acceptance:** compute the allowed recipient set once from the customer's current agreement and enforce it at every outbound call, including retries, embeddings, summaries, model judges, and fallback. Record which provider and model actually received which input class. A provider outage must produce a refusal when no allowed substitute exists. Requests expressing a privacy preference are evidence of what was requested, not proof of a provider's internal behavior. Keep owner-run public-code research and customer-private processing profiles separate.

### DATA-07 — The learner must not acquire approval, spending, or publication authority

**Priority: high for unattended operation.** The repository already distinguishes read-only history observation, host execution rights, terminal control, paired devices, task submission, and owner-configured auto-start. Sales provides a similar distinction: [sales.rs](../../../crates/coder/src/task/sales.rs) checks current role and lead scope; [privacy.rs](../../../crates/coder/src/task/sales/privacy.rs) keeps suppression independent of surviving lead records; agent identity and memory do not create contact permission.

The recursive codebase product must extend this principle to its own loop. Learned scores may recommend a file, a worker, or a candidate. They must not rewrite the protected checks, increase a budget, expand repository credentials, authorize their own release, or turn a public issue into permission to contact a customer. Treat repository documents, issue text, model outputs, retrieved traces, and contributed datasets as task data at authority boundaries.

**Acceptance:** freeze the task authority, allowed effects, budget, check definitions, and publication policy before execution. Require an independent authority to change them. A durable owner policy can permit routine operations, but it must remain inspectable and bounded. Test malicious issue text requesting secrets, a patch that weakens checks, an agent that labels itself accepted, stale grants, revoked devices, a task copied into another workspace, and replayed approvals for a different artifact. The safe result is a refused effect with useful evidence, not a model judgment about whether the request seems trustworthy.

### DATA-08 — Deletion and withdrawal need a joined lifecycle across raw and derived stores

**Priority: high before a learning pilot.** The [sales privacy implementation](../../../crates/coder/src/task/sales/privacy.rs) tracks bounded copies, recipients, digests, expiry, and file identity. It records unavailable copies honestly and keeps minimal suppression information so deleting a lead cannot restore contact permission. The sales contract also states that separate owner-controlled evidence and exports need their own cleanup. Closed [#10867](https://github.com/OpenAgentsInc/openagents/issues/10867) qualified these paths with synthetic contacts; it did not certify every possible external copy.

The learning paths create additional stores: run folders, patch blobs, briefing blobs, admitted traces, finder feedback, token/interface indexes, embeddings, corpus files, recipes, teacher targets, trial outputs, checkpoints, registry bindings, public reports, and backups. The source reviewed here does not establish one deletion or revocation traversal across them. A corpus tombstone cannot remove the raw trace source; deleting a web trace cannot be assumed to invalidate a model trained from an exported copy.

**Acceptance:** maintain a derivation graph from consented source to every persisted artifact and active model. On withdrawal, stop new training and disclosure immediately, exclude the source from all future corpus/index builds, and apply the agreed disposition to existing copies and models. Record deletion completed, blocked by retention, unavailable, or pending external confirmation separately. State whether an already trained model is retired, retrained, or allowed to persist under the agreement; do not promise automatic unlearning unless it is implemented and verified. Exercise expiry, withdrawal during training, crash/restart, restore from backup, export/reimport, and a stale worker trying to publish after revocation.

### DATA-09 — Portability and sharing are useful foundations, but neither is training consent

**Priority: medium.** Closed [#11134](https://github.com/OpenAgentsInc/openagents/issues/11134) added whole-account export through the web and `coder export --account`. Closed [#11109](https://github.com/OpenAgentsInc/openagents/issues/11109) added private trace upload, sharing, and deletion; [#11178](https://github.com/OpenAgentsInc/openagents/issues/11178) extended traces to orchestration trees. The [trace routes](../../../crates/openagents-web/src/traces.rs) describe sharing or deleting a trace as applying to its agents too.

These are valuable for customer inspection and exit. They do not prove that a downloaded record includes every training copy or that publishing a trace licenses model training. A shared trace can contain personal information that is not a recognized credential. The [secret-screen API](../../../crates/secret-screen/src/lib.rs) deliberately distinguishes credential rules from email/home-path redaction; passing a credential scan is not a privacy review.

**Acceptance:** include customer-readable data-use and derivation records in the codebase product's export. Let the customer see which source items were used by which corpus/model and under which agreement. Preview the whole orchestration scope before sharing, including descendants, and define what happens if agents are added after sharing. Preserve private defaults. A shared URL grants no live host control, repo write, payment, or training right. The export format should remain readable without a running service, while excluding credentials and other customers' data.

### DATA-10 — Security qualification must follow the actual customer path

**Priority: high before private repositories or self-service.** The previous health audit found weaknesses across payments, identity, gateway, web, desktop, and mobile. Some changed before this snapshot. For example, [gateway receipt logging](../../../crates/gateway/src/receipt_log.rs) now uses the blocking pool and records write failures, and [#11186](https://github.com/OpenAgentsInc/openagents/issues/11186) fixed the shared-tenant provider-key and related account-state issue. These fixes must not be presented as still-open defects.

Other boundaries remain relevant. [GitHub repository tokens](../../../crates/oa-auth/src/repos.rs), `seal_bound` and `open_bound`, use an account-bound AES-GCM envelope under the configured application key with a `v1` encoding; the reviewed envelope has no key identifier or multi-key rotation mechanism. This differs from the keyring-based BYOK store. Cloud repo-scoped credential delivery still has open work in [#11226](https://github.com/OpenAgentsInc/openagents/issues/11226). Tenant-pooled quota/concurrency remains in [#11190](https://github.com/OpenAgentsInc/openagents/issues/11190).

**Acceptance:** qualify the exact service configuration with separate synthetic customers, private scratch repositories, least-scope credentials, key rotation/revocation, interrupted workers, and expired installations. Inspect logs, artifacts, exports, traces, and error responses for seeded credentials. Verify that account access, repository access, host access, and training authority remain independently revocable. Run the affected consumers' focused tests; do not substitute a workspace-wide test count, a live owner-host smoke, or a UI screenshot for these boundaries.

## A minimum customer learning record

A customer learning pilot needs a small enforceable record before it needs a broad settings dashboard. At minimum, retain:

- The responsible customer and workspace, stable repository identity, permitted revisions or source scope, and exact agreement version.
- Separate purposes for execution, provider disclosure, tenant training, shared training, evaluation, and publication, with explicit exclusions.
- Current authorized recipients, provider/model constraints, region or execution restrictions where agreed, and budget owner.
- Raw-data retention, derived-artifact retention, withdrawal handling, and the disposition of already served models.
- Source-to-run-to-corpus-to-recipe-to-candidate identities, protected evaluation allocation, and each authority that approved a transition.
- Revocation state and the result of each cleanup action, including copies that cannot be verified as removed.

Do not put customer content, secrets, raw contact records, or private agreement text in this public audit. A private evidence store can retain those under the customer's terms. Public records should carry reviewed digests and aggregate results only when that projection is separately authorized.

## Qualification scenarios for the first customer

Use isolated fixtures before involving a real customer. The following scenarios test joins that individual crate tests can miss:

| Scenario | Required result |
| --- | --- |
| Same repository basename in two workspaces | Separate caches, feedback, corpora, cost records, and credentials |
| Valid execution grant, no training grant | Task can run; corpus admission refuses |
| Tenant-only training, shared-model request | Shared admission refuses even if the source is public or the model predicts benefit |
| General chat account with a narrower pilot contract | The pilot restriction survives sync, export, trace upload, and corpus creation |
| Secret in patch, nested teacher JSON, or tool output | No unauthorized raw retention or disclosure; rejection remains content-free |
| Withdrawal during a long training run | No new admission or publication; artifacts receive the agreed disposition |
| Missing provider cost or failed replay | Financial/evidence uncertainty remains visible; no false accepted-outcome claim |
| Deleted corpus restored from an old backup | Current revocation prevents re-admission and triggers cleanup |
| Revoked repo or host grant with a queued task | No new forbidden effect; prior work remains explainable without granting new authority |
| Public trace with private child material | Sharing obeys the exact reviewed scope and does not silently expand |

Passing these fixtures is implementation evidence. A real pilot still needs the exact customer agreement, selected providers, supported installation, accepted delivery, and operator cleanup process qualified under that agreement. A later failed real-world qualification should become a focused defect with its evidence, not an excuse to label every previously closed implementation issue unfinished.
