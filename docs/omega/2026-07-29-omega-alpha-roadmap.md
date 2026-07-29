# Omega alpha roadmap — one surface, three modes

- **Date:** 2026-07-29
- **Subject repositories:** `OpenAgentsInc/omega` at `86d3703562`,
  `OpenAgentsInc/openagents` at `1f9356ac6e`
- **Document type:** consolidated implementation roadmap. It changes no runtime
  authority, allocates no delta number, admits no public claim, and promotes no
  build. `docs/sol/MASTER_ROADMAP.md` keeps cross-program priority.
- **Consolidates:** the
  [zero-base single-experience plan](./2026-07-29-omega-zero-base-single-experience-plan.md)
  and the
  [Episode 263 alpha release gap analysis](./2026-07-29-episode-263-alpha-release-gap-analysis.md),
  against the canonical [Episode 263 transcript](../transcripts/263.md).
- **Relationship to prior ledgers:** where this roadmap's ordering conflicts
  with the packet order in [ROADMAP.md](./ROADMAP.md) (revision 5, 2026-07-24)
  or the [Omega Agent roadmap](./2026-07-25-omega-agent-roadmap.md), the newer
  2026-07-29 owner directions recorded here govern.

## 1. The two directions, reconciled

Two owner directions landed on 2026-07-29, and read naively they collide.

The **single-experience direction** says: remove the legacy Zed editor around
Zero Base, delete the mode flag, fix the buttons that do nothing, and ship one
experience with a bunch of stuff removed.

The **Episode 263 direction** says: the first alpha lets a person command all
of the top coding agents — Codex, Claude Code, Grok Build, and any ACP agent
directly; or command Omega Agent, which routes; or command Sarah by voice.
Today the shipped surface deliberately clamps every new conversation to the
native Omega loop (`OMEGA-DELTA-0149`, `0150`, `0131`), so the gap analysis
correctly rates the release **no-go**.

The reconciliation is one sentence:

> **One experience is not one agent. Omega ships one surface with no launch
> modes, and three conversation modes inside it.**

Concretely:

- The **shell** is singular. There is one launch shape, one init path, one
  render path, and no `--full-editor` second product. The single-experience
  plan stands.
- The **conversation** is plural. Direct Agent, Omega Agent, and Sarah are
  properties of a conversation, chosen at creation, disclosed on the surface,
  and never swapped underneath an existing transcript. The Episode 263 claims
  stand.
- The gap analysis's hedge — keep Zero Base as an optional focus or
  diagnostic launch mode — is resolved **against** launch modes. The repaired
  Zero Base shell *is* the application; it does not need a second name or a
  survival flag.
- The one-agent clamp was a truthful-labeling repair (`0131`, the selector
  that lied), not a product-shape law. Its supersession restores choice while
  keeping the law it protected: every surface names the executor that is
  actually doing the work.
- Vim stays (owner decision, 2026-07-29, recorded in the single-experience
  plan).

