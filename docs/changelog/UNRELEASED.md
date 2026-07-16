# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on `main` is part of the CLAIM-RELEASE protocol — see `README.md` in
this directory for the required format. `pnpm changelog roll` moves these
entries into the next dated release file.

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
- commits: (integration commit on main)
- contracts-specs: target_build_descriptor.v1, native_component_ledger.v1, build_receipt.v1 in apps/openagents-desktop/src/release-staging-contract.ts (integration point for ReleaseSet v2 refs)
- invariants: INVARIANTS.md DIST-03 entry incl. signer/notary nondeterminism exception; packaging entrypoints require OA_DESKTOP_TARGET
- evidence: tests/release-staging.test.ts (26) + release suite 99 green; live darwin-arm64 staging with identical ledgerRef across two runs; win32-arm64 typed missing_runtime_package refusal
- lane: fable-dist03-staging-20260716

Desktop packaging now requires an explicit build target: staging happens in a clean per-target workspace with target-correct native components, and every build emits a public-safe native-component ledger and a build receipt binding source revision, versions, toolchain, and worker identity. Unsigned development output is structurally inadmissible to publication.

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
