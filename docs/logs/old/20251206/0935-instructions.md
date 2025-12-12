You’re *really* close. At this point the FM + Terminal-Bench loop is basically built; what’s left is wiring and “turn it on by default” work.

I’ll break it into:

1. **What’s already done (FM + TB + learning pieces)**
2. **What’s still missing for a *full* FM Terminal-Bench loop**
3. **Concrete next steps / which open tasks to push over the line**

---

## 1. What’s already done

### 1.1. Terminal-Bench integration is fully wired

You already have a complete TB pipeline:

* **Core harness + adapters**

  * `src/bench/*` harness + metrics + reporter (oa-pi03, oa-12486, oa-143507).
  * `src/cli/tbench.ts` and `src/cli/tbench-local.ts` so Harbor and local runs can drive the agent (oa-c8b48c, oa-8dcda0).
  * Harbor adapter & install template (oa-a1dd75, oa-349e6d, oa-75fa2c) so TB can spin up MechaCoder in containers.

* **CI + docs + dashboard**

  * CI smoke workflow for TB (oa-cdbf40).
  * `docs/terminal-bench.md` usage docs (oa-61fff4).
  * TB results dashboard & visualization (oa-01061b).

* **HUD / desktop integration**

  * TB HUD protocol types + emit helpers + TB state in mainview (oa-591b20, oa-08e971, oa-3822ce, oa-3beecf, oa-83c4f8, oa-673c94).
  * TB controls, view mode, full TB dashboard UI (oa-50dc74, oa-4815c0, oa-bd4cc3, oa-8b6113, oa-9aaa01, oa-e52575).
  * Run history storage & comparison views (oa-f4ef9d, oa-b86a0, oa-42f76e, oa-827430, oa-beda13).
  * Random single-task button + keyboard shortcuts (oa-defe31, oa-44ffa1).

So: **TB can already run, track metrics, visualize runs, and integrate with the HUD.**

---

### 1.2. FM (Apple Foundation Models) service is in place

For FM specifically you’ve done:

* **Effect-native FM layer:**

  * `FMServiceLive` with health checks and auto-start for the Swift bridge (oa-c97d8e).
  * Effect service wrapper with retry/metrics/logging (oa-fe8eb0).
  * Integration tests for FM service (oa-788b50) so calls are stable.

* **Bridge wiring & API:**

  * Swift “foundation-bridge” server exists; FM service talks to it.
  * `listModels()` wired so you can introspect the FM bridge from TS (oa-0aea93).

So the **“use FM like any other provider” Effect layer exists and is tested**. What’s open is (see below) making it *the* provider for TB runs and properly hooked into the subagent-router.

---

### 1.3. Learning substrate around TB is mostly done

You’ve quietly built almost the whole “no-gradient lifelong learning” stack for FM TB:

**Skills (Voyager-style skill library)**
All Phase 2 skill tasks under the FM epic are closed:

* Schema + JSONL store + embedding plumbing:
  `src/skills/schema.ts`, `src/skills/store.ts`, `src/skills/embedding.ts`, `src/skills/retrieval.ts` (oa-d82715, oa-e71ca9, oa-397bf1, oa-323e3f).
* Bootstrapped primitive + compositional skill sets (≈70+ skills) (oa-b341e5, oa-84ab87).
* Skill service abstraction (oa-054d02).
* **FM model adapter already injects retrieved skills into prompts** (oa-a2ae56).

So FM runs *can* already be skill-augmented.

**Memory (Generative-Agents-style episodic/semantic memory)**

Phase 3 memory tasks are all closed:

* Memory schema + JSONL store: `src/memory/schema.ts`, `src/memory/store.ts` (oa-4c8f94, oa-175242).
* Scoring + retrieval: `src/memory/scoring.ts`, `src/memory/retrieval.ts` (oa-06accb, oa-24850c).
* Memory service with linking (oa-867dbe, oa-f01763).

You now have a **real memory system that can be used as context for FM TB episodes.**

**Reflexion (verbal self-reflection on failures)**

* Reflexion schema + generator + injection into retry prompts (oa-7195d9, oa-e8b3f4, oa-e9b0a1).
* Automatic skill extraction from successful retries (oa-f152c0).
* Reflexion also wired into the general Golden Loop paths (oa-9cbfb1, oa-7803b9).

