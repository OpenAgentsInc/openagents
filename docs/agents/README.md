# General agent infrastructure

Status: target architecture, 2026-09-21. OpenAgents is building reusable agent
infrastructure. **Coding is the first specialization**, implemented through
Coder. A shared protocol or runtime primitive is not evidence that every
domain adapter, product, or assurance level already exists.

The [TypeSafe coding-agent proposal](../coder/design/thoughts-on-a-typesafe-coding-agent.md)
is a useful concrete design input. Its ideas about explicit state, small typed
judgments, scoped tools, context selection, and shared work apply beyond coding.
Its Git, shell, code-search, and test examples supply one domain's operations.
The [OpenAgents NIPs](../../nips/openagents/README.md) define shared contracts;
the [remaining architecture work](roadmap.md) identifies what still needs
design and implementation for broader use.

## Semantic contracts and measured improvement

Define the behavior an agent needs independently of the model or inference
strategy that supplies it. The [AI programming architecture](../optimization/README.md)
separates semantic signatures, pinned implementations, bounded optimization,
and host enforcement. DSPy and GEPA can help author or search implementations;
Gym and domain evaluators establish workload evidence.

Typed decisions, generation, retrieval, and bounded compositions are alternative
implementation materials. The shared infrastructure must permit measured
replacement of those choices while protecting authority, disclosure, required
evidence, effect confirmation, and budgets. Coding-specific question sets and
benchmarks do not define the general semantic interface.

[OPT](../../nips/openagents/NIP-OPT.md) records the lifecycle. Implementation
is deferred; the [consolidated proposals](../optimization/proposed-issues.md)
cover the complete integration without requiring every host to run an optimizer.

## Three layers

| Layer | Responsibility | Examples |
| --- | --- | --- |
| Shared infrastructure | Identify, admit, execute, coordinate, explain, and evaluate bounded agent work. | Identity, component releases, typed programs, task state, context, grants, budgets, receipts, recovery, and evaluation reports. |
| Domain profile | Supply the sources, resource semantics, operations, policies, and acceptance criteria for a class of work. | Coding, research, document processing, data analysis, and account/record workflows. |
| Product and host | Bind the profile to real systems and provide an interface users can control. | Coder's terminal/headless host; a future research client or business-service worker. |

A *domain profile* is an architectural term for an assembly of existing
components and host configuration. It is not a new Nostr kind, a new EXT
component type, or an implicit grant. EXT packages distribute its portable
parts; CAP bindings and host policy supply the actual connections and authority.
One host can support several profiles without duplicating the program engine.

## What generalizes

| Shared concept | General meaning | Coder specialization | Other example |
| --- | --- | --- | --- |
| Task | Objective, constraints, inputs, acceptance criteria, and unresolved work. | Repair a parser regression. | Produce a cited research brief. |
| Resource | An object or effect destination in an admitted tenant/account/system scope. | Repository file, branch, or worktree. | Document, dataset, calendar, service record, or message destination. |
| Observation | Versioned evidence of what a source returned, with completeness and consistency limits. | File bytes and test output at a known base. | Document revision, query result, or captured service response. |
| Context | Exact permitted representations supplied to one operation or recipient. | Failure, source spans, instructions, and selected tool schemas. | Relevant passages, customer constraints, or selected rows. |
| Operation | Typed input/output with explicit effects, bounds, and a host binding. | Search, edit, test, or delegate. | Retrieve, transform, compare, draft, or conditionally update. |
| Program | Reusable bounded dataflow among admitted operations. | Investigate, repair, check, and propose integration. | Collect sources, compare claims, draft a report, and check citations. |
| Authority | Independent permission for a principal to act in a particular scope. | Read/write grants and protected checks. | Account-scoped retrieval or approval for an exact outgoing message. |
| Coordination | Dependencies, claims, shared budgets, freshness, and recovery. | Isolated writers with a protected integration gate. | Independent analysts sharing captures or a conditional record-update queue. |
| Verification | Evidence that the domain's stated acceptance criteria hold. | Test results and independent artifact checks. | Citation coverage, schema consistency, or provider-confirmed state. |
| Integration | Authorized adoption into the intended destination when requested. | Apply a verified patch to the current base. | Accept a report revision or confirm a record update. |
| Evaluation | Workload-specific outcome evidence tied to exact versions. | Repair success, regressions, and cost. | Supported claims, wrong-recipient errors, or reconciliation accuracy. |

CAP, PRG, EXT, RUN, CTX, POL, COORD, and EVAL express these reusable contracts.
CJ's decision and execution families carry domain-independent jobs; `CJ` is
the retained historical identifier. Existing Coder conversation payloads keep
their own compatibility rules. No repository, terminal, or shell is mandatory
in the shared execution family.

## What remains specific to coding

Repository discovery, Git revision rules, source-language parsers, symbol
indexes, compiler diagnostics, test runners, shell permits, patch formats,
worktree isolation, and merge checks belong to the coding profile. AGENTS.md
discovery is one implementation of scoped instructions. A compiler exit status
is one form of verification evidence. These adapters should not become required
fields or universal success criteria for every agent.

