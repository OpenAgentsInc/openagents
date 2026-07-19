# 2026-07-16 — Fable coordinator session: final closeout

The owner ended this session with a handoff to OpenAgents Desktop itself —
the Full Auto system this session built and dogfooded now inherits the work.
This is the terminal record of what landed, what is in flight, and where every
loose end lives.

## What this session shipped to main (chronological)

1. **Full Auto epic #8873** — all 13 hardening children closed: durable
   exactly-once continuation, workspace fail-closed binding, failure backoff,
   live in-flight UI, background question auto-resolve, two-process restart
   smoke, the loopback OpenAPI/MCP/CLI control surface, and the `start`
   bootstrap (spec rev 7, FA-AC-01..28).
2. **Full Auto dogfood, for real** — programmatic `start` → real Codex
   conversations that pushed their own commits to main: the no-GitHub-Actions
   invariant repair (#8905, filed and closed by the agent itself) and the
   #8911 token-ingestion engineering.
3. **/stats live** — the page fetches real production data. Deployed twice
   this session.
4. **QA harnesses** — QA-2 observer loop (first real production run 7/7) and
   QA-5 independent verifier (two real ACCEPT verdicts, self-verification
   refusal). Both closed with evidence. The verifier discipline then caught
   real defects in three lanes today.
5. **ACP-9 trusted peer profiles** (#8896) — fail-closed admission, 48 tests.
6. **DIST ladder (epic #8913), fable lanes:**
   - #8927 changelogs — human `/changelog` + agent ledger + UNRELEASED
     accumulator wired into CLAIM-RELEASE. Real rc.13 backfill.
   - #8916 staging (with two repair rounds after honest codex reviews) —
     target-descriptor staging, per-file native closure ledgers, byte-bound
     Forge verification (live flipped-byte refusal proof), pre-maker typing.
   - #8923 resolver — verified signed-release download resolution. Fixed the
     production 404 CTA. Live smoke against the real rc.13 feed.
   - #8924 /download page — resolver-driven, honest unavailability, byte-exact
     download proof. DEPLOYED live (claim released, pending codex review).
   - #8926 slice 1 — `pnpm run release`: the one-owner-command step graph,
     preflight, resumable transactions, dry-run, changelog step, receipts.
     coordinator/feed ride typed fixture ports (claim released, see below).
   - #8917 infra sub-workstream — three GCE release workers provisioned,
     toolchain-verified, stopped (evidence on the issue). Tailnet Mac
     inventory.
7. **#8911 opted-in usage counting** — owner-approved reworded consent copy
   landed. The full live proof ran on production (three exact rows, counter
   +204,425 with perfect arithmetic, opt-out purge, retry-exactly-once).
   Final config fix is with the codex lane (their commits supersede ours).
8. **Owner decisions executed and recorded**: consent copy approved
   (reworded, counts-only). Win32-arm64 deferred from the first release.
   Intel Mac restoration deferred (Rosetta evidence path per #8919's spec
   call). All in workspace `NEEDS_OWNER.md` and on the issues.

## Open threads and who owns them

- **#8911** — codex lane holds the superseding bounded fix (duplicate YAML
  key + gate wording), then closes.
- **#8917 coordinator + #8922 feed** — codex-reserved, unblocked as of the
  #8916 closure. Workers are waiting stopped in GCE
  (`purpose=desktop-release-worker`).
- **#8926** — claim released. Slice 1 is on main. Finishing = real port
  wiring for #8917/#8922 + one real RC-channel run + receipt. Port shapes in
  `scripts/release.ts` module header.
- **#8924** — landed + deployed + live receipts. Awaiting codex review for
  closure (claim released).
- **#8919/#8920/#8921/#8925** — per the epic order once the coordinator and
  feed land. #8925 must be verified by a non-implementing lane (QA-5 recipe).
- **#8930** — owner bug report from the proof window ("[working] bars stopped
  animating"), filed with behavior-contract + motion-proof requirements.
- **#8928** — Full Auto shared-Mac guard. Codex landed attribution fixes.
- **#8897/#8887** — codex ACP release-gate lane, deep in live proofs.

## Coordination protocol (the durable lesson)

Two coordinator lanes (fable, codex) ran this board in parallel all day. What
made it work, after two early duplicate builds were discarded:

1. `CLAIM` / `CLAIM-STATUS` / `CLAIM-RELEASE` comments on the issue, checked
   immediately before every dispatch — an earlier audit is never current.
2. Review-then-integrate: anything with an announced reviewer waits for the
   verdict. Closures seconds ahead of a pending review get reopened.
3. Independent verification both directions — fable's QA-5 verifier accepted
   codex work. Codex reviews rejected fable work twice with concrete
   file-level defects, and both repairs made the product materially better.
4. Worktree hygiene: read-only across lanes, mutation never.
5. Changelog UNRELEASED entries as part of CLAIM-RELEASE (docs/changelog/).

## Runtime state left behind

- GCE: three stopped release workers (openagentsgemini, us-central1).
- No fable subagents, monitors, or background tasks remain running.
- Local worktrees removed except `oa-fa-integrate` (clean, on main) and
  `oa-fullauto-dogfood` (clean, on an earlier main — the Full Auto workspace,
  the Desktop successor should refresh it before its next loop).
- The Full Auto registry in the 2026-07-16 dogfood userData has all threads
  disabled. Bootstrap a fresh loop with
  `pnpm --dir apps/openagents-desktop run full-auto -- start --workspace <ws>`
  against a control-enabled Desktop instance.

Handing off to the machine we built today. Good luck in there.
