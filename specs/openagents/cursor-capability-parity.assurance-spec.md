---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.cursor.capability.parity"
assurance_revision: 2
title: "Cursor-Class Capability Parity AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposed AssuranceSpec creates exact criterion-to-obligation coverage without claiming that proof design, execution, evidence, admission, or release is complete.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "CP-AC-01",
      "CP-AC-02",
      "CP-AC-03",
      "CP-AC-04",
      "CP-AC-05",
      "CP-AC-06",
      "CP-AC-07",
      "CP-AC-08",
      "CP-AC-09",
      "CP-AC-10",
      "CP-AC-11",
      "CP-AC-12",
      "CP-AC-13",
      "CP-AC-14",
      "CP-AC-15",
      "CP-AC-16",
      "CP-AC-17",
      "CP-AC-18",
      "CP-AC-19",
      "CP-AC-20",
      "CP-AC-21",
      "CP-AC-22",
      "CP-AC-23",
      "CP-AC-24",
      "CP-AC-25",
      "CP-AC-26",
      "CP-AC-27"
    ],
    "document_digest": "sha256:a50322b69d6b85e4d65c38f349e65d6ab656a4b147fdf0e17061d1f0dbc27498",
    "path": "specs/openagents/cursor-capability-parity.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 3
  }
}
```

## Risk Model

No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.

```assurancespec-risks
{
  "risks": [],
  "source_digest": "sha256:a7e33c316fcfa1f3b092b5742ea90cd01c5475df9619d4fae967769b56728e24",
  "source_snapshot": "- Cursor changes quickly. Evidence pinning, observation dates, and automated\n  freshness warnings are required or parity becomes a stale marketing claim.\n- “Everything” can flatten sequencing. The ledger is complete; implementation\n  remains criterion-addressed and dependency-ordered through roadmap and work\n  authority.\n- An unbundled product can expose too many choices. Opinionated defaults and\n  progressive disclosure must coexist with inspectable advanced control.\n- Extension and computer-use breadth expand the attack surface. Isolation,\n  least authority, provenance, and receipts are launch requirements, not\n  follow-up hardening.\n- Local embedding models can underperform managed indexes, while remote indexes\n  can violate custody expectations. The product must state quality, cost,\n  corpus, and data-flow differences rather than presenting false equivalence.\n- Compatibility import can accidentally ingest secrets or proprietary state.\n  Import uses an allowlist and produces a complete disposition report.\n- “Better model” is not permanent architecture. Model quality remains a\n  replaceable plane and is measured independently from product parity.\n- A package checklist can look like IDE parity while project, document,\n  language, Git, terminal, and agent states still diverge. The integrated\n  generation-fenced journey corpus, not dependency presence, is the gate.\n- Public code sharing can accidentally become path/content exfiltration or a\n  browser authority path. Bundle compilation is allowlisted, bounded,\n  audience-aware, verifiable, revocable, and mutation-free.\n- Choosing Rust because the reference IDE uses Rust would create a second\n  application core and cross-language drift. Native helpers require measured\n  necessity, generated contracts, authority-free state, and reversal tests.\n- Treating Vim or a theme as library configuration would let adapter state\n  bypass the command, document, focus, accessibility, and teardown laws. Both\n  remain app-owned projections with explicit packaged journeys.\n- Shipping only Tokyo Night can be mislabeled as accessibility/theme parity.\n  It is the admitted initial default; the broader first-party theme corpus\n  stays a visible parity gap until separately proven."
}
```

## Assurance Scope

Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.

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
      "CP-AC-01"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-01-01",
    "source_claim_digest": "sha256:f83da7133ec72f6610c3c1e5b2a20806368b0e86bbe53d7d03767fb6fe3f3eb1",
    "source_claim_snapshot": "The maintained capability ledger contains every family and\nrequired field named in this spec, pins the Cursor evidence/version/date,\nand has no missing, duplicate, stale-without-warning, or “not needed” row.",
    "title": "Assure CP-AC-01"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-02"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-02-01",
    "source_claim_digest": "sha256:12bf184f0cc039b91ea480faa72b204bd4df8ac0766bc6657b836123105a9b69",
    "source_claim_snapshot": "A release claiming Cursor parity has no required row below\n`owner_accepted`; an owner exception names the preserved user outcome,\napproved substitute, expiry/review date, and evidence, and is never counted\nas parity until its substitute is accepted.",
    "title": "Assure CP-AC-02"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-03"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-03-01",
    "source_claim_digest": "sha256:97c4bf9917ac963e724db702098854c81779a5755c4b6f3b23e64cd28675d18e",
    "source_claim_snapshot": "One test repository can be opened in the classic workbench and\nagent-first window, and both project the same threads, drafts, queues,\nworktrees, terminal sessions, agent graph, changes, checkpoints, and\neffective execution identities without duplication or lost state.",
    "title": "Assure CP-AC-03"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-04"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-04-01",
    "source_claim_digest": "sha256:04a0aad9f2ecaea9c893fdd987083eb8ea46fb2cb9a7fe7b6fd61c9d869ae7ec",
    "source_claim_snapshot": "The editing corpus proves single-line and multi-line completion,\nnext-edit prediction, inline generation/transformation, multi-file apply,\naccept/reject/undo, diagnostics, semantic repository context, and exact patch\nprovenance under checked-in latency and correctness budgets.",
    "title": "Assure CP-AC-04"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-05"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-05-01",
    "source_claim_digest": "sha256:9bf77a2276b03cb0d54e2e29855c9783b65147be973c04997aa658dc99357697",
    "source_claim_snapshot": "Ask, plan, execute, review/debug, design, and custom modes compile\nto explicit model, tool, permission, placement, memory, and instruction\npolicies; changing a mode cannot silently broaden authority or conceal the\neffective model.",
    "title": "Assure CP-AC-05"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-06"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-06-01",
    "source_claim_digest": "sha256:e597cd2081d85bc77af3af8ca0b39cab763a6130a4b20a5c3d5544b8a1f89b17",
    "source_claim_snapshot": "Parallel-session, subagent, worktree, background-shell, and\nbest-of-N tests prove complete child transcripts, isolated mutation claims,\ncollision detection, explicit comparison, deterministic fan-in, and an\nacceptance decision outside the producing agent.",
    "title": "Assure CP-AC-06"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-07"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-07-01",
    "source_claim_digest": "sha256:0b5f4f1867ed5d5927f74cf10ccb2ca8385020e0fe0920af8b9f10c2d834b1bf",
    "source_claim_snapshot": "Browser, preview, screenshot, DOM, and computer-use tests prove\npartition isolation, declared network/OS authority, approval enforcement,\nsecret redaction, and action receipts while preserving the supported Cursor-\nclass workflow.",
    "title": "Assure CP-AC-07"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-08"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-08-01",
    "source_claim_digest": "sha256:71949997bffe75ee5e5254bf0c6b19795a163d123ced72e39176ae56d378a922",
    "source_claim_snapshot": "Background and automation tests start from schedule, repository,\nissue/PR, and webhook/manual triggers; run under local, owner-managed, and\nOpenAgents-managed placement; survive client closure/restart; enforce caps\nand idempotency; notify and accept intervention; and produce reviewable\noutcomes without duplicate execution.",
    "title": "Assure CP-AC-08"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-09"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-09-01",
    "source_claim_digest": "sha256:813f6004d21d9c2f6555dea95dee120f0fd91b6cc1102a503a27c2b6ff0d7d04",
    "source_claim_snapshot": "The same portable session can start on Desktop, continue in a\nbackground placement, be supervised from web and mobile, and return to an\nowner host with one identity and at most one accepting attachment generation.",
    "title": "Assure CP-AC-09"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-10"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-10-01",
    "source_claim_digest": "sha256:f6992aaf30d10b58d9eab7c7f7ef43017aabb09c26ca86f4fb1fbe5c4010c1e2",
    "source_claim_snapshot": "The CLI, SDK, terminal UI, Desktop, web, and mobile invoke the\nsame stable command IDs and observe the same durable outcomes; scripting and\nJSON modes expose typed errors and never introduce a parallel authority path.",
    "title": "Assure CP-AC-10"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-11"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-11-01",
    "source_claim_digest": "sha256:c7b179fe2123eb80912a574bfecd4a1720783c51a121a36c633b82d682a5f8e7",
    "source_claim_snapshot": "Skills, MCP servers, rules, hooks, plugins, extensions, and\nsubagent definitions support discover/install/import/export/update/disable/\nremove and team distribution, with provenance, compatibility, declared\npermissions, isolation, rollback, and receipts; untrusted code never executes\ninside the trusted shell or engine process.",
    "title": "Assure CP-AC-11"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-12"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-12-01",
    "source_claim_digest": "sha256:e178ce1a3192779caafc867922290b00366bd3efc86455022f2e6968e7860986",
    "source_claim_snapshot": "For every session, the placement disclosure and receipt identify\nselected and effective harness, model/provider/account, execution target,\nsandbox profile, network policy, index/data flows, cost/usage, and retained\nartifacts; automatic substitution is visible and policy-bound.",
    "title": "Assure CP-AC-12"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-13"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-13-01",
    "source_claim_digest": "sha256:7ad3677807148b7069666c5cf7c5503a1685c3f88a9b6fbbeb15edd6aafa2e6f",
    "source_claim_snapshot": "With remote embeddings disabled, repository path/text/symbol\nsearch and agent context remain functional; each optional local or remote\nsemantic backend has an independent scope, freshness, rebuild, export,\nretention, and verified deletion path.",
    "title": "Assure CP-AC-13"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-14"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-14-01",
    "source_claim_digest": "sha256:0529856f9ff776f8de39ab6d4c59d5e5f21ae2a850b1f449cee0eb9adce4d568",
    "source_claim_snapshot": "The data inventory enumerates all local and remote data classes\nnamed in scope, reports size and last write, and proves selective repository-\nknowledge deletion, chat-preserving index reset, complete export, account/\ndevice revocation, full local reset, and remote deletion/tombstone receipts.",
    "title": "Assure CP-AC-14"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-15"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-15-01",
    "source_claim_digest": "sha256:14b26fe02d5a7a0159eed6e81e99da9ed1a5745b789ca97c36537ea125767fd5",
    "source_claim_snapshot": "macOS, Windows, and Linux release evidence covers supported x64\nand arm64 targets, signed update and rollback, startup, editor, agent,\nterminal, browser, extension, accessibility, and offline-degradation matrices;\nunsupported platform/capability pairs are visible gaps, not silent omissions.",
    "title": "Assure CP-AC-15"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-16"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-16-01",
    "source_claim_digest": "sha256:0209b0115752ea042154a842677cba00cbff61a940e0c7c45cb56c030d5b2bac",
    "source_claim_snapshot": "Fast Follow refresh detects a new or changed Cursor capability,\nrecords exact evidence and freshness, creates or updates the parity gap\nwithout granting mutation authority, and links any admitted implementation\nto target-owned acceptance and assurance refs.",
    "title": "Assure CP-AC-16"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-17"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-17-01",
    "source_claim_digest": "sha256:3758a4e3448c072108d292d7ca75b7552aab1322fe1fbc0f3d3e110a1c5855df",
    "source_claim_snapshot": "A migration dogfood imports supported portable settings, rules,\nskills, MCP configuration, and keybindings from a clean Cursor profile without\nimporting credentials, opaque telemetry IDs, proprietary binaries, or hidden\ncloud state, and reports every unsupported item explicitly.",
    "title": "Assure CP-AC-17"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-18"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-18-01",
    "source_claim_digest": "sha256:86a0edb23831ca0bd2a5b0228ed2ad35182ad18892caea8688fa42bcc033c4d3",
    "source_claim_snapshot": "Performance, accessibility, offline, crash/restart, revocation,\nprovider failure, index corruption, and network-partition fault suites fail a\nparity claim when the equivalent workflow disappears, degrades silently, or\nfabricates completion.",
    "title": "Assure CP-AC-18"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/src/ide/run-host.test.ts",
      "apps/openagents-desktop/src/ide/run-service.test.ts",
      "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-packaged-run.json",
      "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-acceptance.json"
    ],
    "criterion_refs": [
      "CP-AC-19"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-19-01",
    "source_claim_digest": "sha256:c3b0aec3647337f7c1a7e4b1df16825f74d40db14ed5e74c157a1a836946e489",
    "source_claim_snapshot": "The integrated-IDE corpus opens the same file from Finder,\nExplorer, quick open, search, Problems, symbols, Git, and an agent backlink\ninto one current Monaco document; proves multi-root/worktree isolation,\ndirty/conflict/restart recovery, language/navigation, Git/review,\nterminal/tasks/tests/debug, and agent proposal/apply/undo without another\neditor; and rejects every stale attachment/document/service generation.",
    "title": "Assure CP-AC-19"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-20"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-20-01",
    "source_claim_digest": "sha256:f06574d421c001f5c80cfa695f9bc0a2b67d6de1a08ad3065662d99034cabe09",
    "source_claim_snapshot": "A coding session can attach to an exact project, disclose a\ngeneration-bound context manifest, receive a version-bound multi-file\nproposal, review it in the same Changes/diff plane, apply or refuse it under\ncurrent authority, rerun diagnostics/tests, and backlink between code and\nthe creating turn without granting the harness direct editor authority.",
    "title": "Assure CP-AC-20"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-21"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-21-01",
    "source_claim_digest": "sha256:dc560bcecd172cb2fb93a0a5c2969c4c4b3a49fff44f036fbae4c39fa5479133",
    "source_claim_snapshot": "Local, owner-managed, and admitted managed project-capability\njourneys use identical command/result contracts and stable refs while\nexposing effective placement, version, latency, freshness, custody, and\nattachment generation; disconnect, revocation, or incompatibility never\nsilently uploads, installs, relocates, or substitutes a capability.",
    "title": "Assure CP-AC-21"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-22"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-22-01",
    "source_claim_digest": "sha256:41afd623dc79aaad1c524c10bc939244404ac0bc6e47e532346584914dd5e93d",
    "source_claim_snapshot": "A public or audience-scoped code-share journey compiles an\nallowlisted, verifiable bundle containing only selected tree/excerpt/diff/\nproposal/problem/test/artifact/log/agent/receipt evidence; snapshot/live\nmode, omission, staleness, expiry, revocation, access, and public-copy risk\nare visible; and the page has no workspace, terminal, Git, model, or mutation\nauthority and leaks no root, environment, credential, secret, private\ncontext, raw terminal, embedding, or unselected repository content.",
    "title": "Assure CP-AC-22"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents-desktop/scripts/check-ide-boundaries.ts",
      "apps/openagents-desktop/src/ide/run-benchmark-contract.test.ts",
      "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-10-run.json"
    ],
    "criterion_refs": [
      "CP-AC-23"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-23-01",
    "source_claim_digest": "sha256:ffa0cf646695b5cc68934dafc124c24b3045446756e10faf1c81c2119ab57ed5",
    "source_claim_snapshot": "An architecture audit finds every authoritative project,\ndocument, language, Git, terminal/task/debug, agent, policy, persistence,\nprojection, and receipt class in Effect/TypeScript. Any Rust helper is a\nprocess-opaque bounded primitive with generated conformance fixtures,\nexplicit failure behavior and reversal threshold, and no credential,\nproject/session/policy/database/receipt authority.",
    "title": "Assure CP-AC-23"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-24"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-24-01",
    "source_claim_digest": "sha256:7c55cca0a2904868851724e13cf0990a5e5414b54787a561877b351ab6f2f575",
    "source_claim_snapshot": "The maintained daily-editor corpus enables and disables Vim\nfrom Settings, command palette, and status control; proves persistent\noff-by-default mode, core Normal/Insert/Visual operations, counts, operators,\ntext objects, registers, repeat, search, supported colon commands, clipboard,\nconflict-safe save/close, split/worktree isolation, IME/accessibility,\nrestart, and complete listener teardown; and finds no extension-host or\ndirect-Monaco authority path.",
    "title": "Assure CP-AC-24"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-25"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-25-01",
    "source_claim_digest": "sha256:bbdd9ed9034aaadbe481819845cd119ce5fafb3a7b779d28c8da77aeb107e031",
    "source_claim_snapshot": "Every initially supported Desktop IDE projection renders from\none provenance-pinned Tokyo Night semantic token plane before first paint,\npasses checked contrast and non-color-state journeys, works offline, and\nretains document/terminal/review state across theme initialization. A full-\nparity claim additionally requires the deferred first-party light, high-\ncontrast dark/light, and system-following corpus; Tokyo Night alone is never\nreported as complete theme/accessibility parity.",
    "title": "Assure CP-AC-25"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-26"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-26-01",
    "source_claim_digest": "sha256:1a71e0c105fd7613327c0ff90853ae4e6e3c3aeefdb06c4bc8fc4651c3d0fe54",
    "source_claim_snapshot": "An architecture audit finds a single Effect Schema source and\nderived TypeScript type for every persisted, IPC, helper, mobile/web, and\npublic-share boundary; constrained branded refs and stable codegen schema\nidentifiers where applicable; no raw interface or handwritten union acting\nas a parallel contract; and every untrusted input decoded before use.",
    "title": "Assure CP-AC-26"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "CP-AC-27"
    ],
    "disposition": "required",
    "id": "AO-CP-AC-27-01",
    "source_claim_digest": "sha256:3685cf8baec302d586766acd45359774ceba4b4cd43b796d410f1906972b032a",
    "source_claim_snapshot": "A lifecycle and release audit finds IDE capabilities composed\nas Context.Service/Layer.effect services with named Effect.fn operations,\nSchema.TaggedErrorClass failures, and scoped resource interruption, and maps\nevery Cursor ledger row to the exact IDE-00..19 packet, release rung,\nacceptance ref, proposed/admitted assurance state, remaining gap, and owner\ndisposition without promoting a narrower rung by inference.",
    "title": "Assure CP-AC-27"
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