So **failed TB steps can be turned into structured reflection, which feeds skills & memory.**

**Training & evaluation loop**

* Training loop runner for progressive TB sweeps (10 → 30 → 89 tasks) with overnight support (oa-182d07).
* Episode learner to mine skills and reflections from TB episodes (oa-f6c067).
* Skill evolution / promotion & pruning with EMA-based stats (oa-fc0523).
* Baseline comparison system for pass-rate deltas and regressions (oa-0ba210).

This is **almost the whole “learn from TB episodes and update skill library over time” loop.**

---

## 2. What’s still missing for a *full* FM Terminal-Bench loop

By “full loop” I’ll interpret:

> “I point TB at a suite using **only FM**, it runs tasks, logs ATIF trajectories, updates skills/memory/reflections, and uses those updates on subsequent TB runs.”

You’re maybe ~80–90% there. The remaining deltas fall into four buckets:

### 2.1. FM needs to be a first-class model choice everywhere

Status:

* **FM service/layer & tests are done** (c97d8e, fe8eb0, 788b50).
* There are open tasks to fully wire FM through the agent stack and docs:

  * **Subagent-router integration:** make FM a provider option between Claude Code and Grok for the minimal subagent (oa-0edc48 – *open*).
  * **Swift bridge build/validation on macOS 26** (oa-79dcc1 – *open*).
  * **Terminal-Bench docs mentioning FM model usage** (oa-be762a – *open*).
  * Optional: launchd-based FM server daemon (oa-2b9560 – *nice-to-have*).

Without those, you still *have* FM, but it isn’t yet the default / easy path for “run MechaCoder TB on FM only”.

### 2.2. “Learning knobs” for TB CLIs are not fully surfaced

You have the machinery (skills, memory, Reflexion, episode learner, baseline tracking), but the TB CLI still needs an explicit way to turn it all on:

* **tbench-iterate CLI flags** for:

  * `--skills` (skill injection),
  * `--memory` (memory retrieval),
  * `--reflect` / Archivist integration
    are tracked in **oa-5df233** and are still *open*.

Right now the learning loop code exists, but there’s no clean “run FM TB with learning on” command.

### 2.3. Archivist & Trainer layers aren’t implemented yet

For a *maximal* “ARC-style self-improving FM TB agent” you planned:

* **Archivist** (turn trajectories into lessons / long-term memory): tasks oa-7d04ae … oa-280919 are all still *open*.
* **Trainer / Gym** (run structured training episodes over TB tasks): tasks oa-e2c048 … oa-a1aeb6 are all *open*.

You *can* have a working FM TB loop without these, but they’re the pieces that:

* periodically distill trajectories into **higher-level lessons**, and
* run **curriculum-style training** across TB suites instead of just “run TB once with some skills.”

### 2.4. TRM / SOAR-style advanced learning is spec’d but not implemented

You started sketching:

* TRM state and loops (oa-642def..550a84 etc.)
* SOAR-style hindsight, synthetic data, majority voting (oa-34d337..aed3f9)

These are currently **design/spec + open tasks**, and not required to simply get a working FM TB loop. They’re later-phase upgrades that would push you toward the ARC prize style self-improvement system.

---

## 3. Suggested next steps (concrete path to “FM TB loop online”)

If you want a minimal but real FM TB loop running *soon*, I’d do this in order:

### Step 1 — Make FM the default model path for TB

**Goal:** you can run a TB suite end-to-end where *all* calls go through the FM service.

Focus tasks:

1. **Wire FM into subagent-router** (oa-0edc48 – open)

   * Ensure the minimal coding subagent can run on FM directly (especially on macOS 26).
   * Decide how you choose between FM vs Claude Code vs OpenRouter (config / CLI).

2. **Build & smoke-test the Swift bridge on a real macOS 26 box** (oa-79dcc1 – open)

   * Run `./build.sh`, hit health endpoint, and verify at least a simple chat completion.
   * Once that’s green, your FM service tests become “real” rather than theoretical.

