# Omega Agent plugin tool specification

- Status: specification under ProductSpec revision 3, 2026-07-27
- Owner: OpenAgents
- Companions: [slim-agent audit](./2026-07-27-slim-agent-audit.md),
  [slim-agent specification](./2026-07-27-slim-agent-spec.md)
- ProductSpec of record: `specs/omega/omega-agent.product-spec.md` at
  `spec_revision: 3`
- Admission state: the owner direction of 2026-07-27 set the six-tool
  surface. The direction names the tools: `read`, `write`, `edit`,
  `bash`, `delegate`, and `plugin`. Revision 3 of the ProductSpec
  records that surface. This document specifies the sixth tool.

This specification defines `plugin`, the sixth tool of Omega Agent.
`plugin` calls deterministic functionality: a typed, versioned,
sandboxed function with a receipt for every run.
`delegate` hands work to another agent. `plugin` calls a function.
An agent answer is judgment. A plugin result is a fact.

## 1. Product statement

A person talks to one agent, and the agent has six tools.
Four tools do direct work: `read`, `write`, `edit`, and `bash`.
`delegate` hands a task to another executor and names it.
`plugin` runs a deterministic function and proves the run.

A plugin is a unit of packaged capability.
It has a name, an exact version, and a content address.
It declares a typed input schema and a typed output schema.
It runs in a sandbox with no ambient authority.
The same input and the same plugin version give the same output.
Every run produces a receipt that a stranger can check.

The plugin is also the marketplace unit.
A person can install a plugin, publish a plugin, and later sell a
plugin, with revenue sharing to its author.
Episode 262 names the layers Omega adds to the fork: verification,
markets, revenue-sharing, and multiplayer.
The `plugin` tool is where the first three layers meet the agent.

## 2. Lineage

This tool is not a new idea. It is the return of a proven one, on a
foundation that can hold it. Three OpenAgents systems feed this
contract, and each contributes exact laws.

### 2.1 The 2024 plugin economy

The retained transcript archive records a complete plugin economy,
built and operated in 2024:

| Episodes | What ran |
| --- | --- |
| [`048`](../transcripts/048.md)-[`061`](../transcripts/061.md) | A plugin architecture for extensible agent logic. WASM modules loaded through Extism, a plugin registry with upload and inspection surfaces, per-plugin fees, and agents calling plugins as work steps. |
| [`062`](../transcripts/062.md)-[`075`](../transcripts/075.md) | Payments. Agents paying L402 Lightning endpoints, plugins deployed behind paid endpoints, host functions for plugin HTTP access, and plugin nodes composed in agent graphs. |
| [`066`](../transcripts/066.md), [`088`](../transcripts/088.md) | Decentralized distribution. A community-built Nostr plugin registry, and a NIP-78 key-value plugin through which agents share state over relays. |
| [`087`](../transcripts/087.md) | Self-extension. A community plugin wired into a meta-agent upgrade loop: the agent improves through plugins. |
| [`092`](../transcripts/092.md)-[`098`](../transcripts/098.md) | The market. The Agent Store launch: a marketplace with revenue sharing paid in Bitcoin, first daily, then every minute, metered on usage. |

The lessons that survive as laws:

1. Plugins are WASM in a sandbox, not code in the host process.
2. Discovery belongs on an open registry, and a signed listing needs no
   platform's permission.
3. A plugin can carry a fee, and its author can be paid from usage,
   automatically, in Bitcoin.
4. The unit must be small enough for one community member to build.
   Both registry milestones in the archive were community submissions.

### 2.2 DSE, DSPy in Effect

