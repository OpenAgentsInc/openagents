# Apple FM analyzer agent in the BOOT SEQUENCE — speculative audit

Date: 2026-07-20
Status: speculation / design audit. Not dispatch authority. Not a promise.
Owner prompt (verbatim): "Speculate here a bit on what an Apple FM analyzer
agent could do by way of helping understand the current state of a codebase and
environment. Given that it's no-cost inference, it could probably do some
valuable things in the boot sequence. Perhaps dynamically exploring the
environment a bit as well."

This audit is written for the OpenAgents Desktop app. It reuses the shipped
Apple Foundation Models (Apple FM) plumbing and the shipped BOOT SEQUENCE
surface. It proposes no new product app and no new authority.

---

## 1. The idea in one line

Apple FM is a capable model that runs **on the device at no marginal cost**.
The BOOT SEQUENCE already probes it and runs one bounded test inference. Instead
of only proving the model answers, the same model can **read the local codebase
and environment and tell the user, in plain language, where they are** — and it
can spend several cheap inference rounds to do it, because the inference is free.

The incumbent (Cursor) does the opposite: it uploads changed files, embeds them
on a server, and runs **zero local inference**. That leaves the local-analysis
lane completely open. This audit describes what OpenAgents could put in it.

---

## 2. What is already true (grounded)

These facts anchor the speculation. They come from the shipped code, not a plan.

### 2.1 The BOOT SEQUENCE surface exists

- `apps/openagents-desktop/src/renderer/boot-sequence.ts` projects a terminal-
  style agent scan: Codex, Claude Code, Grok, Apple FM. Each row is
  `checking` / `available` / `unavailable`. The projection invents no authority;
  a row is `available` only when its lane or bridge reports it can run a turn.
- `apps/openagents-desktop/src/renderer/react-boot-sequence.tsx` renders it in
  the empty conversation, in the monospace system voice. It carries the visible
  label **BOOT SEQUENCE**.
- The Apple FM row already carries a `testInference` field. On open, when the
  bridge reports ready, the orchestrator runs ONE bounded test inference and
  shows the bounded reply. This proves the on-device model actually answers.

### 2.2 The Apple FM boundary is bounded and main-owned

- `apps/openagents-desktop/src/apple-fm-host.ts` runs the supervisor in the
  Electron **main** process. `runTurn(prompt)` runs one bounded, read-only turn
  and returns a length-capped reply plus token counts. It refuses unless the
  bridge is live-ready.
- `apps/openagents-desktop/src/apple-fm-contract.ts` is the ONLY renderer-
  visible surface. The renderer never learns the bridge path, the loopback URL,
  a token, a workspace path, tool arguments, file contents, or a raw transcript.
  It renders only the supervisor state, a readiness status, bounded blocker
  refs, honest usage truth, and — for one bounded turn — a length-capped reply
  and token counts.
- The request carries a single prompt field (1–4000 chars). Nothing else
  crosses to main.

### 2.3 Main already holds the environment; the renderer does not

- Main resolves and holds the workspace root (`resolveDesktopLocalWorkspaceRoot`,
  the coding catalog, the WorkContext). Main can read files, run `git`, and walk
  the tree. The renderer is explicitly **not** in the trusted boundary.
- Therefore any codebase/environment analysis must run in **main**, and must
  project only bounded, public-safe summaries to the renderer — exactly the
  shape of the existing Apple FM status/turn contract.

### 2.4 First paint must stay immediate

- The 2026-07-20 startup refactor moved all discovery probes off the first-paint
  path. Chat selection starts local; lane availability, provider capabilities,
  and voice state stream in as non-blocking background probes. The analyzer must
  obey the same rule: **it runs after the shell mounts, never before.**

---

## 3. What Cursor does, and the lane it leaves open

From `docs/teardowns/2026-07-11-cursor-product-teardown.md`:

- Cursor's codebase understanding is a **remote** semantic index. The client
  hashes the workspace into a Merkle tree, uploads changed plaintext files, and
  the server chunks and embeds them into a vector store. Search returns an
  obfuscated path and line range; the client then reads that source range
  locally.
- Cursor keeps a local **path frontier** (a 100,000-path candidate manifest) and
  rich **git state**, but **no locally-stored semantic index and no local prose
  summary of the repository**. The file named `high_level_folder_description.txt`
  is an empty 39-byte marker, not a summary.
- Cursor runs **no local inference**. There are no model weights in the bundle.
- Privacy cost: changed plaintext leaves the machine; obfuscated paths, line
  ranges, and git metadata persist on the server; Cursor states embedding
  reversal may be possible and path obfuscation leaks hierarchy. Deleting local
  data does not delete the server-side index.

