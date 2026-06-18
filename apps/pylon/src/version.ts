// Single source of truth for the Pylon release version.
//
// The `bun --compile` binary does NOT bundle package.json, so the compiled
// Pylon reports THIS constant from `status --json` and the OTA self-updater
// compares against it. Keep package.json's "version" in sync, but treat this as
// authoritative for the running binary.
//
// Bump for each release cut. v1.0-rc cut: 2026-06-16 (#5122).
// rc.9: spark-selftest diagnostic + readiness auto-report + binary SDK guard (#5166).
// rc.14: Spark wallet send/withdraw + Spark-primary balance + Spark treasury
// payouts (#5176). Supersedes the rc.13 cut, which bumped package.json but NOT
// this constant — so rc.13 binaries self-reported rc.12, auto-update-looped, and
// showed the wrong version in /api/pylons. Keep BOTH in sync on every cut.
// rc.33: install the Breez SDK stdout guard as a TOP-LEVEL side effect of the
// first entry import (src/breez-stdout-guard.ts) so the SDK's "Node.js storage
// automatically enabled" banner cannot reach stdout before the guard is in
// place in the compiled binary. The rc.32 runtime guard fixed backup-status but
// not status --json, because the bundled binary could eval the SDK before main()
// installed it. Breez stays lazily imported.
// rc.34: claim tip-recipient readiness with the node's native, derived/static
// Spark address (Spark→Spark, registration-free) as the primary tip destination
// (#5345). The Spark Lightning Address becomes a best-effort optional add for
// external Lightning senders and no longer blocks readiness when its LSP is
// unreachable.
// rc.35: add `pylon sessions exec` — a blocking run-to-completion task primitive
// (W-1, #5377). It spawns a coding session and drives its turn loop to a terminal
// state over the existing control verbs (session.spawn/list/events/artifact),
// returning a structured JSON result (final state, summary, changeset, verify
// outcome, refs). Exit 0 on success-terminal, nonzero on failure/timeout. New CLI
// surface, so the version bumps.
// rc.36: add `pylon sessions exec --on-approval auto` (W-3, #5379) — a BOUNDED
// autonomous approval policy so a coding task runs to completion without manual
// per-step approval, within safety bounds and fully audited. Auto-approves only
// allow-listed, in-scope, in-bounds actions; escalates/denies spend/secret,
// destructive (rm -rf / force-push / history rewrite), network/exfil, and
// out-of-scope-path approvals; caps auto-approvals per session + a wall-clock
// window. Every decision lands in the result `autoApprovals[]` audit trail with
// the approval ref + a stable reason. NOT a blanket bypass and NOT the supervised
// danger-mode. New CLI surface (--approval-policy, --max-auto-approvals,
// --auto-window-seconds, --auto-out-of-bounds), so the version bumps.
// rc.37: fix `pylon sessions exec --verify` (and `sessions spawn --verify`)
// reporting `verification_failed` even when the work succeeded (#5389, EPIC
// #5376). The verify string was whitespace-split into an argv, so the node
// tried to exec a program literally named like the whole command (a spawn
// error) and quoted/multi-word args were mangled — a TRUE condition could
// report failed. Verify now runs shell-parsed (`sh -c "<cmd>"`) in the
// session's worktree cwd, so the real exit code (0 → passed, nonzero → failed,
// distinct from a spawn error) drives the verify outcome. Observable behavior
// change to an existing CLI flag, so the version bumps.
// v1.0.0: owner-authorized stable cut from the rc.37 line (#5393, Launch L-1).
// Promotes the rc line to the stable `latest` npm dist-tag so a fresh
// `npx @openagentsinc/pylon` installs the Bun/Effect earning-capable node
// instead of the deprecated 0.2.5 GitHub-asset launcher. No code change vs
// rc.37; this is the release-version promotion only. Keep package.json in sync.
// v1.0.1: first post-launch patch from the v1.0 self-serve shakeout. Carries
// the `balance --json` fix (reads the local wallet projection, not the empty
// network earnings ledger) and Gap #2 (surfaces an unregistered Spark payout
// target in `wallet status` + warns on `training claim` so a contributor never
// earns to nothing). Observable CLI behavior change, so the version bumps.
// v1.0.2: Spark wallet helper now auto-starts via the daemon's warm session.
// `wallet status`/`balance` route through the running daemon's already-warm
// Spark SDK instead of doing a cold per-read SDK build that contended with the
// daemon for the same SQLite file and timed out (the false `daemonOnline:false`
// / `helper_unavailable` a node operator saw). Now `daemonOnline: true`
// automatically with no manual step or env flag; genuine failures surface a
// reason-qualified blocker. Observable behavior change, so the version bumps.
// v1.0.3: node-home auto-resolution + read-only live-node queries. The CLI no
// longer blindly falls back to a seedless `~/.pylon` — it auto-discovers the
// seed-bearing node home (e.g. `~/.openagents/pylon`) when PYLON_HOME is unset,
// so `wallet status`/`status`/`doctor` find the real node without exporting an
// env var. `status` and `doctor` now detect a running node and read its state
// read-only (and accept `--remote`/`--connect`) instead of trying to bind the
// control port and crashing when the GUI node already holds it. Observable
// behavior change, so the version bumps.
// v1.0.4: fix npx install on Linux without bun. nostr-effect (a transitive git
// dep via nip90) ran a bun-requiring prepare hook on consumer npm install,
// crashing npx with code 127 when bun was absent; now a Node-only guard no-ops.
// Republished with nip90 0.1.1 carrying the fixed nostr-effect pin.
// v1.0.5: supersedes 1.0.4, which packed a stale nip90@0.1.0 (broken bun prepare)
// instead of 0.1.1; 1.0.5 correctly references nip90@0.1.1 with the fixed pin.
export const PYLON_VERSION = "1.0.5"
export type PylonVersion = typeof PYLON_VERSION

// Composed client-version string sent in presence/heartbeat payloads.
export const PYLON_CLIENT_VERSION = `openagents.pylon@${PYLON_VERSION}` as const
export type PylonClientVersion = typeof PYLON_CLIENT_VERSION
