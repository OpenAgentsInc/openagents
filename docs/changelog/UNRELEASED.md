# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

## Windows releases are x64-only (#8913)

- issues: #8913, #8917, #8920, #8924, #8926
- commits: integration commit (this entry)
- contracts-specs: apps/openagents-desktop/src/release-set-contract.ts, docs/deploy/openagents-desktop-cross-platform-release.md ProductSpec 1.1.0
- invariants: the promotable Desktop matrix is five targets and eleven artifacts; Windows ARM64 is removed from ReleaseSet, promotion, and download availability
- evidence: scripts/desktop-release-coordinator.test.ts, apps/openagents-desktop/tests/release-set-contract.test.ts, apps/openagents.com/apps/start/src/desktop-download-resolver.server.test.ts
- lane: codex-root-dist-x64-policy-20260717

OpenAgents Desktop for Windows will ship on x64 only. Windows-on-Arm devices
will no longer see an ARM64 installer choice, avoiding a support promise that
cannot yet be built and tested on owned native hardware.

The signed ReleaseSet, release coordinator, owner command, update feed tests,
and `/download` catalog now converge exactly five targets and eleven artifacts.
The dormant Windows ARM64 staging descriptor remains non-promotable scaffolding
for a future reviewed policy revision.

## Full Auto mode for the Desktop composer (#8852)

- issues: #8852, #8853, #8873 (hardening epic: #8874–#8886)
- commits: 5357ae8377, d480f779aa, c74bc449f1, 5bc6e62fae, 0353b307fa, 3967982eaa
- contracts-specs: specs/desktop/full-auto.product-spec.md, specs/desktop/full-auto.assurance-spec.md
- invariants: none changed (control surface is opt-in, loopback-only, scoped-bearer-gated; enable authority refuses on workspace mismatch)
- evidence: FA-H12 two-process restart smoke receipt recorded in specs/desktop/full-auto.product-spec.md (commit e964e4bd82)
- lane: backfilled by fable-dist14-changelog-20260716; original landings by the #8852/#8873 lanes

The Desktop composer gains a per-conversation Full Auto toggle: when it is
on, the agent keeps working after each completed turn without waiting for you
to resend, the toggle state survives an app restart, and background Full Auto
turns stay visible with a working stop control. An opt-in local control
surface (HTTP API, MCP, and command line, loopback-only) can start and
supervise Full Auto runs programmatically.

