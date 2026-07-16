# Autopilot workbench T15 integration proof

Date: 2026-07-16  
Issue: #8872  
Epic: #8857  
Proof base: `d4b8306d686a522effcdc12c0eb0a5911ed80296` (`origin/main` when the recovery worktree was created)

## Verdict

The shared typed workbench is integration-ready on this proof base. Desktop,
the component gallery, Splash, Share, and Khala Chat Sync all use the intended
Khala/Autopilot visual grammar. Every typed `WorkbenchItem` kind has an explicit
dispatch branch, the default renderer does not stringify arbitrary payloads,
and the complete Desktop verification passes after removing one confirmed
test-only build race.

This receipt does not claim authenticated Khala Sync. The local
`/khala/chat-sync` capture correctly stops at its credential gate, while the
focused render test proves its shared message component path. It also does not
claim that a pending approval was produced in the live Codex run; approval
variants are present in the component-family pixel proof and exercised by the
shared component tests.

## Confirmed defect and fix

The first clean Desktop verification reached 170 passing test files / 1,689
passing tests, then failed in `tests/build.test.ts` with:

```text
EEXIST: file already exists, mkdir '.../apps/openagents-desktop/dist/builtin-skills'
```

`build.test.ts` and `release-preflight.test.ts` could independently start the
same destructive `dist/` build in parallel. The fix moves the real-artifact
release oracles into `build.test.ts`, the suite's single build owner, and leaves
the preflight file with its pure and repository-identity checks. No oracle was
removed. The two-file focused suite then passed three consecutive runs, and the
complete Desktop verification passed.

After this receipt commit was rebased over the newly landed ACP runtime work,
Desktop typecheck exposed a second current-main integration failure: shared
`agent-client-protocol` and `agent-stdio-transport` sources use the ES2023
`Array.prototype.toSorted` API, while the Desktop compiler declared only the
ES2022 library. Desktop runs on the repository's Node 24 / current Electron
baseline, and both owning shared packages already declare ES2023. Advancing the
Desktop `lib` declaration to ES2023 restores the source-level contract without
changing its emitted target. Desktop typecheck and the focused build/preflight
suite pass with that compatibility fix.

## Automated verification

All commands ran in a clean dedicated worktree except for the fix and receipt
recorded here.

| Gate | Result |
| --- | --- |
| `pnpm run check` | pass |
| Desktop `pnpm run verify` | pass: typecheck; 171 files / 1,689 tests, 39 skipped; production build; native smoke; React smoke |
| build/preflight race stress | pass: 3 consecutive runs, 2 files / 10 tests each |
| focused Start route sweep | pass: 5 files / 27 tests |
| Start production build | pass |
| focused shared workbench sweep | pass: 7 files / 53 tests |
| `packages/ui` typecheck | pass |
| `git diff --check` | pass |

The Start sweep covered `-components-workbench-page`, `-share`,
`-share-fetch`, `-chat-sync`, and `-splash`. The shared sweep covered all test
files under `packages/ui/src/workbench`.

## Completeness and raw-payload audit

`packages/ui/src/workbench/dispatch.tsx` explicitly handles all fourteen
contract kinds: `message`, `reasoning`, `command`, `fileChange`, `toolCall`,
`agent`, `plan`, `approval`, `meter`, `notice`, `compaction`, `sleep`, `review`,
and `hook`. The `/components/workbench` completeness test additionally requires
every runtime export of `@openagentsinc/ui/desktop-workbench` to be referenced
by the gallery and passed in the focused Start sweep.

A production-only `JSON.stringify` sweep of the Desktop renderer and shared
workbench found four call sites. They are limited to a clone/decode boundary,
two local-storage persistence writes, and a memoization signature. None supplies
default visible content. The history renderer documents and tests the sanctioned
exception: raw input can be opened from the item inspector, while JSON-shaped
tool input and opaque blobs are summarized or suppressed in the default card.

## Pixel proof

The first four captures were made from the Start production preview bundle with
Playwright Chromium at a 1600 × 1000 viewport, dark color scheme, reduced motion,
and disabled screenshot animations.
The Share endpoint was intercepted with the same widened, schema-valid public
projection used by the T14 tests so the route displays every newly shared typed
kind without using private data. Each image was visually inspected after capture.

1. [`01-components-workbench.png`](./01-components-workbench.png) — 1600 ×
   11104. Full component family: messages, reasoning, commands, turn diffs,
   tool calls, plans, approvals, delegated agents, meters, long-tail rows, work
   groups, composer, session rail, header, and shell. SHA-256
   `7e9852e6afdf450363b9249a2a489f45ba8f2027da617335ed5198314ae170f3`.
2. [`02-splash.png`](./02-splash.png) — 1600 × 2434. Public Desktop surface
   with command, delegated-agent, plan, narrative, and composer components.
   SHA-256 `890ee723d23f4056d657fe38351f3818dcdce66532fb1ade535a40e15a453674`.
3. [`03-share.png`](./03-share.png) — 1600 × 1000. Widened public share with
   reasoning, command, file change, MCP call, plan, approval, delegated agent,
   notice, compaction, and context meter rendered through the shared dispatch.
   SHA-256 `00c74655c63e4bf79c6657bb054d2db2b3533bde0866e4095b4a24ebe2754100`.
4. [`04-khala-chat-sync.png`](./04-khala-chat-sync.png) — 1600 × 1000. Honest
   unauthenticated route state and credential boundary; no fake synced messages.
   SHA-256 `10dae027fb2220997f2e9bac5dc79182d1b305a163ac5f5a38b1e109f84c294b`.
5. [`05-desktop-real-full-auto.png`](./05-desktop-real-full-auto.png) — 1225 ×
   768. A running real Codex Full Auto task in OpenAgents Dev with reasoning
   and typed activity cards expanded. The live accessibility inspection also
   observed typed command and aggregate turn-diff/file rows; this committed
   frame was chosen because it contains no host filesystem paths. SHA-256
   `8125174264d5623672af85f5e9f7ba399f101284e3d80ec8f77983cefd624ae5`.

## Closeout gate

Do not close #8872 from this commit alone. At capture time, a separately owned
typography change (Inter default, Zalando accent) was still in progress against
the shared Desktop/web surfaces. After that owner change lands, rebase or
cherry-adapt this fix and receipt onto current `origin/main`, rerun the short
focused race suite, and refresh at least the affected Desktop/web images (or
prove their render hashes did not change). Then attach the refreshed directory
to #8872, close #8872, update #8857's lane ledger to show T1–T15 complete, and
close the epic with the Wave 0–4 commit/evidence ledger. Do not represent the
credential-gated Chat Sync capture as authenticated Sync.

The post-rebase full Desktop sweep also needs one quiet-host rerun. Under the
current concurrent-agent load it missed three existing wall-clock assertions
(React sheet mount, 500-row timeline under two seconds, and a 20 ms Codex
connect transition). Those exact three files passed together in isolation at
3 files / 66 tests. The earlier full proof-base run was green, but neither fact
is a substitute for the required refreshed full-suite receipt after typography
lands.