Rust is the implementation language of this repository; that choice does not
restrict the domains the resulting agents can serve. Likewise, a terminal is
Coder's first interface. The shared records can support other interfaces and
service workers without embedding terminal behavior in the protocol.

The existing engine and adapters are often still inside `crates/coder`.
Generalization is initially a contract and dependency boundary. Extract shared
Rust modules when a second real consumer needs them and tests demonstrate that
they have no implicit Git, current-directory, or Coder configuration dependency.
Renaming crates before that evidence would not create a reusable runtime.

## What a domain profile must supply

1. **Resource and source identity.** Define tenant/account boundaries, canonical
   IDs and aliases, supported version tokens, read consistency, retention, and
   source-adapter schemas. A live service response is a captured observation;
   it is not automatically a transactional snapshot of the world.
2. **Operations and effects.** Provide typed inputs/results, compatible host
   bindings, preconditions, destinations, disclosure rules, and limits. Reading
   an API and mutating it require different authority even at the same endpoint.
3. **Execution guarantees.** State what is enforced by the host, guaranteed by
   the external system, estimated, or unknown. Bind idempotency, confirmation,
   cancellation, and reconciliation to the actual operation.
4. **Policy and credentials.** Identify who can authorize each action, how scope
   policies are authenticated, and which recipients may see each input. Keep
   credentials in host-owned stores; a package or document cannot grant itself
   organization authority.
5. **Evidence and acceptance.** Define what counts as a draft, a submitted
   action, a confirmed effect, a verified outcome, and accepted integration.
   Provide protected checkers or explain when verification remains unavailable.
6. **Workload evaluation.** Supply representative cases and adverse outcomes.
   Preserve refusals, unknown effects, human escalation, and failed retries in
   results. Coding calibration and task-success measurements do not transfer
   automatically to another workload.

These definitions use existing EXT schemas/descriptors and CAP host bindings.
New schema semantics must be pinned and supported before admission. Arbitrary
domain strings in metadata cannot introduce new authority, effects, or workflow
transitions. Unknown required semantics refuse.

## External effects need domain guarantees

The shared `writes` scope includes sending, publishing, changing records, and
other external mutations. It is broader than filesystem access. An operation's
compute/spend allowance does not automatically cap the value of a transaction
it performs; the domain's typed input and policy must bound that separately.

A coordinator can fence participating workers. It cannot lock an independent
person or application out of a third-party service merely by recording a claim.
Use provider preconditions, downstream idempotency, and authoritative readback
where supported. Explicitly accepted weaker assurance remains labeled as such;
when a required guarantee is unavailable, the operation refuses.

Preparing a message or record update is different work from sending or applying
it. Approval binds the exact action, destination, state preconditions, and
domain limits. A timeout after submission can mean unknown effect, so retry
only after reconciliation under the binding's contract. Compensation is a new
action; it can fail and does not erase the original history. Worktrees protect
local drafts but cannot roll back an external action.

The revised v1 drafts now provide an external-observation format, generic
instruction scopes, and generic findings/proposals. They also clarify resource
and integration semantics. Long-lived waiting, richer event subscriptions,
multi-party authorization, and specialized physical or media constraints still
need the contracts and proofs listed in the [roadmap](roadmap.md).

## Two reference workflows

**Research assistance:** A host captures an admitted document collection with
versioned sources. It constructs a task-specific context, runs bounded claim
comparison, produces a cited draft, and independently checks that citations
support the statements. A reviewer can consume a different context over the
same sources. No Git checkout is needed. The report can remain local, and
publishing it requires a separately admitted operation.

**Service-record assistance:** A host reads a scoped record and retains the
provider version and capture. The agent proposes a typed update. Admission
checks account authority, payload, current preconditions, and limits before
submission through a configured adapter. A conditional provider update and
authoritative confirmation establish the effect where supported. Lost responses
produce reconciliation work, not automatic duplicate mutations. This is a
future reference profile, not a deployed integration claim.

These should exercise the same artifact, policy, execution, and evaluation
contracts used by Coder. Begin with read-only research and a local mock record
service so the generality proof does not require real external mutations.

## Where changes belong

| Change | Place |
| --- | --- |
| Meaning needed by independent hosts or clients | Shared NIP schema/profile with conformance fixtures. Reuse existing envelopes and jobs when their semantics fit. |
| New domain operation, evidence format, or acceptance rule | Pinned schema, source/operation/checker definition, package, and host adapter. |
| Scheduling, credential access, provider calls, OS enforcement, or transactional claims | Host/coordinator implementation. Record its effective guarantees. |
| Presentation, approval UI, source expansion, notifications, and inspection | Client over shared host records. |
| Claims about quality, safety, efficiency, or portability | Workload-specific evaluation and explicit supported-role evidence. |

A new domain does not automatically need a new NIP or event kind. Add protocol
semantics only when existing typed components and host policy cannot express
the required interoperable behavior. Preserve ordinary local execution and
bounded non-agent operations; every useful action need not start a program or
consult a model.