Engineering detail: the continuation decision is owned by the main process
and persisted per-thread (rev 2, #8853); dispatch is serialized with durable
leases and typed failure/backoff handling (FA-H2–FA-H6); the FA-H13 control
surface is gated by `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1`, binds to
127.0.0.1 with a per-process scoped bearer, and serves a hand-authored
OpenAPI 3.1 document that the MCP and CLI clients pass through.

## Live network stats on openagents.com/stats

- issues: none
- commits: aeb5a73bb5
- contracts-specs: none
- invariants: none changed (fail-soft fetches; no fabricated values)
- evidence: production endpoint inventory recorded in apps/openagents.com/apps/start/src/routes/-stats-data.ts (verified against production 2026-07-16)
- lane: backfilled by fable-dist14-changelog-20260716; original landing by the /stats live-data lane

The stats page at openagents.com/stats now shows live data instead of static
placeholders: the tokens-served counter, the daily history chart with a
daily/cumulative toggle, model and channel mix, pylon status, and forum
launch gates all fetch live public data in the browser. Anything that cannot
be fetched is marked unavailable honestly — the page never invents a number.

## Desktop release changelogs (DIST-14, #8927)

- issues: #8927 (epic #8913)
- commits: (integration commit on main)
- contracts-specs: RELEASE_NOTES_MAX_LENGTH=2000 exported for ReleaseSet v2 (#8915); /changelog route in the Start route table
- invariants: human changelog copy carries no commit hashes, internal codenames, or forbidden vocabulary (route test enforces)
- evidence: scripts/changelog.test.ts (17), -changelog.test.tsx (6), pnpm changelog check green
- lane: fable-dist14-changelog-20260716

Every release now publishes two changelog artifacts: a human-centric changelog at openagents.com/changelog and a detailed engineering ledger under docs/changelog/. Landing lanes append their entry to docs/changelog/UNRELEASED.md as part of CLAIM-RELEASE; `pnpm changelog roll` cuts the release file and emits the bounded release-notes string for the signed release set.

## Target-aware release staging with provenance (DIST-03, #8916)

- issues: #8916 (epic #8913)
- commits: (integration commit on main; repaired per the two independent review comments and the codex re-review)
- contracts-specs: target_build_descriptor.v1 (exact format coverage), native_component_ledger.v1 (pre-maker-staging phase, per-file §9 closure + toolchain/lockfile/OS metadata, planned maker identities), build_receipt.v1 (full toolchain + stagedTree/asarAllowlist gates + per-artifact ACTUAL makerRef refusing pending refs) in apps/openagents-desktop/src/release-staging-contract.ts (integration point for ReleaseSet v2 refs); packaging entrypoints require OA_DESKTOP_STAGING_WORKSPACE via scripts/stage-and-package.ts
- invariants: INVARIANTS.md DIST-03 entry incl. signer/notary nondeterminism exception; staged-tree-only Forge consumption with before-copy/post-package ledger byte binding and actual-tool identity refusal; live post-package ASAR gate with per-closure-entry placement fidelity; staging workspace cleanup with explicit --retain
- evidence: tests/release-staging.test.ts (46) + release suite green; real isolated darwin-arm64 staging with identical ledgerRef across two independent runs and a real Forge package/asar assembly consuming the staged tree; win32-arm64 typed missing_runtime_package refusal
- lane: fable-dist03-staging-20260716

Desktop packaging now requires an explicit build target and a real isolated staging pipeline: each target stages in a clean temporary workspace from the exact exported source revision, derives every runtime/tool pin from that exported revision, executes a locked target-only production install from the immutable lockfile (staged runtime versions must equal the exact locked versions or staging refuses typed), builds native components with an explicit Rust triple into the staging workspace, and every build emits a public-safe per-file pre-maker-staging native-component ledger (ProductSpec §9 metadata: lockfile digest, OS image, Electron/Node/pnpm/Forge/maker/Rust/compiler identities, per-executable architecture/signing/ASAR state) and a build receipt binding descriptor, toolchain, gate results, final artifact digests with actual maker identities, and worker identity. Electron Forge packages ONLY the staged tree (the developer checkout and shared node_modules are never the packaged source), verifies the ledger against the CURRENT staged bytes before copy and post-package, refuses mismatched installed Electron/Forge versions, and a live post-package gate re-audits the REAL app.asar entry list with per-closure-entry placement fidelity before any maker or signing work. Descriptors require exact per-target format coverage; unknown or truncated executable identity and escaping symlinks fail closed; auto-created staging workspaces clean up on success and error unless explicitly retained; unsigned development output is structurally inadmissible to publication.

## Verified desktop download resolution (DIST-10, #8923)

- issues: #8923 (epic #8913)
- commits: (integration commit on main)
- contracts-specs: openagents.desktop.download_resolution.v1 + download_telemetry.v1; GET /api/public/desktop-download (+/artifact 302); DIST-09 feed-path assumptions documented in desktop-download-resolver.server.ts
- invariants: /download INVARIANTS entry annotated; unavailable responses carry no URL; broken v2 never downgrades to v1
- evidence: 57 resolver tests + seam 7 + start suite 305 green; live smoke resolved signed 0.1.0-rc.13 and 302'd to an HTTP-200 artifact
- lane: fable-dist10-resolver-20260716

openagents.com now resolves Desktop downloads from the cryptographically verified release feed instead of hand-written links. Visitors get the right installer for their platform with explicit alternatives, and the site can never serve a download URL that does not match the signed release — including fixing a dead pinned link that had been returning 404.

## One owner release command: step graph, preflight, dry-run, resumable transactions (DIST-13, #8926 slice 1)

- issues: #8926 (epic #8913)
- commits: (integration commit on main)
- contracts-specs: openagents.desktop.release_transaction.v1 and openagents.desktop.release_receipt.v1 in scripts/release.ts; typed integration ports ReleaseCoordinatorPort (#8917) and ReleaseFeedPort (#8922) with fixture implementations only
- invariants: implements the automated boundary of the DIST-01 one-command entrypoint invariant (dry-run, durable resume, idempotence, pre-promotion failure); fixture ports refuse non-dry-run execution, so no channel pointer can be touched by this slice
- evidence: scripts/release.test.ts (28 green); live --dry-run receipt exercised from this revision
- lane: fable-dist13-release-command-20260716

Releasing OpenAgents Desktop now has its single owner entrypoint: `pnpm run release` plans and walks the whole nine-step release pipeline — checks, worker bring-up, six-target builds, test gates, candidate, changelog, promotion, and public-page verification — as one resumable transaction with a clear receipt at the end, and a safe dry-run mode that walks the full plan without building or publishing anything.

Engineering detail: steps 2-5 and 7-8 execute only against typed ports (fixtures) until the #8917 coordinator and #8922 feed land; preflight (clean origin/main freeze, version authority, toolchain pins, credential PRESENCE), the DIST-14 changelog roll step, transaction state under `.release/`, owner gates named up front, and the ProductSpec §11.1 receipt are real.

## Platform-aware download page (DIST-11, #8924)
- issues: #8924 (epic #8913; consumes #8923, notes seam from #8927)
- commits: (integration commit on main)
- contracts-specs: consumes openagents.desktop.download_resolution.v1; /download route loader + page projection in apps/openagents.com/apps/start/src/routes/{download.tsx,-download-page.tsx,-download-data.ts}
- invariants: DIST-01 /download entry annotated with the DIST-11 page boundary (root INVARIANTS.md); no policy relaxed — platform availability admitted only by the promoted release set
- evidence: 23 page/route tests + start suite 325 green; local built-server SSR smoke (detected, unknown, and override clients) plus a CTA fetch whose downloaded bytes hash-matched the signed release set sha256
- lane: fable-dist11-download-page-20260716
The download page at openagents.com/download now shows live release truth instead of hand-written labels: it detects your platform, offers one clear download button for a supported machine, and lists every architecture and format explicitly — with version, channel, size, minimum OS, release notes, and verification guidance. Platforms without a promoted build say so honestly ("Not yet available") instead of being claimed, and if the release feed cannot be verified the page shows no download links at all rather than a stale or fake one. The page works fully without JavaScript, and every public download button routes through the download page and the verified redirect — never a raw file link.
Engineering detail: the route loader resolves server-side against the DIST-10 resolver during SSR (client-hint/user-agent detection with explicit ?target/?format/?channel overrides), so the no-JS page is the fully resolved page; marketing surfaces dropped the last hard-coded MAC_RELEASE version/size constants; download-selection telemetry fires only through the artifact 302, never on page render; structured data and the meta description are derived from the promoted catalog so SEO copy cannot overclaim platforms.

## Opted-in Desktop local Codex usage now counts toward the public tokens-served counter (#8911)

- issues: #8911
- commits: 6f08644cb6, 8809f79b56, (this closeout commit)
- contracts-specs: openagents.desktop.codex_turn_usage.v1 / codex_turn_admission.v1 (apps/openagents-desktop/src/desktop-codex-usage-reporter.ts, apps/openagents.com/workers/api/src/desktop-codex-usage-routes.ts); server rollout gate DESKTOP_CODEX_USAGE_INGEST_ENABLED added to scripts/cloudrun/env-production.yaml and env-staging.yaml
- invariants: none changed — the ordinary shipped control keeps user consent default-off, the server ingest gate remains independently controlled, and every report still requires a server-verified session and pre-admission; the live proof temporarily enabled consent and ingest, then restored the deployed server gate off pending the integration deploy
- evidence: issue #8911 live-proof receipts comment (three exact token_usage_events rows: ordinary 14,096; restart-retry 14,303; Full Auto continuation 176,026 — each matching the public counter delta exactly once); docs/sol/2026-07-16-desktop-local-usage-opt-in-verification.md
- lane: fable-8911-live-proof-20260716

If you turn on "Share local Codex usage" in Desktop Settings, turns you run with your own local Codex account now count toward the public tokens-served counter on openagents.com. Only token counts, the model name, and a one-time turn reference are sent — never your prompts, responses, files, paths, account names, or credentials — and it stays off until you explicitly opt in. Turning it off stops reporting immediately and deletes anything queued.

Engineering detail: the owner approved the consent disclosure (reworded counts-only copy at 8809f79b56). The opted-in live proof ran on production per the verification runbook: signed-in pre-turn admission, exact-usage ingest with ledger idempotency, restart persistence with an induced 503 retried exactly once, opt-out zero-traffic, and a Full Auto continuation through the same lane seam. This commit adds the server gate to the sanctioned Cloud Run rollout configuration; enablement ships with the next production deploy from main.