The two programs also share a root cause and therefore share their first
work: the broken buttons the single-experience plan inventories (C1–C9) are
the same defects the gap analysis lists as release blockers — the Sarah entry
that dispatches a refused action (C2 = EP263-05's broken entry), the ACP
registry that opens invisibly (C3 = EP263-03's registry), and the workbench
surfaces whose panels were never loaded (C4/C5 = the §5.2 work-surface
requirement). Repairing the drawn surface is not a prelude to the alpha. It
is the alpha's first half.

## 2. The end state

A person downloads Omega from `openagents.com`, launches it with no flags, and
sees one surface: a thread, a composer, a sidebar with recent threads and
tester channels, and a workbench rail whose every drawn control works. The
`+` control offers three ways to start a conversation — a direct agent
(Codex, Claude Code, Grok Build, or any ACP agent), Omega Agent with routing
disclosed, or Sarah by voice with price and limits shown before the
microphone opens. Several conversations run concurrently on different
executors without colliding. Every turn names its executor. Every visible
control has an admitted action, a loaded dependency, and a visible result.
There is no other Omega to fall back to, and after the post-alpha waves there
is no legacy editor left in the binary.

## 3. The waves

Ordering rule: the application gets less broken at every step, the alpha cut
line sits where testers stop being able to tell the difference, and deletion
work that testers cannot see runs after the alpha, not before it.

### Wave 0 — record the decisions (blocks everything)

The delta registry currently *enforces* the one-agent clamp and the
hides-not-deletes contract, so implementation cannot start until the policy
records move. One packet: supersede `0131`/`0149`/`0150` (executor selection
returns, with truthful labeling retained), set the single-experience direction
on `0048`/`0052`/`0053` (subtraction becomes the product; the flag is
scheduled for removal), record vim retention, and state the one-surface,
three-modes launch shape in `PRODUCT.md`-adjacent product docs. Acceptance: a
code search and product-doc review find one answer to what a normal launch
exposes, and `cargo test -p omega_deltas` is green with the amended checks.

### Wave 1 — make the drawn surface work (single-experience Phase 0/1)

| Packet | Source | Content |
| --- | --- | --- |
| Load the workbench panels | SE Phase 0.1, C4/C5 | Load and add `ProjectPanel`, `GitPanel`, `TerminalPanel` in the one init path, exactly as `test_support` already proves; converge the test harness with shipped init. Fixes the Files/Git/Terminal rail and directory reveal. |
| Reveal before open | SE Phase 0.2, C3/C7/C8/C9 | One helper that calls `reveal_zero_base_center` before every centre-pane open reachable from the surface. Kills the invisible-success class. |
| Admit vim | SE Phase 0.5, C6 | Admit the vim action set so `vim_mode: true` works in the composer and the revealed pane; find the mode indicator a home in the composer bar. |
| One honest menu bar | SE Phase 1, C1 | Replace `app_menus` with the admitted-set menu (App/Edit/View/Thread/Window/Help); fix or remove the refused `+`-menu entries. Roughly ninety dead menu items disappear. |

Exit test: a scripted session over every drawn control produces **zero
refusal lines in the log**. The action gate's output flips from designed
outcome to defect signal.

### Wave 2 — the three modes (EP263-02 … EP263-05, EP263-09)

| Packet | Source | Content |
| --- | --- | --- |
| Three-mode front door | EP263-02 | Persistent Direct Agent / Omega Agent / Sarah entry points; mode, executor, project, and readiness visible before send; four honest row states (ready, setup required, temporarily unavailable, not supported); an existing conversation never changes executors. |
| Direct ACP agents | EP263-03 | Enable the Codex, Claude Code, and Grok rows; make Add More Agents open a working, visible ACP registry (depends on Wave 1 reveal); keep each harness's own auth and config; executor-specific thread restore; prove one generic ACP agent beyond the named three. |
| Omega Agent routes for real | EP263-04 | Deterministic routing policy over task requirements and live executor readiness; attach every executor the release claims; visible route disclosure with override; journaled decisions; visible failure when the chosen executor disappears. |
| Sarah voice admission | EP263-05 | Replace the broken panel action with the real voice-mode entry (depends on Wave 1); define the alpha cohort and identity path; show price, hold, session limit, confirmations, and capability bounds in the UI before the microphone opens; reconcile the "personal lieutenant" copy with the bounded v1 command set. |
| Concurrent supervision | EP263-09 | Per-thread running/waiting/failed/completed/cancelled state; two write-capable agents never silently collide in one worktree; queued input, cancellation, and restart recovery per concurrent agent. This is what makes "command all of the top coding agents simultaneously" true as concurrent threads. |

### Wave 3 — alpha operations and the gate

| Packet | Source | Content |
| --- | --- | --- |
| Tester channels | EP263-06 | A versioned channel registry (or one honestly singular tester room), visible on first launch, read and send proven from an installed candidate, moderation and outage behavior included. |
| Omega download truth | EP263-07 | An Omega product identity in the signed release resolver on `openagents.com`; never relabel the Electron Desktop artifact; version, platform, digest, signature, and limitations on the page. This packet lands in the `openagents` repository. |
| Installed UX and failure pass | EP263-10 + SE §2.5 | First launch, no folder, missing binary, expired auth, adapter crash, offline service, exhausted credit, relay outage, narrow window, keyboard-only, screen reader, reduced motion. The law: a visible control has an admitted action, a loaded dependency, and a visible result — no enabled-looking no-op survives. |
| Sealed baseline and gate harness | SE Phase 2 prereq + gap §7 | Land the sealed visual baseline (the shipped surface currently has zero automated render coverage) and script the installed-candidate matrix so the gate is repeatable. |
| Feedback operations | EP263-11 | Triage ownership, severity ladder, response expectations, log-attachment guidance, and a release-candidate identifier bound to every report. |
| Cut and verify the candidate | EP263-01 + EP263-08 | Cut the candidate only after every ledger row is green; run the full installed matrix from the gap analysis §7; independent reviewer repeats the held-out journey; reconcile every spoken Episode 263 line against the exact candidate; record source, digest, signature, and known limits. |

**The alpha ships at the end of Wave 3.** The gate is the gap analysis's
installed-candidate matrix, unchanged, plus the Wave 1 zero-refusal exit test
rerun on the packaged candidate.

### Wave 4 — complete the single experience (post-alpha)

| Packet | Source | Content |
| --- | --- | --- |
| Remove the mode split | SE Phase 2 | Delete `--full-editor` and the editor-only arguments (`--diff`, `--dev-container`, `--demo-workroom`); collapse the five branch points; `is_active()` becomes constant; the identity-gate bootstrap ordering survives; retire the two full-editor visual baselines. |
| Delete the legacy editor set | SE Phase 3 | Roughly 54 crates and 222,000 lines in leaf order, vim retained; each commit strips its keymap bindings and updates `FORBIDDEN_KEYMAP_NAMESPACES` and `REMOVED_FILES`; brand-gate floors re-measured in the same commits; the keep-if catalog in the single-experience plan §6 governs the survivors (file finder, cursor position, ACP tools behind a dev flag, LSP visibility). |
| Invert the proofs | SE Phase 4 | The action gate becomes a tripwire; a refusal recorded during proof runs fails the run; per-surface drawn-implies-working delta checks; `PRODUCT.md` and `taxonomy.md` rewritten to the one-surface, three-mode product. |

Wave 4 is deliberately after the alpha: crate deletion is invisible to
testers, high-churn, and safest against a surface that the alpha gate has
already proven. Nothing in Wave 4 changes what a tester can do.

## 4. What the first alpha requires — the cut line

Required: Waves 0 through 3, in full. Not required: everything in Wave 4.
The alpha may ship with `--full-editor` still present and the legacy crates
still compiled, because neither is reachable from the advertised journey and
the gate's launch row already asserts no hidden `--full-editor` prerequisite.
What the alpha may **not** ship with: a refused control that looks enabled, a
mode row that lies about readiness, an executor label that names the wrong
agent, an invisible open, or a download page that points at the wrong
product.

## 5. Delta bookkeeping, consolidated

New entries (numbers allocated by the registry, not here): the three-mode
conversation contract (selection returns, truthful labeling stays); zero base
is the application (no launch flag — lands with Wave 4); drawn implies
working (per-surface checks); removed editor crates stay removed; the refusal
log is empty in proof runs. Amendments in supersession style: `0131`, `0149`,
`0150` (the clamp), `0048`, `0052`, `0053`, `0116` (the mode split), with the
owner's 2026-07-29 directions as the recorded reasons. The registry rule
holds throughout: entry, check, and test change together, and no check is
weakened merely to pass.

## 6. Suggested GitHub issues

Repository is `OpenAgentsInc/omega` unless marked otherwise. Titles follow
the repository's imperative convention. Each issue should cite this roadmap,
the relevant source packet, and carry its acceptance list from section 3.

### Wave 0

1. **Record the one-surface three-mode product decisions in the delta
   registry** — Supersede `OMEGA-DELTA-0131`/`0149`/`0150` so a person can
   again choose an executor at conversation creation while every surface
   keeps naming the executor that does the work. Set the single-experience
   direction on `0048`/`0052`/`0053` and record vim retention. Update the
   amended checks and tests in the same change. Acceptance: one documented
   answer to what a normal launch exposes; `cargo test -p omega_deltas`
   green.

### Wave 1

2. **Load the workbench panels in the default launch** — Extend the
   zero-base branch of `initialize_panels` to load and add `ProjectPanel`,
   `GitPanel`, and `TerminalPanel` (as `test_support` already proves works),
   and converge `test_support` with shipped init so the harness can never
   again prove a configuration production skips. Fixes the Files/Git/Terminal
   rail surfaces and transcript directory reveal.
3. **Reveal the centre pane before every editor open** — One shared helper
   that calls `reveal_zero_base_center` before `open_path` /
   `open_abs_path` / `add_item_to_active_pane` on every path reachable from
   the default surface (thread outline, skill chips, mention opens, registry
   view). An invisible successful open is defined as a defect.
4. **Admit the vim action set in the default surface** — `vim_mode: true`
   must work in the composer and the revealed editor pane instead of being
   silently refused (180+ refused vim actions observed in the rc log). Give
   `vim::ModeIndicator` a home in the composer bar.
5. **Ship one honest menu bar and `+` menu** — Replace `app_menus` with a
   menu built from the admitted set; every retained item works, every refused
   item is gone. Fix or remove the Sarah and Add More Agents entries' broken
   dispatches. Exit test for the wave: a scripted pass over every drawn
   control writes zero refusal lines to the log.

### Wave 2

6. **Add the three-mode new-conversation front door** — Persistent Direct
   Agent, Omega Agent, and Sarah entry points with mode, executor, project,
   and readiness visible before send; four honest readiness states; an
   existing conversation never changes executors underneath its transcript.
7. **Restore direct Codex, Claude Code, Grok, and ACP agents** — Enable the
   disabled executor rows, make Add More Agents open a functional visible
   ACP registry, keep each harness's own auth/config/billing identity,
   restore executor-specific threads across relaunch, and prove one generic
   ACP agent beyond the named three. No path silently lands on the native
   loop.
8. **Make Omega Agent select among real executors** — Deterministic routing
   over task requirements and live executor readiness; attach the claimed
   executors; visible, journaled route disclosure with a user override; two
   materially different fixtures route to two executor classes; failure is
   visible when the selected executor disappears.
9. **Admit Sarah voice for the alpha cohort** — Real voice-mode entry
   replacing the refused panel action; defined cohort and identity path;
   price, hold, session limit, remaining credit, confirmations, and exact
   capability bounds in the UI; reconcile the "personal lieutenant" copy with
   the bounded v1 command set; one installed non-owner account completes the
   full session lifecycle including reconnect and final charge.
10. **Supervise concurrent agents safely** — Per-thread lifecycle states,
    worktree collision prevention or explicit warning for two write-capable
    agents, queued input and cancellation per thread, restart recovery.
    Defines "simultaneously" as concurrent independent threads.

### Wave 3

11. **Ship tester channels in the sidebar** — Versioned channel registry or
    honestly singular tester room, visible on first launch, installed
    read/send proof with a second account, moderation and outage behavior.
12. **Publish Omega download identity on openagents.com**
    (`OpenAgentsInc/openagents`) — Omega as an explicit product in the signed
    release resolver; version, channel, platform, digest, signature, minimum
    OS, and limitations on the page; the Electron Desktop identity is never
    relabeled.
13. **Run the installed UX, accessibility, and failure pass** — The full
    matrix from EP263-10 (first launch through reduced motion), with the
    standing law that a visible control has an admitted action, a loaded
    dependency, and a visible result.
14. **Land the sealed visual baseline and the installed release-gate
    harness** — First automated render coverage of the shipped sealed
    surface; script the gap-analysis §7 matrix so the release gate is
    repeatable rather than heroic.
15. **Stand up alpha feedback operations** — Triage ownership, severity
    definitions, response expectations, privacy and log-attachment guidance,
    release-candidate identifier bound to each report.
16. **Cut and verify the Episode 263 alpha candidate** — Only after issues
    1–15 are green: cut the candidate, run the installed matrix, independent
    reviewer repeats the held-out journey, reconcile every spoken line, and
    record digest/signature/limits. This issue closes the Episode 263
    promise or narrows the copy — never the reverse.

### Wave 4 (post-alpha)

17. **Remove the full-editor mode split** — Delete `--full-editor` and the
    editor-only arguments, collapse the five branch points, make the mode
    constant, retire the full-editor baselines, preserve the identity-gate
    bootstrap ordering.
18. **Delete the legacy editor crate set** (epic, leaf-batch children) —
    Roughly 54 crates / 222k lines in reverse dependency order, vim retained;
    keymap strips, `FORBIDDEN_KEYMAP_NAMESPACES`, `REMOVED_FILES`, and
    brand-gate floor re-measurement travel in the same commits; the
    single-experience plan §6 keep-if catalog governs survivors.
19. **Invert the zero-base proofs** — Action gate becomes a tripwire; proof
    runs fail on any refusal line; per-surface drawn-implies-working delta
    checks; rewrite `PRODUCT.md` and `taxonomy.md` to the one-surface,
    three-mode product.

## 7. Dependencies at a glance

```text
1 (deltas)
└── 2,3,4,5 (Wave 1, parallel)
    ├── 6 (front door)  ── 7 (direct agents) ── 8 (router) ── 10 (concurrency)
    ├── 9 (Sarah entry; also needs 5)
    └── 13 (UX pass, after 6-11)
11 (channels) and 12 (download) run parallel to Wave 2
14 (baseline+harness) starts any time; must land before 16
16 (cut) requires 1-15
17 ── 18 ── 19 after the alpha ships; 14 must precede 17
```

## 8. What would change this roadmap

- If the owner directs shipping the alpha with fewer than three modes, cut
  the corresponding spoken line from the published Episode 263 materials
  rather than shipping a claim the gate cannot prove.
- If the Sarah cohort or pricing decision slips, EP263-05 narrows to an
  explicitly labeled owner-only preview and the copy changes with it.
- If Wave 4's crate removal surfaces a survivor the alpha depends on, the
  keep-if catalog in the single-experience plan §6 is the decision record to
  amend — with the owner, not silently.
