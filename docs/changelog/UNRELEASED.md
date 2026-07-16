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

Every release now publishes two changelog artifacts: a human-centric changelog at openagents.com/changelog and a detailed engineering ledger under docs/changelog/. Landing lanes append their entry to docs/changelog/UNRELEASED.md as part of CLAIM-RELEASE; `pnpm changelog roll` cuts the release file and emits the bounded release-notes string for the signed release set.

- issues: #8927 (epic #8913)
- commits: (integration commit on main)
- contracts-specs: RELEASE_NOTES_MAX_LENGTH=2000 exported for ReleaseSet v2 (#8915); /changelog route in the Start route table
- invariants: human changelog copy carries no commit hashes, internal codenames, or forbidden vocabulary (route test enforces)
- evidence: scripts/changelog.test.ts (17), -changelog.test.tsx (6), pnpm changelog check green
- lane: fable-dist14-changelog-20260716