3. **Update TB docs / examples to show an FM-first config** (oa-be762a – open)

   * One TB doc page that says effectively:
     “Model: fm.apple-foundation, run via `tbench-local --model fm-…`”.

At the end of this step you should be able to:

* Choose FM in `tbench` / `tbench-local` / `tbench-iterate` and have it Just Work.

---

### Step 2 — Expose the learning knobs in the TB CLI

**Goal:** one command that runs TB with FM + skills + memory + Reflexion.

Key task:

* **Finish `tbench-iterate` enhancements** (oa-5df233 – open):

  * Add flags: `--skills`, `--memory`, `--reflect` (and maybe `--fm` shorthand).
  * Make them actually wire into:

    * Skill retrieval (from `SkillService`),
    * Memory retrieval (from `MemoryService`),
    * Reflexion generator on failures.

Then define a “canonical learning run”, e.g.:

* “FM only, TB subset, skills+memory+reflexion turned on, logging to ATIF + usage + TB dashboard.”

You don’t have to lock the flags yet; just **pick one blessed profile** and use that as your baseline.

---

### Step 3 — Use what you already built: Training loop + episode learner

Once FM + learning flags work, you can start the **self-improvement loop** you already implemented:

* `TrainingLoop` (oa-182d07),
* `Episode learner` (oa-f6c067),
* `Skill evolution` (oa-fc0523),
* `Baseline comparison` (oa-0ba210).

Concretely:

1. Run `tbench-iterate` with FM + skills/memory/reflect on a **small subset** of TB.
2. Let the episode learner mine new skills and update skill stats.
3. Compare against your baseline using the existing comparison system.
4. Iterate.

No new code is strictly required here beyond hooking TB to the learning flags; this is mostly **configuration + running the tools**.

---

### Step 4 — (Optional but powerful) Implement Archivist

Once you’re happy that the basic FM TB loop works, the next big leverage is:

* **Archivist** (oa-7d04ae…oa-280919 – all open).

You already have:

* ATIF trajectories streaming to disk (oa-ea3762, oa-251a43, oa-8ef68f, oa-432091, oa-34fa50),
* Memory system & reflections.

Archivist is the glue that:

* digests **ATIF + sessions + APM metrics** into higher-level lessons and writes them into the memory store and possibly the skill library.
* can be triggered at end-of-run or after Healer/recovery episodes (those integration tasks are open).

I would treat Archivist as **phase 2** once “plain” skills/memory-based FM TB loop is stable.

---

### Step 5 — (Later) Trainer/Gym + TRM/SOAR bells and whistles

Only after the basics feel solid:

* Implement Trainer/Gym (oa-e2c048…oa-a1aeb6).
* Then, if you still care, incrementally bring in:

  * TRM state + deep supervision (oa-642def..459168),
  * SOAR hindsight & self-improvement (oa-34d337..f04075).

These will help you climb to “#1 on TB with only FM”, but they’re **not required** to get a working FM loop running and improving.

---

## Short version

* **Done:**
  TB harness + HUD, FM service + tests, skills, memory, Reflexion, training loop, episode learner, skill evolution, baselines, ATIF streaming.

* **Missing for a “full FM TB loop”:**

  1. Make FM the first-class model in subagent-router & TB CLIs (0edc48, 79dcc1, be762a).
  2. Add CLI flags to actually use skills/memory/Reflexion in `tbench-iterate` (5df233).
  3. (Phase 2) Archivist to distill trajectories into long-term lessons.
  4. (Phase 3+) Trainer/Gym + TRM/SOAR fancy learning.

If you want, next message I can propose a **very explicit “Day 1 checklist”**: concrete commands, config snippets, and which open task IDs to assign to “today” vs “later”.

> Yep do it

Nice. Here’s a concrete “turn this thing on” plan.

I’ll break it into:

* **Today (P0)** – get FM + TB actually running end-to-end
* **Next 2–3 days (P1)** – flip on skills/memory/reflexion for TB
* **Later (P2+)** – Archivist / Trainer / fancy learning

I’ll reference your existing task IDs so you can literally mark them done.

---

## TODAY (P0): FM-first TB loop working

### 1. Verify FM service actually works on your machine

