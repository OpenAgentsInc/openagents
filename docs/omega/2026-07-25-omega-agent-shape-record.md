# Omega Agent shape record

- Status: shape record for `OMEGA-AGENT-00`, 2026-07-25
- Owner: OpenAgents
- Issue: omega#74, under epic omega#73
- Supersedes the design ask in omega#72
- ProductSpec: [Omega Agent product contract](../../specs/omega/omega-agent.product-spec.md)
  at `spec_revision: 1`
- Companion: [cloud-coupling severability trace](./2026-07-25-omega-agent-cloud-severability-trace.md)
- Source pin: `OpenAgentsInc/omega` `b768854c56` (`0.2.0-rc10`)

This document records the shape decision for the first-party Omega Agent.
It records the alternatives, the reason the accepted shape wins, and the cost
of each rejected alternative.
It also records one question the owner has not answered.

This document is a record. It is not release authority and not public-claim
authority.
The ProductSpec owns intent. `docs/omega/2026-07-25-omega-agent-roadmap.md`
owns sequence.

## 1. What omega#72 asked

Issue omega#72 asked for four decisions before code.

1. Which shape, A, B, or C.
2. The default model for a first-party agent.
3. What first party means for identity, and whether an agent turn is signed.
4. Whether `agent_ui` is kept or replaced.

Issue omega#72 also asked for a cloud-coupling trace with file-level evidence.
That trace is the companion document.

## 2. The alternatives, as omega#72 stated them

| Shape | Description | Stated benefit | Stated cost |
| --- | --- | --- | --- |
| A | Extend `crates/agent` in place with our tools, prompt assembly, and permission model. | Cheapest. Inherits the turn loop and the persistence. | Omega carries the upstream runtime decisions. Divergence costs more at each rebase. |
| B | A separate binary that speaks ACP and registers through `crates/agent_servers`, as `codex-acp` does. | Clean boundary. Independent test. No rebase pressure. | A process boundary and inter-process communication on each turn. |
| C | A Nostr-native agent. The durable record is signed Nostr events. ACP is the execution transport. | Portable, signed, replayable agent history. | The largest. It needs the relay path proven first. |

## 3. The accepted shape

The accepted shape is a Rust implementation of the existing `AgentConnection`
trait that owns routing, disclosure, and receipts, and owns no execution.
Its executors are the native loop in `crates/agent`, external ACP agents
through `crates/agent_servers`, and `omega-effectd` engine lanes.

### 3.1 Evidence that the seam supports this shape

| Fact | Evidence |
| --- | --- |
| `AgentConnection` is a trait with eight required methods and many defaulted methods. | `crates/acp_thread/src/connection.rs:91` through `:260` |
| A new implementation needs `agent_id`, `telemetry_id`, `new_session`, `auth_methods`, `authenticate`, `prompt`, `cancel`, and `into_any`. | `crates/acp_thread/src/connection.rs:91`-`:260` |
| A checked downcast for implementation-specific capability already exists. | `crates/acp_thread/src/connection.rs:259` and `:262`-`:266` |
| Capability extension traits already use that pattern. | `AgentSessionTruncate` `:268`, `AgentSessionRetry` `:285`, `AgentTelemetry` `:293`, `AgentSessionModes` `:303` |
| The panel already downcasts to the native connection for native-only behavior. | `crates/agent_ui/src/agent_panel.rs:3523` and `:4537`, `crates/agent_ui/src/conversation_view.rs:1026`, `:1234`, `:1808` |
| The panel `Agent` enum is `#[non_exhaustive]`, so a new variant is possible without a fork. | `crates/agent_ui/src/agent_ui.rs:424`-`:438` |
| `crates/acp_thread` depends on no `client` crate and no cloud crate. | `crates/acp_thread/Cargo.toml` |

The seam supports the accepted shape. The code does not need a fork of
`AcpThread` and does not need a second thread projection.

### 3.2 Why the accepted shape wins

1. It reuses a proven interface. The native runtime already adapts to this
   interface, and every external agent already arrives through it.
2. It needs no new thread user interface, no new persistence, and no new
   review surface.
3. It keeps the router at keystroke latency, because the decision runs
   in-process.
4. It keeps policy in `omega-effectd`, which already owns lane readiness,
   capacity, and receipts.
5. It adds no process boundary to an interactive turn.

