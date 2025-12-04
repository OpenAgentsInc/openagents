Right now you’re basically choosing **how isolated each MechaCoder run should be, and how many you can safely run in parallel**.

Let’s walk combo-by-combo in MechaCoder terms (orchestrator + subagents, `.openagents`, HUD, etc.).

I’ll assume:

* Orchestrator runs on the host.
* Each *agent run* works on some view of the repo and may be:

  * a **minimal subagent**, or
  * **Claude Code** (via the SDK), eventually mostly CC.
* Golden Loop = “task from `.openagents` → edits → tests/typecheck → commit/push → close task”.

---

## 1) Worktrees **+** Containers

> One worktree per task/agent, and each agent runs inside its own container (Docker/Apple Container) with that worktree mounted.

### Pros

**Git / correctness**

* Each agent has:

  * its own **branch** (`agent/{taskId}`),
  * its own **working directory** (`.worktrees/{taskId}`),
    so:
  * no staging/index conflicts,
  * easy merging/rollback per task,
  * Golden Loop commits are trivially attributable to `taskId`.
* Parallelism is natural: 10 agents → 10 worktrees → 10 branches.

**Security / isolation**

* Containers give you:

  * process + filesystem isolation from the host,
  * resource caps (memory/CPU),
  * future network egress controls.
* Worktrees + containers together:

  * Even if an LLM goes rogue, it’s constrained to “whatever is in this mounted worktree inside the VM”.
  * You can bind-mount only the worktree and maybe a limited cache, **not** your whole home directory.

**Orchestrator story**

* Orchestrator:

  * picks a task,
  * spawns worktree `agent/{taskId}`,
  * calls “run subagent in sandbox(worktreePath)” which:

    * spins a container with `/workspace` = worktree,
    * runs the CC or minimal subagent entrypoint.
* That matches your container abstraction **and** your worktree/parallel-runner plan nicely.

**Scaling / future-proof**

* Want to go from “nightly 3 agents” to “20+ small agents”?

  * Worktrees let you scale git safely.
  * Containers let you scale resource and security boundaries safely.
* If you ever move beyond macOS to Linux, this same pattern (worktrees + OCI) generalizes.

### Cons

**Complexity**

* You’ve now got **three layers** per agent:

  * orchestrator → worktree → container → subagent.
* More moving pieces:

  * container image build + versioning,
  * container runtime (Apple Container / Docker),
  * worktree lifecycle (create/cleanup/prune),
  * orchestrator glue.

**Resource usage**

* More overhead than worktrees alone:

  * Each container has its own VM or namespace stack.
  * Each worktree duplicates the working tree files.
* On a laptop, “10 worktrees + 10 containers” might be fine… but it’s definitely heavier than “1 main repo + 1 agent”.

**Dev ergonomics**

* Debug loop is more annoying:

  * To poke at a failing run, you have to:

    * jump into the right worktree directory,
    * possibly also run inside the container shell to repro environment.
* Logs and traces come from inside the container; you’ve partly solved that with run-logs, but it still adds cognitive load.

**Networking weirdness**

* With Apple Container today:

  * host↔container networking is still a bit early/awkward.
  * Running tests that assume certain local services or network state gets trickier.
* You may end up doing “LLM calls on host, code/tests in container” which complicates the mental model even more.

---

## 2) Worktrees **without** Containers

> One worktree per agent, everything runs on the host (Bun/Effect/Grok/Claude Code all directly in macOS).

### Pros

**Git / correctness (same as above)**

* Same benefits:

  * per-task branch,
  * isolated working directories,
  * painless merges & rollbacks.
* Parallelism is straightforward; each agent has its own index and branch.

**Simplicity**

* No container runtime, no images:

  * You run exactly what you run now, just in different directories/branches.
* Orchestrator is simpler:

  * pick task → `createWorktree(task)` → run subagent in that directory → do merge/PR.
* Debugging is a dream:

  * `cd .worktrees/oa-123456` and you’re looking at exactly what the agent saw.

**Performance**

* No VM/container startup cost.
* No extra I/O layers; everything is native FS.
* Great for fast iteration while you’re still designing orchestrator + Claude Code interplay.

**Fits your current stack**

* You already have:

  * `.openagents` tasks,
  * agent loop hardening,
  * HUD visualization,
  * orchestrator/subtask/progress infrastructure.
* Worktrees drop in cleanly as **“multi-repo in a single repo”** without any extra infra.

### Cons

**Security**

* This is the big one: all code runs with your user privileges.
* Even with:

  * “verify tests before commit”
  * run-logs for forensics
* …a compromised/buggy agent could:

  * read arbitrary files (SSH keys, other repos, secrets),
  * open outbound network connections,
  * rm -rf things in your home directory.