The [DSE history audit](../dspy/2026-07-20-dspy-in-effect-git-history-audit.md)
records the February 2026 implementation of `@openagentsinc/dse`:
Declarative Self-Improving Effect, "DSPy, but Effect TS."
DSE had typed Effect Schema signatures, structured prompt artifacts
with canonical JSON and stable hashes, an evaluation system with
datasets and metrics, a bounded deterministic compiler, immutable
policy artifacts, receipts, and budgets.
The removal was an architecture reset, not a quality rejection.
DSE returned in the released SDK train: `@openagentsinc/dse` with
`contract`, `optimizer`, and `runtime` entry points, under the
single-authority guard (#9163).

The lessons that survive as laws:

1. A callable unit declares typed input and output schemas, and the
   boundary validates both.
2. An artifact is immutable and content-addressed. A stable hash names
   exactly what ran.
3. Runs are evaluated against datasets and metrics, and evaluation is
   cached and reported.
4. Optimization is bounded and deterministic, and an optimizer output
   is a candidate, never a deployment.

### 2.3 Blueprint

Blueprint is the typed program kernel: Signatures, Modules, and Program
Runs, with the first typed program live as `khala.fleet.delegate` in
`packages/khala-tools/src/fleet-delegate-program.ts`.
Its modules are named steps with typed preconditions, typed blocker
codes, and admitted parameters.
Its standing invariant transfers to `plugin` unchanged:

1. **Program runs are decision evidence. They do not authorize
   writes.** A plugin result never carries authority. The agent, the
   permission ladder, and the engine keep their existing authority.
2. **No keyword routing.** Plugin selection is an exact name from the
   installed catalog, never a string match over intent.
3. **Optimizer acceptance is not runtime promotion.** A better plugin
   version is a candidate until an explicit install pins it.

### 2.4 Verifiable software

Episode [`259`](../transcripts/259.md) positions the IDE as the engine
of verifiable software: work that carries its own proof, with receipts
that let acceptance be checked and paid by strangers.
A deterministic plugin is the smallest unit of verifiable software.
Its receipt binds exact code to exact input and exact output, and a
verifier can replay the run.

### 2.5 Pay the People, and the Khala composition

Episode [`223`](../transcripts/223.md), Pay the People, states the
economic law this tool serves.
Anyone who contributes anything valuable to an AI workflow gets paid a
share of revenue, in Bitcoin, proportional to the contribution.
Episode 1 made that promise in response to a platform that promised
revenue sharing and paid nothing.
The 2024 Agent Store kept it at small scale.
The plugin is the contribution unit that lets Omega keep it at product
scale: a bounded, verifiable, meterable thing one person can build,
whose usage receipts are its payout record.

Episode [`242`](../transcripts/242.md) defines Khala as "one collective
mind, built up of a bunch of plugins, a bunch of little programs
that can compose into a response," selected by Bitcoin-paid verified
value, grown from an open pool rather than engineered from a closed
one.
Episode [`244`](../transcripts/244.md) names the assembled result a
super model.
The `plugin` tool gives that composition its local unit.
A plugin that extends Omega Agent on one machine is the same shaped
unit that can serve in Khala's open pool for everyone.
A person who publishes a plugin contributes a capability to the super
agent, and revenue sharing pays that person when the capability is
used.
The seam stays a seam: Omega runs plugins locally, Khala composes them
across the network, and the phase-three admissions in section 6 own
when payment turns on.

## 3. What a plugin is

A plugin is a package with these parts:

| Part | Content |
| --- | --- |
| Manifest | Name, exact version, author identity, one-line description, capability declarations, and the schema references |
| Input schema | A typed schema for the single input value |
| Output schema | A typed schema for the single output value |
| Module | The executable body, compiled to WebAssembly |
| Content address | A digest over manifest, schemas, and module. The digest is the plugin's identity |

Definition laws:

1. **Deterministic core.** A plugin run makes no model call. Given the
   same input, the same plugin digest, and the same declared-capability
   results, the output is identical.
2. **Typed boundary.** The input validates against the input schema
   before the run. The output validates against the output schema
   after the run. A violation is a typed tool error, never a partial
   result.
3. **Exact identity.** Every reference to a plugin resolves to an
   exact version and digest. A bare name resolves through the installed
   catalog's pin. There is no silent `latest`.
4. **Sandboxed execution.** The module runs in a WASM sandbox with no
   ambient authority. No filesystem, network, clock, or randomness
   reaches the module except through a declared capability.
5. **Declared capabilities.** The manifest declares every capability
   the module can request, from a closed vocabulary. The first
   vocabulary is: `http_fetch`, `project_read`, `clock`, and `random`.
   A capability the manifest does not declare is refused at
   installation, not discovered at run time.
6. **Recorded capability results.** The receipt records a digest of
   each capability request and result. Replays substitute the recorded
   results, so a run with capabilities is still verifiable.

Points 5 and 6 keep the owner's word "deterministic" honest.
A pure plugin is deterministic outright.
An effectful plugin is deterministic relative to its recorded
capability results, and the receipt makes the difference visible.

## 4. The tool contract

### 4.1 Input

| Field | Meaning |
| --- | --- |
| `plugin` | The plugin name from the installed catalog, or `name@version` for an explicit pin |
| `input` | The input value. It must validate against the plugin's input schema |

### 4.2 Output

1. The output value, validated against the output schema.
2. A typed run receipt: plugin name, exact version, content digest,
   input digest, output digest, capability log digest, duration, and
   outcome.
3. A `plugin:<run_id>` address. `read` on that address returns the
   full receipt and the bounded run log.

### 4.3 Catalog visibility

1. The system prompt carries a bounded plugin catalog: name, one-line
   description, and nothing else. The catalog section follows the
   skills-catalog pattern and its budget discipline.
2. `read` on `plugin:<name>` returns the manifest, both schemas, and
   the capability declarations. The model reads a schema before a
   first call, exactly as it reads a skill body.
3. An empty catalog removes the section. The prompt stays
   cache-stable under `OMEGA-DELTA-0135`'s equality gate.

### 4.4 Failure classes

Every failure is typed. The classes at this revision:

| Class | Meaning |
| --- | --- |
| `no_plugin` | The name is not in the installed catalog. The message lists near matches, and the tool never substitutes. |
| `input_invalid` | The input failed schema validation. The error names the failing path. |
| `output_invalid` | The module produced output that failed its own schema. The run fails. A plugin does not get partial credit. |
| `capability_refused` | The module requested an undeclared or ungranted capability. |
| `plugin_trap` | The module trapped, exceeded its fuel bound, or exceeded its memory bound. |
| `plugin_timeout` | The run exceeded the wall-clock bound. |

`no_plugin` mirrors `delegate`'s `no_executor` law: name it or fail,
never guess.

## 5. What `plugin` is not

The six tools stay distinct, and the distinctions are the design:

| Surface | It is | It is not `plugin` because |
| --- | --- | --- |
| `bash` | Arbitrary commands with full user authority | Untyped, unbounded, host-specific, and unreceipted beyond the transcript. A shell script cannot carry a schema, a digest, a capability declaration, or a revenue share. |
| `delegate` | A task handed to another agent | Nondeterministic by nature. A delegate exercises judgment and returns prose plus a disclosure. A plugin computes a value and returns proof. |
| Skills | Instructions loaded into the model's context | A skill changes what the model says. A plugin computes without the model. A skill can teach the model when to call a plugin. |
| MCP tools | A live server's tool surface, trusted per server | Session-bound, version-fluid, and unpinned. A plugin is content-addressed and offline-verifiable. The slim profile keeps MCP off. |
| DSE programs | Typed model programs with optimization | A DSE program calls models by design. A compiled DSE artifact's deterministic parts can ship as a plugin, and its model-calling parts cannot. |

One sentence for the model, and for the person:
use `bash` for this machine, `delegate` for judgment, and `plugin` for
a fact you may need to prove later.

## 6. Distribution and the market

Distribution arrives in three phases. Each later phase needs its own
owner admission. Phase boundaries are product law, not sequencing
preference.

### 6.1 Phase one: local

1. Plugins install from a local path or a fetched archive into the
   user catalog. A project can carry project-scoped plugins.
2. Installation verifies the digest, validates the manifest and both
   schemas, and records the capability declarations.
3. Installation is the trust act. It shows name, author, digest, and
   declared capabilities. Capability grants happen here, once,
   visibly.
4. The agent can author a plugin with its ordinary tools: write the
   module source, compile it with `bash`, and propose installation.
   Installation of an agent-authored plugin is explicit in the
   transcript, never a silent side effect of a turn.
   This is the Episode 087 self-extension loop, with the trust act
   kept visible.

### 6.2 Phase two: registry

1. A public registry lists plugins as signed listings: manifest,
   digest, author identity, and version history.
2. The Nostr registry lineage (Episodes 066 and 088) is the reference
   shape: signed, decentralized, replicable listings that need no
   platform permission. The exact protocol binding is decided at the
   phase-two admission, under the standing rule that Nostr carries
   the record and never command authority.
3. Install-from-registry keeps the phase-one trust act unchanged.
   The registry adds discovery, never authority.

### 6.3 Phase three: the paid market

1. A plugin may carry a price, and its author a payout address.
2. Usage metering, payment acceptance, and revenue sharing run on the
   existing OpenAgents payment rails. The plugin system mints no
   second treasury, no second ledger, and no second payout path.
3. The 2024 Agent Store proved the loop this phase restores: a
   marketplace with revenue sharing paid in Bitcoin, metered on usage,
   at minute granularity (Episodes 092 through 098).
4. Every paid run's receipt is the billing record. Payment disputes
   resolve against receipts, not prose.
5. This phase is the Pay the People commitment made structural: a
   person who contributes a capability to the super agent earns a
   share of the revenue that capability produces, for as long as it is
   used.

## 7. Execution substrate

The decision criteria bind, and the exact runtime is chosen at the
implementation packet:

1. The substrate executes WebAssembly with fuel, memory, and
   wall-clock bounds.
2. It supports deny-by-default host functions, so the capability
   vocabulary maps to explicit imports.
3. It embeds in the Rust application without a process per call.
   A process boundary per plugin run is a cost the interactive loop
   does not pay.
4. Candidates: Wasmtime as the embedded engine, with the Extism
   envelope from the 2024 arc as prior art for the manifest and
   host-function shape.

The sandbox posture is stricter than `bash` on purpose.
Omega chose allow-by-default for the person's own shell.
A plugin is third-party code, and the standing risk rule applies:
untrusted-origin work does not get full-allow execution.
Capability grants at installation keep the friction at one visible
moment, which honors confirm-on-data-loss-never-on-capability while
the code stays contained.

## 8. Receipts and verification

The run receipt is the product surface of this tool.

1. Receipt fields: plugin name, exact version, content digest, input
   digest, output digest, capability log digest, duration, fuel used,
   and outcome class.
2. The receipt is a typed record first and a rendered line second,
   per the shape record's standing condition. A later Nostr-signed
   record needs a signer, not a rewrite.
3. A verifier with the plugin package, the input, and the recorded
   capability results reproduces the output digest. Verification is
   replay, in the Tassadar sense, at function scale.
4. Receipts project into the same evidence surfaces as delegate
   disclosures. A turn that used plugins can show which facts came
   from which digests.

## 9. Deltas required

Each row lands as a numbered delta in `OMEGA_DELTAS.md` with a
mechanical check in `crates/omega_deltas`, watched failing first.
Numbers come from the delta cleanup lane.

| Proposed delta | Content | Check sketch |
| --- | --- | --- |
| The six-tool surface | The basic profile holds exactly `read`, `write`, `edit`, `bash`, `delegate`, `plugin` | Parse `default.json`, assert the six names |
| The typed plugin boundary | Input and output validate against the plugin's schemas | Unit tests: valid round trip, `input_invalid` path, `output_invalid` path |
| No model call in a plugin run | The plugin runtime has no reference to the model-request path | Compile-boundary check: the plugin crate does not depend on the language-model crates |
| Exact identity | Every run receipt carries version and digest, and no resolution path yields an unpinned module | Assert receipt fields, assert catalog pin resolution |
| Deny-by-default capabilities | An undeclared capability request fails as `capability_refused` | Unit test with an undeclared-import module |
| The visible trust act | Installation renders name, author, digest, and capabilities, and agent-proposed installs require the explicit act | Assert the installation flow's typed transcript result |
| Bounded execution | Fuel, memory, and wall-clock bounds hold | Unit tests: a spin loop hits `plugin_trap`, a sleep-shaped module hits `plugin_timeout` |

## 10. Packet plan

The program extends the `OMEGA-AGENT` ledger. Every packet cites
ProductSpec revision 3. Issues are minted under the claim protocol at
dispatch, and this specification reserves names only.

| Packet | Goal | Depends on |
| --- | --- | --- |
| PLUG-00 | ProductSpec revision 3, landed with this specification | none |
| PLUG-01 | The plugin package format: manifest, schemas, digest, and the installation verifier | PLUG-00 |
| PLUG-02 | The sandbox runtime: WASM execution, capability imports, bounds, failure classes | PLUG-01 |
| PLUG-03 | The `plugin` tool registration, the prompt catalog section, and the `plugin:` read addresses | PLUG-02, SLIM-01 |
| PLUG-04 | Run receipts, the capability log, and replay verification | PLUG-02 |
| PLUG-05 | The agent-authored plugin journey: write, compile, propose, explicit install | PLUG-03 |
| PLUG-06 | Behavior contracts and the delta sweep for the program | PLUG-01 through PLUG-05 |

Phase-two and phase-three packets are not minted here. Each needs its
own owner admission per section 6.

## 11. Acceptance criteria

These criteria are recorded in ProductSpec revision 3 as
`OMEGA-AGENT-AC-22` through `OMEGA-AGENT-AC-25`, with the six-tool
surface amended into `OMEGA-AGENT-AC-15`.

- **PLUG-AC-01:** The basic profile exposes exactly six tools to the
  model: `read`, `write`, `edit`, `bash`, `delegate`, `plugin`.
- **PLUG-AC-02:** A plugin run makes no model call, and its input and
  output validate against the plugin's declared schemas.
- **PLUG-AC-03:** Every plugin run produces a typed receipt with the
  exact version, content digest, input digest, output digest, and
  capability log digest. A replay with the recorded capability
  results reproduces the output digest.
- **PLUG-AC-04:** Plugins execute in a deny-by-default sandbox. An
  undeclared capability fails typed. Fuel, memory, and wall-clock
  bounds hold.
- **PLUG-AC-05:** A `plugin` call for an unknown name returns
  `no_plugin` and never substitutes. Resolution always lands on an
  exact digest.
- **PLUG-AC-06:** Installation is the visible trust act. An
  agent-authored plugin enters the catalog only through the explicit
  installation flow.
- **PLUG-AC-07:** A plugin result is evidence, never authority. No
  plugin output bypasses the permission ladder, the guard laws, or
  the engine's run authority.
- **PLUG-AC-08:** No phase-one packet adds a registry, a price, a
  payment path, or a payout path. Those arrive only through the
  phase-two and phase-three admissions.

Falsifiers, stated so the program can be caught:

1. A plugin that reaches the network or filesystem without a declared,
   granted capability.
2. A run whose receipt cannot be replayed to the same output digest.
3. A silent plugin installation, by the agent or by a project checkout.
4. A model call observed inside the plugin runtime.
5. A payment or registry surface that lands before its admission.

## 12. Considerations and risks

1. **The sixth-tool cost.** The slim program argued five tools hard,
   and the sixth must not open the door to a seventh. The six-tool law
   replaces the five-tool law with the same rigidity: the next tool
   needs a new spec revision, and the delta check asserts the six
   names exactly.
2. **Prompt weight.** The catalog section costs prompt bytes per
   installed plugin. The byte ceiling from `OMEGA-DELTA-0135` binds
   the whole prompt, so a large catalog forces summarization, not
   ceiling growth.
3. **WASM toolchain friction.** Authoring requires compiling to WASM,
   which is heavier than writing a script. The agent-authored journey
   (PLUG-05) must make this a one-command act, or self-extension will
   route around `plugin` into unreceipted `bash` scripts.
4. **Capability vocabulary creep.** Every added capability weakens the
   determinism story. The vocabulary is closed, small, and grows only
   by delta.
5. **Marketplace timing.** Phases two and three repeat a loop that ran
   in 2024 and was torn down in resets. The phase gates exist so the
   market returns on proven receipts, not on nostalgia.
6. **Relationship to DSE.** DSE programs optimize model behavior, and
   plugins package deterministic behavior. The seam is explicit: a DSE
   artifact's deterministic parts can compile into a plugin, and the
   `plugin` runtime never grows a model path to absorb the rest.
7. **Documentation language.** Program documentation lands in
   `docs/omega-agent/` in this repository and in the omega repository where
   the fork's discipline requires. These internal records can use normal
   technical language and do not run the public STE checks.

## 13. Open owner questions

1. **The capability vocabulary.** This specification proposes
   `http_fetch`, `project_read`, `clock`, and `random` as the closed
   first set. The owner can cut or extend it at the PLUG-01 admission.
2. **Pure-only phase one.** A stricter start ships phase one with no
   capabilities at all: pure plugins only, capabilities arriving with
   phase two. This trades usefulness for a cleaner determinism story.
   This specification includes the four capabilities. The owner can
   cut to pure-only.
3. **The registry protocol.** Phase two proposes signed listings with
   the Nostr lineage as the reference shape. The binding is decided at
   that admission.
4. **Payment rails.** Phase three assumes the existing OpenAgents
   payment rails. The owner decides the exact rail at that admission,
   under the standing Spark-primary policy.
