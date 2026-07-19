---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.desktop.trust.complete.workbench"
assurance_revision: 2
title: "Desktop Trust-Complete Workbench AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposed AssuranceSpec creates exact AC-1..AC-52 criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete. The ProductSpec is validator-managed with a structured `productspec-acceptance-criteria` block. The current deterministic AssuranceSpec proposer still extracts only top-level Markdown criterion bullets, so this proposal was mechanically bridged from the ProductSpec parser's validated structured item list and then rebound to the SHA-256 of the unchanged original ProductSpec bytes. No criterion text, ID, revision, or subject digest was inferred or normalized silently.

## Subject

The proposal is bound to the exact original ProductSpec bytes, revision, path, and stable structured criterion identifiers below. Until the proposal/session tooling consumes structured ProductSpec items directly, reviewers must treat the bridge limitation as a proof-tooling gap: structural validation and exact digest checking are available, but a failed executable-profile session probe is not permission to rewrite the ProductSpec or ignore the binding.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-9",
      "AC-10",
      "AC-11",
      "AC-12",
      "AC-13",
      "AC-14",
      "AC-15",
      "AC-16",
      "AC-17",
      "AC-18",
      "AC-19",
      "AC-20",
      "AC-21",
      "AC-22",
      "AC-23",
      "AC-24",
      "AC-25",
      "AC-26",
      "AC-27",
      "AC-28",
      "AC-29",
      "AC-30",
      "AC-31",
      "AC-32",
      "AC-33",
      "AC-34",
      "AC-35",
      "AC-36",
      "AC-37",
      "AC-38",
      "AC-39",
      "AC-40",
      "AC-41",
      "AC-42",
      "AC-43",
      "AC-44",
      "AC-45",
      "AC-46",
      "AC-47",
      "AC-48",
      "AC-49",
      "AC-50",
      "AC-51",
      "AC-52"
    ],
    "document_digest": "sha256:019c4b3b55c69f94764bb44c65c6f5bbe135b2dfb71e40145907fbea5ddbe9d0",
    "path": "specs/desktop/desktop-trust-complete-workbench.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 7
  }
}
```

## Risk Model

No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.

```assurancespec-risks
{
  "risks": [],
  "source_digest": "sha256:cfb6dd5f82ca67176de4e80558e28dd72484c064dd69e781dc1733c5e409059b",
  "source_snapshot": "- Trust surfaces could read as friction if receipts and manifests are not\n  quiet by default; the design must keep them one gesture away, not in the\n  path of every action.\n- Fail-closed containment on platforms with weak sandbox primitives may block\n  workflows competitors allow; the owner-local danger mode must remain an\n  explicit, visible escape hatch without becoming a default.\n- Full Auto is only trustworthy after the bug-bash defect classes (queue\n  replay, composer leaks, restart ambiguity, active-thread eviction) are\n  closed by contract; shipping it earlier converts every defect into an\n  unattended failure discovered a day later.\n- Cross-provider handoff is currently plumbing-present, experience-unverified;\n  claiming it before the end-to-end acceptance tests exist would be exactly\n  the false-green failure the product exists to eliminate.\n- Multi-account failover must stay within provider terms; own-capacity-only\n  and no-resale-of-subscription-inference are standing constraints.\n- The six-target matrix and owned-runner release chain are operationally\n  expensive; sequencing must not let breadth starve depth.\n- Rendering the full agent tree live at fleet scale has real performance\n  risk; the perf-baseline gates exist to keep it honest.\n- A shared experience bank can leak private run content or create cross-tenant\n  influence if scope and retention are implicit; eligible snapshots must be\n  consented, tenant/workspace filtered, content-addressed, and deletion-aware.\n- Adaptation can make evaluation meaningless if a run learns from its own\n  outcome or changes policy between turns; the frozen pre-run snapshot and\n  immutable effective bundle are release-blocking invariants.\n- Optimization creates a false-green shortcut if candidates can self-promote;\n  held-out evidence and an independent Blueprint release gate are mandatory.\n- Monaco, Pierre, language servers, terminals, and agent adapters can create\n  accidental parallel state owners unless every result is fenced by the one\n  Effect project/document generation graph and every package remains behind an\n  owned adapter.\n- A premature “Rust backend” would duplicate the project and persistence\n  planes, weaken Effect Schema authority, and make cross-surface behavior\n  harder to prove. Native helpers therefore require empirical necessity,\n  authority-free contracts, failure semantics, and reversal tests.\n- Vim packages can bypass focus, command, dirty/conflict, accessibility, and\n  teardown laws if treated as editor glue. The app-owned controller, canonical\n  command translation, precedence table, and Vim-on/Vim-off corpus are the\n  admission boundary.\n- A fixed dark palette can masquerade as accessibility completion. Tokyo Night\n  is the explicit initial product choice; checked semantic contrast and\n  non-color cues are immediate gates, while light, high-contrast, and system\n  modes remain visible IDE-18/full-parity gaps until their own evidence passes."
}
```

## Assurance Scope

Every structured ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable. All 52 generated obligations remain incomplete and `needs_design`. No repository candidate, environment, or proof technique is selected.

## Environments

Repository facts are proposal context only. No Environment Profile, adapter, capability, or permission is selected by inventory.

```assurancespec-environments
{
  "profiles": [],
  "repository_inventory": {
    "candidate_artifact_refs": [],
    "declared_scripts": [],
    "diagnostics": [
      "repository_not_supplied"
    ],
    "inventory_digest": "sha256:13cef510a746daf9c1d6b2766fef971b7f66c7392a70709fd61ccd271f1b02e4",
    "repository_label": "not-supplied",
    "state": "absent",
    "tracked_file_count": 0,
    "truncated": false
  }
}
```

## Obligations

Each criterion receives one incomplete proposed obligation. Missing proof-design fields project as needs_design and prevent admission or execution.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-1"
    ],
    "disposition": "required",
    "id": "AO-AC-1-01",
    "source_claim_digest": "sha256:8184dd1e9234f62ab989dd4e6d1af8873295e3df1a5fa57764bfc4821ce5cca8",
    "source_claim_snapshot": "When a user sends a follow-up while a turn is running, the composer requires an explicit queue-or-steer choice, and the transcript later shows the input's admission, promotion, execution, and terminal states as distinct facts.",
    "title": "Assure AC-1"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-2"
    ],
    "disposition": "required",
    "id": "AO-AC-2-01",
    "source_claim_digest": "sha256:9652fe487c61f3af3623963f8158119f2851eb4a91f4e1a26921615f654a78ec",
    "source_claim_snapshot": "When a user opens any run's detail view, they see both the authority manifest (what policy admitted) and the execution receipt (what containment actually enforced); requested and effective enforcement are never merged into one indicator.",
    "title": "Assure AC-2"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-3"
    ],
    "disposition": "required",
    "id": "AO-AC-3-01",
    "source_claim_digest": "sha256:a64c596ddfa19dad0174c8752ab41683b2597b0f8e7b1ce375cbc0b17308dbd9",
    "source_claim_snapshot": "When a run is started under a profile whose OS enforcement cannot be represented on the current platform, the run refuses to start with a typed reason instead of degrading silently.",
    "title": "Assure AC-3"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-4"
    ],
    "disposition": "required",
    "id": "AO-AC-4-01",
    "source_claim_digest": "sha256:7aaff45c5fcf5e62e98daa9b3a6600d0c90ee68310f98416d6779e03d0d39ef4",
    "source_claim_snapshot": "When a session spawns child agents, the agent tree shows every retained child with live lifecycle state and latest activity, an in-flight spawn is visible before it resolves, any child's full transcript opens within two interactions, and unlinked history renders as an explicit gap node.",
    "title": "Assure AC-4"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-5"
    ],
    "disposition": "required",
    "id": "AO-AC-5-01",
    "source_claim_digest": "sha256:9fe343f32445315da26cd4a873ea9c066e14102a950da4a253188d85cf0f686e",
    "source_claim_snapshot": "When a user rewinds to a prior turn, the app stages the restore, discloses reversible versus irreversible effects and externally modified files, and only applies on explicit commit, emitting a rewind receipt.",
    "title": "Assure AC-5"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-6"
    ],
    "disposition": "required",
    "id": "AO-AC-6-01",
    "source_claim_digest": "sha256:f839ae9cda1bb5669273a5a1f54429d9def4b989f01a5ca31fc3cd471021e27b",
    "source_claim_snapshot": "When agent work in a worktree finishes, unchanged worktrees are auto-removed, changed worktrees are retained, dirty or unpushed worktrees are refused for cleanup, and each outcome emits a cleanup receipt; the Work Unit's delivery state is visible and distinct from turn completion.",
    "title": "Assure AC-6"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-7"
    ],
    "disposition": "required",
    "id": "AO-AC-7-01",
    "source_claim_digest": "sha256:f49ca488e675055a3e24388fd814ca3718a83b4f955d8b24c99ca3a67b7218d2",
    "source_claim_snapshot": "When the packaged app is inspected by release tests, Electron fuse oracles pass (RunAsNode, NODE_OPTIONS, and inspect disabled; ASAR integrity on), and every IPC channel rejects messages that fail schema decoding or sender validation.",
    "title": "Assure AC-7"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-8"
    ],
    "disposition": "required",
    "id": "AO-AC-8-01",
    "source_claim_digest": "sha256:2829acc76437a086d30f1a7d51e71cf1bbb40abfc52d87c03093b01cf0b1233f",
    "source_claim_snapshot": "When a transcript holds ten thousand or more items, scrolling and turn navigation stay within the checked-in p95 frame-time baselines, and those baselines gate release.",
    "title": "Assure AC-8"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-9"
    ],
    "disposition": "required",
    "id": "AO-AC-9-01",
    "source_claim_digest": "sha256:191244af2d34c083d340c1396038e7eb3d871ed2cba98b90530ae9781134c555",
    "source_claim_snapshot": "When an update is offered, the client verifies the signed release-set manifest against the pinned key before install, refuses version downgrades, retains a rollback slot proven by test, and drains live engine work before relaunch.",
    "title": "Assure AC-9"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-10"
    ],
    "disposition": "required",
    "id": "AO-AC-10-01",
    "source_claim_digest": "sha256:1611c788733d77f7cf8ff2c0be365ba0de8a4156cd9fe10592de47882a60e8d4",
    "source_claim_snapshot": "When a run executes under the hermetic profile, the emitted manifest lists every admitted input source, and no ambient instruction, hook, plugin, or memory outside that manifest influenced the run.",
    "title": "Assure AC-10"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-11"
    ],
    "disposition": "required",
    "id": "AO-AC-11-01",
    "source_claim_digest": "sha256:e6910005e63ad74af50fdf50fc3737a664448c5f2886466f1fdbb289ebdacf33",
    "source_claim_snapshot": "When the same command ID is invoked from the palette, a keyboard shortcut, a menu, and a model-proposed action, all four paths produce identical typed outcome records.",
    "title": "Assure AC-11"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-12"
    ],
    "disposition": "required",
    "id": "AO-AC-12-01",
    "source_claim_digest": "sha256:bd964b5a6249da236393b6d541460207dfd47d02fe55b8b4261df3a03512687b",
    "source_claim_snapshot": "When a user binds a chat or command to a hotkey, that binding never reshuffles as chats are created, reordered, or closed; the bound target stays stable until the user rebinds it.",
    "title": "Assure AC-12"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-13"
    ],
    "disposition": "required",
    "id": "AO-AC-13-01",
    "source_claim_digest": "sha256:ce7889f074554ec858140dec41ae51add47877326c4fe0389372dd66d9245b50",
    "source_claim_snapshot": "When a user switches between chats mid-composition, draft text, queued inputs, attachments, and stop/steer controls each remain keyed to their own thread; no composer or queue state leaks across chats, and a dispatched queue item never replays.",
    "title": "Assure AC-13"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-14"
    ],
    "disposition": "required",
    "id": "AO-AC-14-01",
    "source_claim_digest": "sha256:d199e2cb5f55af5362f62ab454c51794ab3452f8ae38f4d8bb97d2ca9449884c",
    "source_claim_snapshot": "When any assistant message renders, its metadata shows the effective model, provider, and account from observed execution events; a turn whose effective model differs from the requested one is displayed as such and is never streamed under the requested model's name.",
    "title": "Assure AC-14"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-15"
    ],
    "disposition": "required",
    "id": "AO-AC-15-01",
    "source_claim_digest": "sha256:69df3146b19809739884fbec9dee7081f7b7ed7a6513681f9ef3521f81ab639c",
    "source_claim_snapshot": "When the Fleet workspace shows a readiness or status light, that light derives from a decoded fresh receipt; absent or stale evidence renders as unknown, never as green.",
    "title": "Assure AC-15"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-16"
    ],
    "disposition": "required",
    "id": "AO-AC-16-01",
    "source_claim_digest": "sha256:8627ea158153bec51902b3146b9b012897faf84f93095c8b2f6d4d42734e2c66",
    "source_claim_snapshot": "When an agent turn produces reasoning, the reasoning is visible in the main trace expanded by default, tool activity is described by what actually ran, and current usage is visible inline without leaving the main view.",
    "title": "Assure AC-16"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-17"
    ],
    "disposition": "required",
    "id": "AO-AC-17-01",
    "source_claim_digest": "sha256:2157f3e313c89ab977288345950c692cde4a0ed5cd5137c865de232d74ef49fa",
    "source_claim_snapshot": "When a user authors a ProductSpec in the workroom and accepts its plan, the resulting work packets each retain exact spec revision, criterion ID, and terminal evidence links, and no packet is displayed as completed without its matching terminal outcome and review post-image.",
    "title": "Assure AC-17"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-18"
    ],
    "disposition": "required",
    "id": "AO-AC-18-01",
    "source_claim_digest": "sha256:2c6b39cebae4a4025177e85b43600d5b5218fc72f509286bc38a0684bde055da",
    "source_claim_snapshot": "When the app restarts with work in flight, each affected session either resumes through typed recovery or terminates with an owner-visible typed reason; restart attribution is never invented, and the app discloses the running build, version, and source on demand.",
    "title": "Assure AC-18"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-19"
    ],
    "disposition": "required",
    "id": "AO-AC-19-01",
    "source_claim_digest": "sha256:c9fe54312307d7468cdf6a42878b6ccfb6fdfa78e0e6d8600dba0d4155e43c1b",
    "source_claim_snapshot": "When a selected account is exhausted or rate-limited, the surfaced error names the real provider condition, and where another connected account is ready, work fails over to it under the recorded execution profile with the rotation visible in the receipt.",
    "title": "Assure AC-19"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-20"
    ],
    "disposition": "required",
    "id": "AO-AC-20-01",
    "source_claim_digest": "sha256:d36562b7efe019f7f15420790b7ee4b6c7a4eb4a889485f175b0f5bb190ef391",
    "source_claim_snapshot": "When a user launches Full Auto from its dedicated action, setup asks once for objective, workspace, and provider/account routing policy, then presents a read-only run view with explicit Play, Pause, and Stop and no ordinary composer; pause, resume, and stop are durable typed transitions that survive an app restart.",
    "title": "Assure AC-20"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-21"
    ],
    "disposition": "required",
    "id": "AO-AC-21-01",
    "source_claim_digest": "sha256:c0fcd9c5d2400324d80b874e9801e85ddcaa5cedf5ffb99b38714234426c53dc",
    "source_claim_snapshot": "When a model tier, account, or provider hits a usage limit or fails during a Full Auto run, the run continues automatically on the next admitted model, account, or provider lane per the routing policy with the rotation recorded in the run receipt; the run never halts to await human acknowledgment of a limit, and it terminates only by owner stop, cap, objective completion, or typed policy block.",
    "title": "Assure AC-21"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-22"
    ],
    "disposition": "required",
    "id": "AO-AC-22-01",
    "source_claim_digest": "sha256:2319ec2f6f91e090da8cb10fac090be04113875f3c4697590d58eba20fbc1c32",
    "source_claim_snapshot": "When a thread's provider lane is switched mid-conversation, the next provider receives bounded host-owned history and continues in the same visible thread, and an end-to-end acceptance test proves a recognizable fact written by provider A is used by provider B in that same thread, with named handoff test runs visible in the sidebar.",
    "title": "Assure AC-22"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-23"
    ],
    "disposition": "required",
    "id": "AO-AC-23-01",
    "source_claim_digest": "sha256:bb063cc47e696c2bb30f3359e9164d325e28c677792a863e974729987be77d88",
    "source_claim_snapshot": "When host caches face pressure, a thread bound to an active Full Auto run is never evicted or rendered unopenable; a continuation that addresses an unopenable thread surfaces as a typed defect with an owner-visible reason, never a generic conversation-not-found error.",
    "title": "Assure AC-23"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-24"
    ],
    "disposition": "required",
    "id": "AO-AC-24-01",
    "source_claim_digest": "sha256:972d3be6c3c6789fef98f669d91d630806effaaf28e9d5c1ec63c28a3dd380f2",
    "source_claim_snapshot": "When a Full Auto run ends for any reason, it produces a bounded run report covering objective, turns, dispositions, provider/account rotations, failures, and evidence links, and any failed run can be reproduced as a replayable fixture run.",
    "title": "Assure AC-24"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-25"
    ],
    "disposition": "required",
    "id": "AO-AC-25-01",
    "source_claim_digest": "sha256:5e7e520a49e7e093e50da1246354f88123e69757adf427f594c1accb3459c43e",
    "source_claim_snapshot": "When the Cursor parity corpus runs, every required row in `specs/openagents/cursor-capability-parity.product-spec.md` maps to a Desktop command, route, adapter, or explicit cross-surface continuation plus current acceptance evidence; no supported workflow silently disappears between the classic workbench and agent-first window.",
    "title": "Assure AC-25"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-26"
    ],
    "disposition": "required",
    "id": "AO-AC-26-01",
    "source_claim_digest": "sha256:220458080b532942a2283771c0f3bbc56da05cce22e282dda828a0b723e97f31",
    "source_claim_snapshot": "When a user performs Cursor-class editing work, completion, next-edit prediction, inline generation, multi-file apply, review, accept/reject/undo, semantic context, diagnostics, Git, terminal, preview, settings, keymaps, and first-party theme switching operate in one project without requiring another editor.",
    "title": "Assure AC-26"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-27"
    ],
    "disposition": "required",
    "id": "AO-AC-27-01",
    "source_claim_digest": "sha256:22aa9fcadd406a5c894ab4b9f18ee7eb08e69aa842e57befc815a6f0cb36c7b7",
    "source_claim_snapshot": "When a user launches browser automation or computer use, the admission view states the exact browser partition, OS/network scope, secrets policy, and approvals; each action is receipted, and unavailable enforcement fails closed without removing the supported workflow.",
    "title": "Assure AC-27"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-28"
    ],
    "disposition": "required",
    "id": "AO-AC-28-01",
    "source_claim_digest": "sha256:0aa04f0c3e168d86fa4e5adff82859f214f331c4ed6f00c96888e4166528f995",
    "source_claim_snapshot": "When a user configures a session, they can select compatible harness, model/provider/account, execution placement, sync posture, and indexing backend independently; the resulting session keeps one identity and reports selected and effective values plus all data flows.",
    "title": "Assure AC-28"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-29"
    ],
    "disposition": "required",
    "id": "AO-AC-29-01",
    "source_claim_digest": "sha256:3cd7d0dbc1a0814671d9deabd2915bd17271e2d2678ff07e57acb7fe4e8631d0",
    "source_claim_snapshot": "When a user inspects storage for a repository, Desktop enumerates every local and remote data class, size, freshness, retention, and sync state and proves complete export, chat-preserving index reset, repository-knowledge deletion, account/device revocation, and full deletion with remote tombstone receipts where applicable.",
    "title": "Assure AC-29"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-30"
    ],
    "disposition": "required",
    "id": "AO-AC-30-01",
    "source_claim_digest": "sha256:387d8399d6c42dc876a2f6c4a2f07b33125002445ca1b246c0635a0627b21737",
    "source_claim_snapshot": "When extensions, plugins, skills, MCP servers, rules, hooks, or subagents are discovered, installed, imported, updated, disabled, or removed, Desktop shows provenance, permissions, compatibility, isolation, and rollback; no untrusted code executes in the shell or trusted engine process.",
    "title": "Assure AC-30"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-31"
    ],
    "disposition": "required",
    "id": "AO-AC-31-01",
    "source_claim_digest": "sha256:a5cf500acc066e72271fce31f97b0c0d43289c94bf3fd737abc30e431412b887",
    "source_claim_snapshot": "When background agents or automations are launched from schedule, repository, issue/PR, webhook, or manual triggers, they may use an admitted local, owner-managed, or OpenAgents-managed placement, survive client closure, accept typed intervention, enforce caps and idempotency, and return reviewable outcomes and receipts.",
    "title": "Assure AC-31"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-32"
    ],
    "disposition": "required",
    "id": "AO-AC-32-01",
    "source_claim_digest": "sha256:ca103698e3ccee26eca68b781b28c79faba215a96e04e7f5b0688a4f1610bd30",
    "source_claim_snapshot": "When a clean Cursor profile is migrated, supported settings, keybindings, rules, skills, and MCP configuration import through an allowlist, credentials and proprietary state do not, and every item receives an imported, skipped, or rejected reason.",
    "title": "Assure AC-32"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-33"
    ],
    "disposition": "required",
    "id": "AO-AC-33-01",
    "source_claim_digest": "sha256:1daa9acc879e0d91de99bd70992bde3b32393b6042bd1d548b940e63456b2f4c",
    "source_claim_snapshot": "When a user inspects or launches any run, Desktop identifies the selected base HarnessPolicyBundle and the observed effective bundle by immutable digest, displays each of the six dimension-policy refs and compatibility result, and never collapses requested and effective harness identity into one label.",
    "title": "Assure AC-33"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-34"
    ],
    "disposition": "required",
    "id": "AO-AC-34-01",
    "source_claim_digest": "sha256:e8ab72dc62162d36e9d794ad0e697e748866fd9ea359d1ef021e19204f6c03ae",
    "source_claim_snapshot": "When run-start adaptation is enabled, Desktop shows the frozen experience-bank snapshot, scope filters, adaptation state, bounded released patches, and HarnessAdaptationReceipt one gesture away; the effective bundle is fixed before the first turn and remains byte-identical through continuations, restart, pause/resume, and provider handoff unless the run fails closed as incompatible.",
    "title": "Assure AC-34"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-35"
    ],
    "disposition": "required",
    "id": "AO-AC-35-01",
    "source_claim_digest": "sha256:9fb7da5bb11d9117975ecd82f92ff334d66bc3a29e171caadfe02093f264fff3",
    "source_claim_snapshot": "When a user inspects the experience bank, Desktop separately inventories execution experiences and released patterns with source-run provenance, visibility, retention, retrieval/training eligibility, size, export, deletion, and tombstone status; deleting a source makes it ineligible for future snapshots and preserves only the minimum non-content tombstone needed to prevent resurrection.",
    "title": "Assure AC-35"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-36"
    ],
    "disposition": "required",
    "id": "AO-AC-36-01",
    "source_claim_digest": "sha256:db10ca5f0cb31d6b099a07627d26c9819b5a8295c14a3624ecfc10bde448c802",
    "source_claim_snapshot": "When offline optimization produces a harness candidate, the UI distinguishes candidate, shadow/dogfood, released, active, rejected, and rolled-back states, presents held-out evaluation and compatibility evidence, and exposes no path by which the producing optimizer or run can self-verify or self-promote into production.",
    "title": "Assure AC-36"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-37"
    ],
    "disposition": "required",
    "id": "AO-AC-37-01",
    "source_claim_digest": "sha256:ee8b24a7f1e063112feb2de23515c80aa87f51b95bbed125f059561a0e2ff8e3",
    "source_claim_snapshot": "When an adapted harness executes, its authority manifest is identical to the base run's authority manifest for workspace, placement, provider/account candidates, tools, approvals, guardrails, budgets, done condition, and external effects; any proposed delta outside the admitted harness-module schema refuses before dispatch with a typed reason.",
    "title": "Assure AC-37"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-38"
    ],
    "disposition": "required",
    "id": "AO-AC-38-01",
    "source_claim_digest": "sha256:88360daac600812feb9f3ed958c55c262c4450b87bb53d1eb2653da1fca194da",
    "source_claim_snapshot": "When Desktop projects MemoHarness state to mobile, web, exports, or public receipts, the projection is explicit-field allowlisted and may include safe digests, release state, compatibility, adaptation status, and redacted receipt refs, but never raw experiences, prompts, transcript text, tool output, embeddings, retrieval queries, private scores, secrets, or filesystem paths.",
    "title": "Assure AC-38"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-39"
    ],
    "disposition": "required",
    "id": "AO-AC-39-01",
    "source_claim_digest": "sha256:12a26a6b7b039cc00793757f7c767fcf66b6c580a2a9293223fbcbbdf83a9a20",
    "source_claim_snapshot": "When a supported source file is opened from Finder, Explorer, quick open, search, Problems, symbols, Git, recent restore, or an agent backlink, every route resolves the same current project/file/document identity and makes a real editable Monaco document primary in the main workspace before chat, provider, index, Git, LSP, or remote hydration completes; failure renders a typed editor state there rather than a side-pane fallback or no-op.",
    "title": "Assure AC-39"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-40"
    ],
    "disposition": "required",
    "id": "AO-AC-40-01",
    "source_claim_digest": "sha256:6d3a6efc26c817257942e83aa73157c1872336d85df2b36763524d05b2d8bfbb",
    "source_claim_snapshot": "When two projects or worktrees contain the same relative path, their Monaco models, dirty recovery, diagnostics, search/symbol results, Git evidence, terminals, tasks, breakpoints, agent context, proposals, and navigation history remain separated by exact project/root/attachment/document/service generations across rapid switching and restart.",
    "title": "Assure AC-40"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-41"
    ],
    "disposition": "required",
    "id": "AO-AC-41-01",
    "source_claim_digest": "sha256:a59a69acddf5698abfdf10d996524675a59e846e87c59e6bdaf0c0830c704227",
    "source_claim_snapshot": "When a document is edited, viewed in splits, renamed, externally changed, deleted, revoked, saved, closed dirty, or recovered after forced termination, one Effect-owned document state reports encoding, EOL, disk revision, generation, dirty/conflict/recovery status and prevents silent overwrite; Monaco mechanics never become the only unsaved copy or filesystem authority.",
    "title": "Assure AC-41"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-42"
    ],
    "disposition": "required",
    "id": "AO-AC-42-01",
    "source_claim_digest": "sha256:a46885acd1f4b4a5a07d4eccc42f5d01be517f5564f6a38ae2b51a5a496009b5",
    "source_claim_snapshot": "When Monaco-local, tsserver, LSP, task, test, or DAP capabilities start, stop, crash, reconnect, move, or return late results, the UI and agent tools show the exact available/degraded/unavailable evidence tier and effective placement, reject stale generations, honor cancellation and bounds, and never present a missing provider as a working feature.",
    "title": "Assure AC-42"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-43"
    ],
    "disposition": "required",
    "id": "AO-AC-43-01",
    "source_claim_digest": "sha256:0ef945ea0a355b04f70baa356783729eaa00b728cccc2d10cadb3b54e1c946f3",
    "source_claim_snapshot": "When an agent receives code context and proposes a single- or multi-file edit, the user can inspect the exact disclosure manifest and effective runtime, review version-bound changes in Pierre, accept or reject at supported granularity, apply or undo through canonical authority, and inspect post-apply diagnostics, tests, Git, and delivery facts; a changed base refuses or enters an explicit rebase flow rather than guessing line positions.",
    "title": "Assure AC-43"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-44"
    ],
    "disposition": "required",
    "id": "AO-AC-44-01",
    "source_claim_digest": "sha256:1a8371f8221dc07e7d0184f4ac446eb521733f94433b6a6d5cdcb1ef50cc8d82",
    "source_claim_snapshot": "When a terminal or task starts, Effect owns its project/worktree/session identity, environment and cwd admission, retention, commands, recovery policy, and receipts; xterm owns screen mechanics; a process-opaque Rust helper owns only PTY/process-group/resize/signal/byte mechanics; helper absence, incompatibility, crash, or stale generation produces a typed fail-closed or degraded result without leaking authority or replaying a mutating command.",
    "title": "Assure AC-44"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-45"
    ],
    "disposition": "required",
    "id": "AO-AC-45-01",
    "source_claim_digest": "sha256:63e28331a9d5ecf0bd5f2d46cb6738127bfbd28ccbc7f221d53a7f840e1eabb4",
    "source_claim_snapshot": "When the same project capability is fulfilled locally, on an owner-managed host, or on an admitted managed target, the command and result shapes stay identical while effective placement, version, latency, custody, freshness, and attachment generation remain visible; connectivity loss, revocation, or incompatibility never causes silent project upload, helper installation, placement change, or managed fallback.",
    "title": "Assure AC-45"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-46"
    ],
    "disposition": "required",
    "id": "AO-AC-46-01",
    "source_claim_digest": "sha256:f9b4baaf0645fc0dce4baebbcb22d8f1fafd904f4aae633cb9c56632d27af3dd",
    "source_claim_snapshot": "When Desktop hands a file, range, diagnostic, proposal, test, artifact, or run to mobile, web supervision, or a public share compiler, the projection uses opaque generation-bound safe refs and an explicit allowlist; Desktop reauthorizes every continuation, and no raw root, environment, credential, private context, terminal, embedding, or unselected repository content crosses the boundary.",
    "title": "Assure AC-46"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-47"
    ],
    "disposition": "required",
    "id": "AO-AC-47-01",
    "source_claim_digest": "sha256:cc7996abd82c5e0689b849715f5be4e4a108565a4fc387270c78e2889ca916b4",
    "source_claim_snapshot": "When an IDE implementation proposes Rust beyond PTY or required containment, its admission evidence names the missed cross-platform Effect/Node budget, optimized baseline, smallest authority-free protocol, failure behavior, generated conformance fixtures, and reversal threshold; without that evidence the capability remains in Effect/TypeScript or a supervised external process.",
    "title": "Assure AC-47"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-48"
    ],
    "disposition": "required",
    "id": "AO-AC-48-01",
    "source_claim_digest": "sha256:6ea529936e6802924d6245ee28ccdd009c6dd9bbf0c922586c8b4d9c49c9c6df",
    "source_claim_snapshot": "When a user enables Vim from Settings, the command palette, or the editor status control, every Editor-mode Monaco view uses the same persistent off-by-default first-party setting; supported modes, motions, counts, operators, text objects, registers, repeat, search, supported colon commands, clipboard, undo/redo, and status are visible and operate through canonical commands, and restart, split, equal-relative-path worktree, conflict, IME, accessibility, disable, and listener-teardown journeys neither bypass document authority nor lose state.",
    "title": "Assure AC-48"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-49"
    ],
    "disposition": "required",
    "id": "AO-AC-49-01",
    "source_claim_digest": "sha256:98cb2915cae30f8bf32b3d684688083ec687b8f8f12a8e05706a14e7e78593e0",
    "source_claim_snapshot": "When any initially supported Desktop IDE surface mounts, one provenance-pinned Tokyo Night semantic projection controls app chrome, Monaco syntax and chrome, Pierre tree and diff, xterm, Problems, Output, debug, proposal/review, browser, and status colors before first paint; checked contrast and non-color cues pass, no executable or raw unreviewed theme code runs, theme initialization does not recreate canonical models or sessions, and no broader theme claim is made until the deferred light/high-contrast/system corpus passes.",
    "title": "Assure AC-49"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-50"
    ],
    "disposition": "required",
    "id": "AO-AC-50-01",
    "source_claim_digest": "sha256:6a9bcb19ff87fb8c3e19cdc25ff950067cb48fc35a92372f6bd42f4c3eb10a11",
    "source_claim_snapshot": "When an IDE contract crosses persistence, IPC, helper, renderer, mobile, web, or public-share boundaries, a contract audit finds one identified Effect Schema source using Struct, TaggedStruct, or TaggedUnion, derives its TypeScript type from that source, constrains scalar refs, and rejects untrusted input through the schema; the audit finds no raw interface or handwritten union acting as a parallel boundary contract.",
    "title": "Assure AC-50"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-51"
    ],
    "disposition": "required",
    "id": "AO-AC-51-01",
    "source_claim_digest": "sha256:8745c581536258d6beea5d4dffae775cc8ad74cbbb9be4fb0e354d5e190afaac",
    "source_claim_snapshot": "When a project, document, language, Git, terminal, task, debug, agent, projection, or storage capability starts and stops, its application service is a Context.Service implementation composed with Layer.effect; named Effect.fn operations expose Schema.TaggedErrorClass failures; watchers, processes, subscriptions, and streams are scoped to and interrupted with the owning project generation; and no renderer package or native helper becomes lifecycle authority.",
    "title": "Assure AC-51"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "AC-52"
    ],
    "disposition": "required",
    "id": "AO-AC-52-01",
    "source_claim_digest": "sha256:0abfb135a36a1fdacff080df790151accaa25e7682ed0cfdf5c291f37c728354",
    "source_claim_snapshot": "When a release or product surface describes IDE readiness, it names exactly one roadmap rung—Files foundation, daily-use basic IDE, agent IDE, portable IDE platform/parity candidate, or full parity—and the IDE-00..19 crosswalk shows current acceptance and assurance refs plus every remaining gap; Monaco, Pierre, Tokyo Night, Vim, or a Zed-quality architecture alone can never satisfy a broader rung or Cursor-parity claim.",
    "title": "Assure AC-52"
  }
]
```

## Gates

No execution or release gates are inferred. Gate design remains blocked pending review.

```assurancespec-gates
[]
```

## Evidence Policy

Links are pointers, not verdicts. Missing or unreviewed evidence remains INCONCLUSIVE.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "needs_design",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This proposal cannot admit, execute, verify, waive, release, or change a public promise.

```assurancespec-authority
{
  "admitted_roles": [],
  "policy_state": "needs_design",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [],
  "verifier_roles": []
}
```