**Resource contension**

* If you run 10 agents:

  * They all share the same OS, same global process table, same everything.
  * No cgroup/VM isolation; one runaway process can starve the machine.
* You can mitigate a little (Bun timeouts, nice/priority tweaks), but it’s not as strong as container limits.

**Future multi-tenant story**

* If you ever want “MechaCoder as a service” or remote workers:

  * Running everything host-native is not acceptable.
  * You’ll eventually *have* to add a sandbox layer.

---

## 3) Containers **without** Worktrees

> A container per agent, but all agents share the same host repo (or a single container workspace).

This splits into two sub-patterns people often conflate:

1. **Single working tree, multiple containers sequentially**
   (only one container mutates the repo at a time).
2. **Single working tree, multiple containers concurrently**
   (yikes).

### Pros

**Security**

* You still get container safety:

  * process/filesystem isolation from host,
  * resource caps, etc.
* If you take the “one container at a time” route, it’s pretty safe and simple conceptually.

**Less disk use than worktrees**

* One working tree on host, N containers over it over time → no extra checkout copies.
* If you’re constrained on disk, this can matter.

**Simpler Git topology**

* You can choose to:

  * commit directly on `main` from each container run, or
  * use simple branches (`agent/{date}`) without managing N worktrees.

This is fine **if you don’t do parallel agents**.

### Cons

**Parallelism story is bad**

* With no worktrees:

  * concurrent agents all writing into the same checkout = instant conflict soup:

    * staging stomps,
    * typecheck/tests reading half-applied changes,
    * merges stepping on each other.
* You can serialize:

  * “only one container mutates the repo at a time, others wait in a queue”…
    but then you’ve mostly given up on real parallelism anyway.

**Suboptimal for your vision (N agents on N tasks)**

* Your goal: multiple agents working on independent tasks overnight.

  * The natural mapping is: **one branch / working dir per task**.
  * Containers without that branch separation means you now need a much more complex “apply patches back into main” flow.
* The worktree approach already solves that elegantly; containers-only re-invent the same problem but less safely.

**More brittle merges**

* Without worktrees, you’ll be tempted to do:

  * “run agent → commit on main from inside container”.
* If a second agent starts on an older view of the repo, its commit conflicts with the first one.

  * You end up with more rebasing/retrying logic plumbed into orchestrator.

---

## So which combo is best for MechaCoder?

If we tie this back to your **stated direction**:

> The reliability will come from Claude Code instead of the Grok OpenRouter stuff, so how close are we to having Claude Code doing most of the heavy lifting after being delegated to by the orchestrator?

You really have two axes:

* **Axis A – isolation / safety:**

  * none → worktrees → containers → containers+worktrees
* **Axis B – concurrency / correctness:**

  * single agent → multiple agents in one tree → multiple agents in worktrees

Given what you’ve already built (orchestration, subagent router, HUD, `.openagents`), the sweet spot **per mode** is:

1. **During design + rapid iteration (now):**

   * **Worktrees without containers.**
   * Why:

     * You’re still iterating on orchestrator behavior and CC integration.
     * Worktrees give you:

       * clean per-task branches,
       * multi-agent concurrency,
       * simple debugging (`cd .worktrees/<id>`).
     * You don’t pay container friction while you’re still changing inner loops every hour.

2. **As soon as you start letting agents touch truly untrusted code / multi-tenant use:**

   * **Worktrees + containers.**
   * Why:

     * Containers are your “seat belt + airbag”.
     * Worktrees are your “lane markers” preventing git-level collisions.
     * Together you get:

       * per-task isolation at git level,
       * per-agent isolation at OS level,
       * a path to “10 CC subagents running in parallel overnight” without risking your laptop.

3. **Containers without worktrees**:

   * Reasonable only if:

     * you run **one** agent at a time, or
     * you’re in a hosted CI-style environment where each run uses a fresh clone/container and you don’t care about parallelism on the same repo.
   * For MechaCoder’s future (N agents on same project), it’s the weakest fit.

---

### TL;DR

* **Worktrees** solve **git correctness + parallelism**.
* **Containers** solve **security + resource isolation**.
* For MechaCoder:

  * **Worktrees, no containers** → best *right now* for speed & clarity; still risky security-wise.
  * **Containers, no worktrees** → fine for single-agent / CI, not for your multi-agent dream.
  * **Containers + worktrees** → the *endgame* for “Claude Code + orchestrator + overnight farm”; heavier to run, but aligns with your vision.

If you want, next step we can sketch **how to actually wire “orchestrator → worktree → CC container”** for a single repo, then generalize to “parallel N” – but that’s independent of Grok vs CC.
