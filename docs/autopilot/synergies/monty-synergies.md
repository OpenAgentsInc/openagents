# Monty and OpenAgents Autopilot / Effect / DSE: Synergies and Learnings

This doc explores **Monty** (`~/code/monty`, [Pydantic Monty](https://github.com/pydantic/monty))—a minimal, secure Python interpreter written in Rust for running LLM-generated code—and how it aligns with our **Autopilot**, **Effect**, and **DSE** approach. It summarizes what we can learn and where integration or shared patterns could add value.

**Monty in one sentence:** Run a subset of Python written by an LLM inside your process; no containers, no host access; all I/O and side effects go through **external functions** you provide; execution can **pause at those calls**, **serialize**, and **resume** later.

---

## 1. Monty recap (relevant bits)

- **Goal:** Let agents run code safely without containers. Startup in microseconds; no CPython, no full stdlib.
- **Security:** No filesystem, env, or network unless you explicitly pass them in as **external functions**. Resource limits (memory, time, stack depth) with hard cancellation.
- **Execution model:** Code declares which external functions it may call. When the VM hits such a call, it **yields** (returns a snapshot: function name, args, kwargs). The host runs the function, then **resumes** with a return value (or exception). So every side effect is a clear boundary.
- **Snapshot / resume:** Both the parsed program and the paused state (at an external call) can be **serialized** (dump/load). Enables durable execution: persist at I/O, resume in another process or later.
- **Type checking:** Optional typecheck of LLM-generated code (e.g. with `ty`) before execution.
- **Surfaces:** Rust core; Python (PyO3) and **JavaScript/TypeScript** (napi-rs) bindings. Used by Pydantic AI for “code mode” (LLM writes Python that calls tools; Monty runs it).

---

## 2. Alignment with our constraints and goals

| Our constraint / goal | Monty angle |
|-----------------------|-------------|
| **No containers** (Autopilot spec) | Monty is explicitly “no containers”: in-process interpreter, no Docker, no sandbox service. Fits our “no containers” rule if we ever run agent-authored code. |
| **Tiny tool surface** (MVP) | Today we only have built-in tools. Monty doesn’t replace tools; it generalizes “one round of code” that can call a fixed set of host functions (our tools as externals). |
| **DSE: typed, optimizable, auditable** | Signatures and tool contracts are typed; compilation produces artifacts. Monty adds: run LLM output only after typecheck and with a fixed external-function surface derived from our contracts. |
| **Effect: clear boundaries, testability** | External functions are a clear boundary (like our tool runtime). Effect services could *be* the host side of Monty externals: one Effect program per external, same testability and layering. |

So Monty is **compatible** with our stack and **complementary**: we keep Autopilot (Workers + DO, no containers), Effect (orchestration, services), and DSE (signatures, compile, artifacts); Monty becomes an optional **execution backend** when the agent emits code instead of (or in addition to) tool calls.

---

## 3. Synergies

### 3.1 External functions as tool boundary

In Monty, the guest can only call functions you list and implement. You pass a map `external_functions: { fetch: async (url) => ... }`. That is exactly a **capability boundary**: our tool registry could define the allowed “external” names and schemas; the Worker (or a dedicated service) runs Monty with those tools as the only externals. So:

- **Tool contracts** (name, input/output schema, policy) stay the source of truth.
- **Monty run** gets a restricted set of implementations (our tool implementations, possibly wrapped in Effect).
- No need to give the interpreter arbitrary host access; same principle we want for tools.

**Learning:** Treat “tools” and “externals for code execution” as the same capability surface. DSE tool contracts could drive both classic tool-calling and the allow-list for Monty.

### 3.2 Code-mode vs many tool rounds

Motivation (Anthropic, Cloudflare Codemode, Pydantic AI): instead of N tool calls and N round-trips, the LLM writes a small Python script that calls your tools as functions; you run it once and get one result. Fewer round-trips, often cheaper and faster.

We could support an **optional code path**:

- Agent (or a DSE module) sometimes emits **Python** (or a restricted form) instead of a sequence of tool calls.
- Worker receives the code, typechecks it (e.g. against a stub that only has our tool signatures), then runs it in **Monty** with our tools as external functions.
- One execution, one reply; transcript still records “agent ran code (Monty) with tools X, Y.”

DSE could even have a **signature** whose output is “Monty script + allowed externals”; the compiler optimizes when to use code vs tool-only flows.

### 3.3 Snapshot / resume and durability

Monty can **dump** state at every external call and **load** it later (even in another process). We already have durability at **message** boundaries (transcript in the Durable Object). We could add durability at **execution** boundaries:

- After each external (tool) call inside a Monty run, optionally persist the snapshot.
- On failure or timeout, we can resume from last snapshot instead of re-running from the start.
- For replay or audit, we have a clear “state at each I/O” story.

**Learning:** “Pause at I/O, serialize, resume” is a first-class pattern for agent execution, not only for Monty—we could design our tool runtime so that long or multi-step runs can be checkpointed at tool boundaries.

### 3.4 Type checking LLM output

Monty supports running a typechecker (e.g. `ty`) on the script before execution, with type stubs you provide. That matches DSE’s “typed contracts” mindset:

- **Stubs** could describe only the allowed externals (our tool names and signatures).
- Typecheck fails if the code calls something not in the allow-list or with wrong types.
- We get a cheap, static guard before we run anything.

**Learning:** For any “agent emits code” path, typecheck against our tool/signature contracts first. DSE already has schemas; we can derive stubs or a schema-based validator for the generated code.

### 3.5 Security checklist

Monty’s CLAUDE.md and design stress a clear **security surface**: no filesystem/env/network by default, path traversal risks, resource limits, no unsafe Rust, etc. We can adopt a similar **explicit checklist** for:

- Autopilot worker (what the DO can do, what it can’t).
- Any future “run agent code” path (Monty or otherwise): allowed externals, resource limits, timeout, no escape.

**Learning:** Document and review a short “Autopilot execution security” checklist (inputs, outputs, capabilities, limits), similar in spirit to Monty’s.

### 3.6 Resource limits

Monty tracks memory, allocations, stack depth, and execution time and **cancels** when limits are exceeded. We should do the same for:

- Tool execution (time, memory if we ever run code).
- Any Monty run we host: time, memory, and a strict external-function set.

**Learning:** Every execution path (tool only, or Monty) should have explicit resource limits and a defined behavior when exceeded (abort, retry, or fail the step).

### 3.7 Effect as the host side of externals

Our tool implementations could be **Effect services** (or use them). When Monty yields at an external call:

- The Worker (or a small runner) maps the call to the right Effect program (e.g. `ToolX.run(args)`).
- We run it with the same layers we use for loaders and API (Telemetry, config, etc.).
- So “Monty external” and “tool execution” share the same Effect stack and observability.

**Learning:** If we add Monty, keep a single “tool/external” implementation surface in Effect; Monty’s `resume(returnValue)` is just another caller of that surface.

---

## 4. What we should learn from Monty’s approach

1. **External functions as the only side-effect boundary** — No implicit I/O. We already want tools to be explicit; extending that to “code run” (only these externals) keeps the model simple and auditable.

2. **Pause at I/O and serialize** — Even without Monty, designing our tool runner so that “after each tool call” we could persist state would help with replay, debugging, and long-running flows.

3. **Typecheck before run** — For any LLM-generated code path, validate against our contracts (tool names, schemas) before execution. Reduces bad runs and makes failures easier to interpret.

4. **Explicit resource limits** — Time, memory, depth; document and enforce them for every execution path.

5. **Single capability surface** — One registry of “what the agent can do” (tools + optionally “run code with these externals”). DSE and the runtime should both consume that surface.

6. **JS/TS bindings** — Monty’s Node/TS API means we could, in principle, run Monty from a TS Worker (if we had native bindings or a small RPC to a Monty host) or from a separate Node service. The integration point exists; we don’t have to leave the TypeScript ecosystem to benefit from the model.

---

## 5. Possible next steps (no commitment)

- **Document** an “Autopilot execution security and limits” checklist (inspired by Monty’s security section).
- **Design** “tool execution + optional checkpoint at each tool” so we can later add snapshot/resume if we introduce code execution.
- **Prototype** (when useful): one DSE signature or tool that “runs Monty script” with a fixed list of externals (e.g. a subset of our tools), to validate latency, security, and observability in our stack.
- **Keep** DSE tool contracts as the single source of truth; if we add code-mode, derive Monty’s external allow-list and type stubs from those contracts.

---

## 6. References

- **Monty:** `~/code/monty`, [github.com/pydantic/monty](https://github.com/pydantic/monty) — README, CLAUDE.md, plan.md, examples (expense_analysis, sql_playground), crates (monty, monty-js, monty-python).
- **Our spec:** `docs/autopilot/spec.md` (no containers, one Autopilot per user, tiny tool surface).
- **Our DSE:** `docs/autopilot/dse/dse.md` (signatures, modules, tool contracts, compile, artifacts).
- **Our Effect:** `packages/effuse/docs/effect-migration-web.md`, `packages/effuse/docs/effuse-conversion-apps-web.md`.