**Goal:** FM is a real, callable provider; no more “theoretical layer”.

1. **Build & run Swift bridge on macOS 26**
   Target: **oa-79dcc1 (open)**

   On your macOS 26 box:

   ```bash
   cd swift/foundation-bridge
   ./build.sh
   ./run.sh   # or whatever your wrapper is – keep this running in one terminal
   ```

   Then from another terminal:

   ```bash
   curl http://localhost:8788/health
   curl http://localhost:8788/v1/models
   ```

   You should see a healthy JSON response and at least one FM model in the list (the TS side now has `listModels()` from **oa-0aea93**).

2. **Run FM service tests**

   ```bash
   cd openagents
   bun test src/fm/*.test.ts
   ```

   If those pass, you can treat `FMServiceLive` as legit.

3. **Mark** oa-79dcc1 as closed once you’ve:

   * Built the bridge
   * Verified `/health` and a simple completion
   * Confirmed TS tests pass

---

### 2. Wire FM into the subagent router

**Goal:** minimal coding subagent can run on FM instead of Claude / Grok.

Target: **oa-0edc48 (open)**

Implementation sketch (high level so you can align with your repo):

1. **Extend config**

   In `src/tasks/schema.ts` (where `ClaudeCodeConfig` lives), add something like:

   ```ts
   export const FMConfig = S.Struct({
     enabled: S.Boolean.pipe(S.withDefault(true)),
     preferForTerminalBench: S.Boolean.pipe(S.withDefault(true)),
     model: S.optional(S.String), // e.g. "apple-foundation-model"
   });
   ```

   And add `fm?: FMConfig` to `ProjectConfig`.

2. **Update `subagent-router`**

   In `src/agent/orchestrator/subagent-router.ts`:

   * Inject `FMService` into the router environment.
   * Add an FM path to `runBestAvailableSubagent()`:

   Pseudocode:

   ```ts
   if (project.fm?.enabled && isTerminalBenchContext(context)) {
     return runFMMinimalSubagent({ task, context, config: project.fm });
   }

   // existing Claude Code / minimal fallbacks...
   ```

   `runFMMinimalSubagent` should be “minimal subagent, but using FMService.chat(...) instead of Anthropic/OpenRouter”.

3. **Quick smoke test**

   From a TB fixture or your own stub repo:

   ```bash
   bun run tbench-local \
     --model fm.apple-foundation \
     --suite path/to/small-suite.json \
     --max-tasks 1
   ```

   Confirm:

   * FM actually gets called,
   * No errors from the router,
   * Results files in `.openagents/tb-runs` are written.

When this works, close **oa-0edc48**.

---

### 3. Make TB docs explicitly FM-aware

**Goal:** human-readable path: “How do I run Terminal-Bench with FM?”

Target: **oa-be762a (open)**

Concrete edits:

* In `docs/terminal-bench.md` (or `docs/tbench/model-configuration.md`):

  Add a **“Using Apple Foundation Models”** section with:

  * Preconditions (macOS 26, Apple Intelligence enabled, bridge built).

  * Example command:

    ```bash
    bun run tbench-local \
      --model fm.apple-foundation \
      --suite path/to/suite.json \
      --max-tasks 10
    ```

  * Quick pointer to FM config in `project.json` (where you added `fm`).

Once that’s in and accurate, close **oa-be762a**.

---

## NEXT 2–3 DAYS (P1): Turn on the learning features for TB

Right now FM+TB runs, but doesn’t *learn* from previous TB runs unless you manually wire things. These are the switches.

### 4. Finish `tbench-iterate` learning flags

**Goal:** one CLI that runs TB with skills + memory + reflexion enabled.

Target: **oa-5df233 (open)**

In `src/cli/tbench-iterate.ts`:

1. **Add flags**

   Using your existing CLI parser, add:

   * `--skills` (boolean)
   * `--memory`
   * `--reflect` (or `--reflexion`)