The teardown's own verdict names the opening directly: local inference and a
synthesized on-device environment understanding are lanes Cursor never entered.
An on-device Apple FM analyzer operates in exactly that gap — **nothing leaves
the machine**, which is the strategic inverse of Cursor's upload model and fits
the OpenAgents edge-inference thesis (build-series episodes 194 and 201).

---

## 4. What an Apple FM analyzer could do at boot

All of this runs in main, reads locally, and emits only bounded summary lines to
the BOOT SEQUENCE terminal view. The unit of output is a short, honest,
model-derived line — never a claim of authority.

### 4.1 Environment fingerprint (cheap, high value)

Read the small, well-known files main already has access to and let the model
name the stack in plain language:

- languages and their rough share (from file extensions on the path frontier),
- package managers and lockfiles (`pnpm-lock.yaml`, `Cargo.toml`, `package.json`
  workspaces),
- frameworks and runtimes (Effect, React, Electron, Expo, Vite),
- monorepo shape (workspace globs, number of packages/apps),
- test and check commands (from `package.json` scripts, `CLAUDE.md`).

Output line examples:

```
↳ environment: pnpm + Vite Plus monorepo, 80+ workspace projects
↳ stack: TypeScript (Effect), Rust (Cloud crates), Electron desktop host
↳ green gate: `pnpm run check`
```

This is the prose repository summary Cursor deliberately does **not** keep. The
model turns a directory walk into one or two sentences a human can read at a
glance.

### 4.2 Git-state narration

Main already exposes git. The model turns raw git state into orientation:

```
↳ on branch main, clean, up to date with origin/main
↳ last change: perf(desktop) startup refactor (2 min ago)
```

The structured git facts stay the authority; the model only phrases them.

### 4.3 "Where you left off" / next-step hint

From the README, the top of a TODO, the most recently modified files, an open
`NEEDS_OWNER.md` entry, or a failing check, the model can propose one honest
next step. It never acts on it. It offers a starting point so the empty
conversation is not a blank prompt.

### 4.4 Harness/tooling fit