### 3.3 Why the alternatives lose

**Shape A as omega#72 stated it.** An extension of `crates/agent` in place
puts the router inside the executor. That breaks the rule that the router owns
no execution. It also grows the surface that each upstream rebase touches.

**Shape B.** A separate ACP binary gives a clean boundary. It also puts
inter-process communication on every interactive turn, including the first
keystroke of a new thread. It gives no benefit that the seam does not already
give, because `crates/acp_thread` is already free of cloud dependencies.

**Shape C.** A Nostr-native agent gives portable, signed, replayable history.
The relay path is live at `relay.openagents.com`, but the record contract in
`docs/omega/2026-07-24-sarah-nostr-record-contract.md` is written for the
Sarah workroom, not for a coding agent thread. Shape C also needs the identity
answer in section 6, which the owner has not given. Shape C is deferred, not
rejected.

### 3.4 A naming correction the record must carry

The roadmap calls the accepted shape "A at the seam".
That label is not omega#72's shape A.
Shape A in omega#72 extends `crates/agent` in place.
The accepted shape adds a sibling `AgentConnection` implementation and leaves
`crates/agent` as one executor.

The two share one property only. Both stay in-process Rust with no new process
boundary.
Read "A at the seam" as "in-process Rust at the `AgentConnection` seam".
Do not read it as an instruction to change `crates/agent` in place.

### 3.5 A benefit claim the record must qualify

The roadmap states that shape B's benefit arrives later, through omega#82,
which serves Omega Agent over loopback ACP.
That claim is partly true and partly not.

Omega#82 gives external attachability. An ACP host can attach the router.
Omega#82 does not give the other three benefits omega#72 attributed to shape
B. The router still lives in the fork tree.
It keeps rebase pressure, the in-tree test boundary, and the in-tree build
boundary.

## 4. The default model answer

The fork ships `google/gemini-3.6-flash`.

| Fact | Evidence |
| --- | --- |
| The default model is provider `google`, model `gemini-3.6-flash`. | `assets/settings/default.json:1087`-`:1095` |
| The model exists in the provider enumeration. | `crates/google_ai/src/google_ai.rs:525`, `:553`, `:566` |
| One test asserts the provider. | `crates/app_identity/src/service_isolation.rs:118` |

The omega#72 claim that the default is `ollama/llama3.1` is stale.

Two stale statements remain in the omega repository. This lane reports them
and does not change them.

| Stale statement | Location | Correction |
| --- | --- | --- |
| "`agent.default_model` is still `ollama/llama3.1`" and "the service-isolation test asserts the current value (`service_isolation.rs:113`)" | `OMEGA_DELTAS.md:263`-`:266` | The value is `google/gemini-3.6-flash`. The assertion is at `service_isolation.rs:118`. |
| "Omega's default native-agent selection is `ollama/llama3.1`" | `docs/src/development/omega-native-agent.md:48` | The value is `google/gemini-3.6-flash`. |

One gap in the test coverage. `service_isolation.rs:118` asserts the provider
string `google` only. No test asserts the model string `gemini-3.6-flash`.
`crates/omega_deltas` contains no default-model check at all. A rebase can
change the model string without a test failure.

## 5. The `agent_ui` answer

`crates/agent_ui` is kept.

Omega-only capabilities travel by an extension trait behind the existing
`into_any` downcast. No packet forks `AcpThread`.

The omega#72 concern was that `crates/agent_ui` carries `cloud_api_types`.
That concern is accurate but small. The severability trace records the exact
use sites. One user-visible control in `crates/agent_ui` sends to a cloud
endpoint. That control is thread feedback, and section 4 of the trace records
its current failure condition.

The omega#72 reference to `agent_ui/Cargo.toml:46` is correct at this source
pin. `cloud_api_types.workspace = true` is at
`crates/agent_ui/Cargo.toml:46`.

## 6. The open question, reserved to the owner

**Does the first-party Omega Agent sign with the Omega Nostr identity?**

The owner has not answered this question.
This lane does not answer it.
The roadmap states that "the Omega Nostr identity binding stays a binding, not
a merge", which describes a preference and not a decision.

The question decides what a Nostr-backed agent thread record contains, and who
authored it.

### 6.1 Branch one: the agent signs with its own principal