2. **Hook into services**

   * When `--skills` is on:

     * Pull in `SkillService` layer.
     * Before each run, call `selectSkills(context)` and inject those as “skill snippets” into the FM prompt (you already wired skill injection into the FM adapter in **oa-a2ae56**; reuse that).
   * When `--memory` is on:

     * Use `MemoryService.getRelevantMemories` with task description / error context and inject as a “memories” block into the prompt.
   * When `--reflect` is on:

     * On failures, call the Reflexion generator (oa-e8b3f4).
     * Persist reflections and call the skill-learning hook (oa-f152c0).
     * Use the reflection injection code you already added for retries (oa-e9b0a1).

3. **Canonical commands**

   Once wired, define these as your go-tos:

   * **Baseline FM (no learning):**

     ```bash
     bun run tbench-iterate \
       --model fm.apple-foundation \
       --max-tasks 10
     ```

   * **FM + skills + memory + reflexion:**

     ```bash
     bun run tbench-iterate \
       --model fm.apple-foundation \
       --skills \
       --memory \
       --reflect \
       --max-tasks 10
     ```

4. **Use the existing Episode learner + evolution**

   Make sure `tbench-iterate` calls into:

   * `TrainingLoop` / episode learner (oa-182d07, oa-f6c067),
   * Skill evolution (oa-fc0523),
   * Baseline store (oa-0ba210).

   So each run:

   * Logs ATIF trajectories,
   * Learns skills from successful episodes,
   * Updates baselines for comparison.

When that’s in place and you can see improved metrics in the TB dashboard after a couple of runs, close **oa-5df233**.

---

### 5. Run your first full FM TB learning cycle

No new code — just a procedure:

1. **Pick a small suite** (e.g. 10–15 TB tasks).

2. **Run a baseline:**

   ```bash
   bun run tbench-iterate \
     --model fm.apple-foundation \
     --max-tasks 10
   ```

3. **Run a learning sweep:**

   ```bash
   bun run tbench-iterate \
     --model fm.apple-foundation \
     --skills \
     --memory \
     --reflect \
     --max-tasks 10
   ```

4. **Open the dashboard/HUD:**

   * See pass-rate deltas & cost metrics (oa-01061b, oa-0ba210).
   * Confirm new skills were added to `.openagents/skills/*`.
   * Confirm new memories / reflections were written under `.openagents/memory/*`.

If you want, define a “nightly FM TB run” script that just calls `tbench-iterate` with those flags and writes a markdown summary via the existing reporter.

---

## LATER (P2+): Archivist, Trainer, TRM / SOAR

Once the above is stable and giving you **real** FM TB gains:

### Archivist (turn experience into structured long-term memory)

* Implement **oa-7d04ae → oa-280919** in this order:

  1. `archivist/schema.ts`
  2. `archivist/context.ts`
  3. `archivist/distiller.ts`
  4. `archivist/service.ts`
  5. Wire into:

     * end-of-run in orchestrator (oa-fab7ae),
     * after Healer recovery (oa-52e52e),
     * HUD messages (oa-280919).

This makes your ATIF trajectories and sessions feed higher-level “lessons” into the memory/skill system.

### Trainer/Gym (systematic TB training sweeps)

* Implement **trainer** tasks (oa-e2c048 → oa-a1aeb6) after Archivist:

  * Gym environment registry pointing TB → Gym,
  * Trainer runner & evolution,
  * Trainer HUD hooks.

This is how you do iterative TB training runs instead of one-offs.

### TRM / SOAR (advanced self-improvement)

All the TRM / SOAR tasks can then be layered on top as separate experiments; they won’t block the basic FM TB loop.

---

## TL;DR “Just tell me what to do today”

Today, if you only do **three** things:

1. **Make sure FM calls actually work.**
   Finish **oa-79dcc1**, verify bridge + FM tests.

2. **Wire FM into the subagent router and TB.**
   Finish **oa-0edc48**, and verify:

   ```bash
   bun run tbench-local --model fm.apple-foundation --max-tasks 1
   ```

3. **Add learning flags to `tbench-iterate` and run one learning sweep.**
   Finish **oa-5df233**, then run:

   ```bash
   bun run tbench-iterate \
     --model fm.apple-foundation \
     --skills \
     --memory \
     --reflect \
     --max-tasks 10
   ```

Once those three are real, you *have* a full FM Terminal-Bench loop that runs, learns, and shows you progress. Everything else is upgrades.