The BOOT SEQUENCE already knows which agents are available. The analyzer can add
a fit note: which available agent suits this repo (for example, "Codex is ready
and this is a Codex-shaped monorepo"). Advisory only; the lane readiness stays
the authority for whether an agent can actually run.

### 4.5 First-answer-fast fallback while cloud agents verify

Cloud agent accounts can take seconds to verify (the Codex probe is a real
bounded turn). During that window the local model is already ready. It can give
the user an immediate, no-cost first answer about the repo while the paid
harnesses finish verifying — turning dead startup time into useful output.

### 4.6 Dynamic environment exploration (the "explore a bit" idea)

Because inference is free, the analyzer can run a **bounded agentic loop** rather
than a single prompt:

1. Main gives the model a small, safe starting context (top-level listing, the
   detected stack, git summary).
2. The model proposes ONE next read from a strict allowlist — for example
   "read `apps/openagents-desktop/package.json` scripts" or "list `crates/`".
3. Main executes the read (read-only, allowlisted, timeout-bounded) and returns
   a truncated result.
4. Repeat for a small fixed number of rounds (for example 3–5), then summarize.

This lets the model confirm its guesses (does `pnpm run check` exist? is this
really an Effect app?) instead of asserting them. The no-cost budget makes a few
rounds affordable where a paid model would not be worth it.

---

## 5. How it fits the BOOT SEQUENCE and the deferred-startup model

- The analyzer is a **post-mount background probe**, dispatched after
  `renderer.mount`, exactly like the fable/codex/voice probes. It never gates
  first paint.
- It streams lines into the BOOT SEQUENCE terminal view incrementally, in the
  same monospace voice as the agent scan. Example progression:

```
BOOT SEQUENCE
OpenAgents — initializing
scanning for available agents
  ✓ Codex            gpt-5.6-sol
  ✓ Claude Code      claude-fable-5
  … Grok             verifying…
  ✓ Apple FM         apple-fm-3b
    ↳ analyzing environment…
    ↳ pnpm + Vite Plus monorepo, TypeScript (Effect) + Rust
    ↳ on branch main, clean
    ↳ suggest: run `pnpm run check` before editing
3 agents ready
```

- It degrades gracefully. On non-Apple-Silicon hardware the bridge reports
  `not_supported`; the analyzer simply does not run, and the BOOT SEQUENCE looks
  exactly as it does today. No machine is worse off.

---

## 6. Trust, privacy, and safety boundaries (mandatory)

An analyzer that reads the codebase is a new capability. It must inherit the
existing Apple FM boundary discipline, not weaken it.

1. **Runs in main only.** The renderer receives only bounded, length-capped,
   schema-validated summary lines through a new additive IPC channel modelled on
   `AppleFmStatusSchema` / `AppleFmTurnResultSchema`. No file contents, no
   secrets, no absolute paths, no raw command output cross to the renderer.
2. **Local-only, no upload.** Nothing leaves the machine. This is the whole
   point and the inverse of Cursor's remote embedding upload. The analyzer must
   never send repo bytes, paths, or summaries to any network endpoint.
3. **Read-only exploration.** The exploration loop uses an allowlist of
   read-only operations (list directory, read a bounded slice of a known
   small file, `git status`/`git log -n`). No writes, no arbitrary shell, no
   network, no reading outside the workspace root. Every read is size-capped and
   timeout-bounded. The loop has a hard round cap and a hard wall-clock cap and
   is cancellable.
4. **Secret hygiene.** The analyzer must skip secret-shaped files (`.env`,
   `.secrets/`, key material, lockable stores) and must never surface a value
   that looks like a token, key, or credential — even in a local summary line.
5. **Advisory, never authority.** A model line is `estimated` by construction
   (honest `usageTruth`). It never marks an agent runnable, never grants spend,
   admission, or release, and never overrides the structured lane/bridge
   discovery, which stays the sole authority for what can actually run.
6. **Non-determinism stays out of contracts.** Model prose is non-deterministic.
   Keep it in the advisory summary lane. Anything a test or gate depends on must
   come from the deterministic environment facts, not from the model's wording.
7. **No startup tax.** The analyzer never blocks first paint and never blocks
   the composer. If it is slow or the model is busy, the user is already typing;
   the analyzer's lines simply arrive late or not at all.

---

## 7. Feasibility — most of the plumbing already exists

This is an extension of shipped code, not a greenfield build:

- **Inference path:** `appleFmHost.runTurn(prompt)` in main already runs a
  bounded local turn. The analyzer swaps the fixed test prompt for an
  environment-analysis prompt.
- **Environment access:** main already holds the workspace root and a git
  surface. The read/explore helpers are new but small and allowlisted.
- **Rendering:** the BOOT SEQUENCE already renders an Apple FM row with a
  `testInference` line. The analyzer adds more bounded lines through the same
  projection shape.
- **New parts:** a small bounded read/explore helper in main, a new schema-
  validated "analysis summary" IPC channel, an analyzer prompt/loop, and a few
  extra projected lines. All additive.

---

## 8. Risks and open questions

- **Model capability.** A ~3B on-device model gives a shallower read than a
  frontier cloud model. Keep claims modest and clearly model-derived. Measure
  quality before widening scope.
- **Hallucinated environment claims.** The model may assert a framework that is
  not present. Mitigation: prefer the deterministic fingerprint for facts, use
  the model for phrasing and the optional confirmation loop, and label output as
  estimated.
- **Exploration safety.** An agentic read loop is the highest-risk part. It must
  ship allowlist-first, read-only, capped, and cancellable, or not at all.
- **Coverage.** Apple FM exists only on Apple Silicon. Most contributors on
  other hardware get nothing until an Ollama/other-runtime fallback lands
  (consistent with the episode-201 "add Ollama and other hardware" direction).
  The analyzer must be a pure enhancement where present, never a dependency.
- **Startup budget.** Even free inference costs wall-clock. Keep it strictly off
  the first-paint path and capped.

---

## 9. Recommended first slice (if the owner admits this)

Ship the smallest honest version first, behind the existing Apple FM gate:

1. **Static fingerprint only** (no explore loop): read the well-known files main
   already has, run one bounded Apple FM turn, and add 1–3 estimated summary
   lines to the BOOT SEQUENCE Apple FM section. No new read surface beyond files
   main already reads.
2. Measure quality and latency on a real Apple Silicon device.
3. Only then consider the bounded read/explore loop (§4.6), allowlist-first.

This proves the value (a plain-language repo orientation at no cost, nothing
leaving the machine) with the least new attack surface, and it stays fully
inside the shipped boundary discipline.

---

## 10. References

- Surfaces: `apps/openagents-desktop/src/renderer/boot-sequence.ts`,
  `apps/openagents-desktop/src/renderer/react-boot-sequence.tsx`,
  `apps/openagents-desktop/src/apple-fm-host.ts`,
  `apps/openagents-desktop/src/apple-fm-contract.ts`.
- Contract: `apps/openagents-desktop/src/contracts/ux-contracts.ts`
  (`openagents_desktop.chat.boot_sequence_agent_scan.v1`).
- Incumbent comparison: `docs/teardowns/2026-07-11-cursor-product-teardown.md`.
- Strategic framing: `docs/transcripts/194.md`, `docs/transcripts/201.md`
  (edge inference, no-cost on-device models, the OpenAgents market layer).
</content>