The agent gets a principal keypair, as `principal.sarah` has one under
`docs/omega/2026-07-24-sarah-nostr-identity-contract.md`.
Agent turns are signed events with the agent as author.

Cost.

1. A new signer principal needs custody, a sealed signer, and a lifecycle.
   The Sarah identity contract is the precedent and the work is not small.
2. Turn records need a kind. Kind `44300` is defined as the Sarah turn record
   and is not a coding-agent thread record. A new kind needs its own contract,
   canonical fixtures, and a projection map.
3. A signed agent record is portable and replayable across devices and hosts.
   This is the property shape C exists to give.
4. Every routed executor result must be attributable inside a record the agent
   signed. The disclosure obligation becomes a record obligation, not only a
   user-interface obligation.

### 6.2 Branch two: the agent does not sign, and history projects only

The owner identity signs the workroom record.
Agent history projects onto that record as rows the owner authored.

Cost.

1. No new custody surface and no new signer lifecycle.
2. No portable per-agent history. A second device sees the owner record and
   cannot verify which agent produced a row from the signature alone.
3. The record cannot separate an agent statement from an owner statement by
   author. It must separate them by field, which is weaker.
4. A later move to branch one rewrites the thread record. Issue omega#72
   named exactly this cost in its falsifier.

### 6.3 Recommendation

Take branch two for the packets now open, and keep branch one available.

Reason. The packets now open, omega#75 through omega#82, need no signature.
They need a name, a front door, honest disclosure, a deterministic router, and
receipts. None of those five needs a signed record.

Condition. The disclosure fields must be designed as record fields from the
first packet, not as user-interface strings. If the disclosure line is a
string built for a label, branch one later needs a rewrite. If the disclosure
is a typed record that a label renders, branch one later needs only a signer.

This is a recommendation. It is not a decision.
The owner decides.

### 6.4 The honest tension with omega#72

Issue omega#72 states a falsifier.
A start of the build before the owner settles the identity question falsifies
the design.
The omega#72 words are "the latter decides whether agent history is portable,
and retrofitting it later means rewriting the thread record".

The current program starts the build and defers the identity question.
That is a real conflict, and this record states it rather than hides it.

The mitigation is the condition in section 6.3. A typed disclosure record
keeps the retrofit cost at the signer, not at the thread record.
If the owner does not accept that mitigation, the identity question becomes a
hard gate on omega#77 and later.
The program must then wait for the answer.

## 7. Findings this lane reports and does not fix

These findings belong to other lanes. This lane changes no file in the omega
repository.

1. **Duplicate delta identifiers.** `OMEGA-DELTA-0010` appears twice, at
   `OMEGA_DELTAS.md:188` ("The title-bar identity entry stays local") and at
   `:210` ("Zed collab is retired"). `OMEGA-DELTA-0011` appears twice, at
   `:199` ("AI onboarding configures providers, not a hosted plan") and at
   `:238` ("The agent is on by default"). The delta cleanup lane owns this.
2. **The duplicate is invisible to the check.** `ENFORCED_DELTAS` at
   `crates/omega_deltas/src/omega_deltas.rs:25`-`:39` lists both identifiers
   twice. The consistency test `the_registry_and_the_checks_agree` at `:579`
   collects both sides into `BTreeSet` values, which removes the duplicate.
   The test passes with the collision present.
3. **The rename delta number is not reserved here.** The highest identifier is
   `0011`, so `0012` is the safe floor. A firm reservation is unsafe until the
   cleanup lane decides between two options. Option one keeps the first
   occurrence of each identifier and renumbers the second occurrence, which
   consumes `0012` and `0013` and makes the rename delta `0014`. Option two
   renumbers differently. `OMEGA-AGENT-01` must take its number from the
   cleanup lane, not from this record.
4. **No delta check protects the default model.** See section 4.
5. **Two stale default-model statements.** See section 4.
6. **Thread feedback cannot succeed.** See section 4 of the severability
   trace.

## 8. What this record does not decide

1. It does not rename any identity surface. Issue omega#75 owns the rename.
2. It does not allocate a delta number.
3. It does not admit a release, a packaged journey, or a public claim.
4. It does not open the Khala packets. `OMEGA-AGENT-K1` through
   `OMEGA-AGENT-K3` need their own owner admission.
5. It does not answer the identity signing question in section 6.
