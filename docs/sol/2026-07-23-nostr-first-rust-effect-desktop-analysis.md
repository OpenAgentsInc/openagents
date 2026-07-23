# Nostr-first OpenAgents Desktop with Rust and Effect

- Date: 2026-07-23
- Class: architecture analysis
- Status: recommended direction for evaluation
- Dispatch: no
- Owner: OpenAgents Desktop architecture
- Base: `64b7f0615e2889cb0fe6334943e67f4a7c21bbd5`
- Source snapshot: exact sources and document digests in this analysis

## Decision

OpenAgents can use much more Rust in the Desktop codebase.
OpenAgents should test this direction now.
The first target should be a medium Rust Nostr engine, not a full Buzz port.

The Rust engine should prove, sign, transport, store, index, and replay Nostr
facts. Effect should decide what those facts mean for OpenAgents.
Effect should also keep product policy, commands, approvals, and receipts.

This split is larger than the current small Rust helper model.
It is still one product with one authority model.
It is not a second application core.

OpenAgents should also test a Zed fork.
That test must stay separate from the current Desktop.
A Zed fork is a native-shell and license decision.
It is not an efficient way to import some editor widgets.

The recommended evaluation has two candidates:

1. Add a Rust Nostr engine below the current Electron and Effect host.
2. Make an isolated GPL Zed fork that connects to OpenAgents services.

The same product journey must test both candidates.
The result must decide the future shell.

This analysis does not revise a ProductSpec.
It does not admit Rust crates or copied source.
It does not change the current roadmap.

## Why the prior decision can move

The current architecture made Rust a small native rind.
That decision matched the prior product shape.
Nostr-first Desktop changes the workload.

The prior [Effect and Rust decision](../ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md)
selected an 85 to 90 percent Effect application.
The [current Zed port status](../ide/2026-07-19-porting-zed-to-effect-status.md)
also rejected a GPUI port.
This analysis reopens those choices for a measured comparison.

Nostr adds one cohesive native domain:

- canonical event format and signature checks
- signer isolation and key use
- long-lived relay sessions
- subscription generations and replay
- a local signed-event archive
- an offline outbox
- deduplication and replaceable-event selection
- Git credentials and commit signatures

The current `nostr-effect` source already covers the audited Buzz NIPs.
Rust is not required to fill a wire-format gap.
Rust can add a durable native engine and an independent conformance oracle.

These tasks form a protocol and data plane.
They are not only small OS calls.
A supervised Rust process can own this plane and not own product semantics.

OpenAgents already has Rust and Effect in one repository.
The root Cargo workspace contains Cloud crates.
Desktop also packages the `oa-desktop-audio` Rust helper.
This proves that the toolchains can coexist.
It does not prove that two application authorities are safe.

The current Rust exception is too narrow for this proposal.
The current ProductSpec, invariants, and IDE roadmap must change before
implementation starts.

## Source identity and freshness

The source audits used tracked Git data.
They did not treat upstream prose as target authority.

| Source | Audited or inspected source | Freshness note | License note |
| --- | --- | --- | --- |
| OpenAgents | `64b7f0615e2889cb0fe6334943e67f4a7c21bbd5` | Current `origin/main` at analysis start | Root Apache-2.0. Desktop package declares MIT |
| Buzz | `acfbb1bb6af54cb29cb152496ff43b8285dcb8cf` | Local tree matched `upstream/main` | Apache-2.0. Keep Block attribution and change notices |
| Zed | `f032f4d433da3747f9d7bcc9e9cd52d6ca3fb3e4` | Current upstream was `97d854b89b5b38c189e9dee6351b861fbb202214` | App and most crates use GPL-3.0-or-later. Some crates use Apache-2.0 |
| Codex | `1bbdb32789e1f79932df44941236ea3658f6e965` | Current upstream was `5bdbd3ee90d746c3b8a040a53c434262ed07ee74` | Apache-2.0 with a `NOTICE` file |
| Grok Build | `c1b5909ec707c069f1d21a93917af044e71da0d7` | Current public source was `69f0ba880aa98f55e3ac1dcc570e2f332f825fe2` | Apache-2.0 with third-party source records |

The current upstream values were checked on 2026-07-23.
They are freshness markers.
They are not audited implementation pins.
Any implementation must select a new exact pin.

The primary local evidence has these SHA-256 values:

| Evidence | SHA-256 |
| --- | --- |
| [Zed teardown](../teardowns/2026-07-18-zed-teardown.md) | `aeda4d02866bff2f8d287c9e76053ea5f680b52ca19026d2475411327130c4e6` |
| [Buzz teardown](../teardowns/2026-07-21-buzz-teardown.md) | `f4e1b22f02870e974a8340b2a72b97fd2d3cdfaa17ac4969a6272219e52ae7c0` |
| [Codex teardown](../teardowns/2026-07-10-codex-agent-runtime-teardown.md) | `e31c1935bbdc8abba4652d4b4589d0061f1d05bff38c0ffb6c60933199d11f2f` |
| [Grok Build teardown](../teardowns/2026-07-15-grok-build-teardown.md) | `355287244597eca66616481e1c2cbfaa59ef9a6a53d3f6ff36a68dcfa8f69b44` |
| [FastFollowSpec](../../FASTFOLLOW.md) | `6ebed1e30c5fa28be4773243ad7a95e81d23a74a4a23c8a6694d5dd6ed671805` |
| [Sol roadmap](./MASTER_ROADMAP.md) | `6cf2200f70f32c9668b2d5342a7089d26b8801014b72709eac37e428d07523d6` |
| [Effect and Rust decision](../ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md) | `164afac197f5e73966249fd3f18ba789850e3d66d3cf864a6f0968f1a4f313dc` |
| [Current Zed port status](../ide/2026-07-19-porting-zed-to-effect-status.md) | `4d4c64a5a5271b05e25c7d2a62ba332d3189cfcffb56c671b06be445814d7ed5` |
| [IDE roadmap](../ide/ROADMAP.md) | `0b7590ccf355c1ba3b523e2429e722abef17ccdf25ace9d397fa034b9ec8e335` |
| [Desktop ProductSpec](../../specs/desktop/desktop-trust-complete-workbench.product-spec.md) | `019c4b3b55c69f94764bb44c65c6f5bbe135b2dfb71e40145907fbea5ddbe9d0` |
| [Effect versus Rust analysis](../fable/2026-07-17-effect-vs-rust-architecture-analysis.md) | `cec80f6e683440a273d122b30a005e638a07e8aec0a13f3f9150e72e642626f0` |

## What “Nostr-first” should mean

Nostr-first should describe the product edge.
It should not mean that one relay becomes all product authority.

The Desktop can make signed identity and social context primary.
It can still use the correct authority for each action.

| Concern | First authority |
| --- | --- |
| signed profile, post, reply, reaction, and relay observation | Nostr event and signer |
| local Nostr archive and queued publish state | Rust Nostr engine |
| accepted OpenAgents command and outcome | Effect command processor |
| thread and fleet product state | Khala Sync and current service authority |
| provider, harness, and worktree execution | Pylon and current runtime services |
| file, document, Git, terminal, and IDE state | Effect project graph |
| user interface and product projection | Effect Native |
| public proof | admitted receipt and evidence compiler |

A relay acceptance is not an IDE save.
It is not a Git merge or a completed agent run.
A signed event can be evidence or a proposal.
Effect must admit it before it becomes an OpenAgents command.

This design avoids a false single truth.
It also gives Nostr a real product role.
Nostr becomes the signed identity, social, and collaboration edge.

## The social IDE opportunity

The social layer should use project and worktree identities.
It should not be a general chat tab beside the IDE.

The first useful product journey can include:

1. A user attaches a project to an explicit NIP-29 group.
2. The project has a NIP-34 repository address.
3. People and agents publish signed activity to the group.
4. A user can read posts, replies, and reactions in project context.
5. A patch or issue event opens the exact code evidence.
6. An agent can propose work from a signed event.
7. Effect applies normal policy and creates a work unit.
8. The result carries an exact commit, patch, test, and receipt.
9. OpenAgents publishes a separate signed outcome event.
10. Offline relay time does not lose local work or invent success.

The initial social scope should include:

- profiles and owner-agent relations
- project rooms and membership
- posts, replies, reactions, and presence
- NIP-17 private messages after a custody review
- NIP-34 repository, issue, patch, and status events
- signed Git credentials and commit signatures
- explicit relay, signature, and provenance state
- links between social events and exact IDE evidence

The initial scope should not include:

- Buzz workflow execution
- relay-owned product commands
- automatic execution from an inbound event
- a custom rich-message format
- a new provider or fleet authority
- a second conversation database
- hidden signer key export

## The proposed Rust boundary

The first Rust program should be a supervised Desktop engine.
This analysis uses `oa-nostrd` as a temporary name.

The process should have a versioned contract.
Effect Schema should remain the contract source.
Generated fixtures must test the Rust decoder.

The process can own:

- raw Nostr event validation
- canonical event IDs and signature checks
- relay connection state
- NIP-42 authentication
- bounded subscription state
- reconnect, catch-up, and replay
- event deduplication
- replaceable-event selection
- a local Nostr SQLite archive
- a queued publish outbox
- signed-event operation facts

The process must not own:

- OpenAgents command admission
- product permissions or approvals
- project or document identity
- provider or harness selection
- Full Auto policy
- Khala Sync product rows
- UI state
- a general secrets database
- final OpenAgents receipts

Use a process boundary.
Do not use FFI, N-API, or a linked native module.
The process must not share the Electron address space.

The protocol needs:

- binary and protocol versions
- process and connection generations
- bounded frames and queues
- typed overload and gap states
- deadlines and cancellation
- explicit capability negotiation
- deterministic shutdown and drain
- crash and incompatible-version states
- contract fixtures in both languages

The current Desktop build has one special audio-binary path.
The new work should first add a native component manifest.
That manifest should describe each binary and target.
It should also describe its protocol, hash, signature, and rollback.

## What to port from Buzz

The current Buzz source has about 222,000 Rust lines under `crates`.
Its Tauri native source adds about 94,000 Rust lines.
A bulk import would add about 315,000 lines before the web user interface.

The initial useful source is much smaller.
The audit estimates 12,000 to 18,000 Rust lines.

### Extract as product-native crates

- `buzz-core` verification, filter, relay URL, and network rules
- selected `buzz-sdk` builders and parsers
- NIP-OA owner attestation
- NIP-AE encrypted agent memory
- NIP-AM agent usage facts
- NIP-AO agent observation frames
- NIP-29 and NIP-34 event builders
- Git permission checks
- the pure audit hash-chain primitive
- independent conformance fixtures

These parts need new OpenAgents names and boundaries.
They also need a file-level provenance record.
Do not keep Buzz product policy in generic event code.

### Adapt as algorithms

- SQLite write-ahead-log and busy-timeout setup
- latest-wins replaceable-event selection
- equal-time local-edit protection
- tombstone-first application
- queued outbox and compare-and-clear
- bounded transport queues and cancellation
- secure-storage failure rules
- no-redirect authenticated fetch

Do not copy Tauri command handlers.
Do not create a second Khala Sync database.
Do not infer channel security from event tags.

### Keep outside the Desktop core

- `buzz-relay`
- `buzz-db`
- `buzz-search`
- `buzz-pubsub`
- `buzz-admin`
- `buzz-workflow`
- `buzz-agent`
- most of `buzz-acp`
- `buzz-push-gateway`
- the Tauri application shell

These parts assume Buzz server policy.
They also assume Postgres, Redis, object storage, or Tauri state.
OpenAgents should keep hosted Buzz external during the first client test.

The small Buzz WebSocket client is only seed evidence.
It does not provide a complete relay pool.
It lacks durable replay, backpressure, and multi-relay merge.
OpenAgents should evaluate a mature Rust Nostr client library.

## What to port from Codex and Grok Build

Apache-licensed Rust can strengthen the same native layer.
Each import still needs a small target-owned boundary.

| Source | Candidate | Disposition |
| --- | --- | --- |
| Codex | apply-patch parser and file-change model | Extract a bounded crate or exact dependency closure |
| Codex | sandbox policy, protected paths, and egress proxy | Adapt behind the authority compiler |
| Codex | bounded process control | Adapt under Effect supervision |
| Codex | app-server protocol | Use as design and fixture evidence |
| Grok Build | `ptyctl` | Evaluate as a bounded PTY engine |
| Grok Build | crash-time terminal restoration | Adapt as a native reliability helper |
| Grok Build | fast worktree plans and pools | Extract only a safe plan and recovery subset |
| Grok Build | ACP leader lifecycle | Adapt version, lock, drain, and reconnect rules |

Do not import whole Rust application cores.
The Codex candidates have large internal dependency closures.
The Grok public source is a sequence of source drops.
It is not a normal upstream fork.

Do not port the Grok sandbox behavior.
It can warn and continue after some failures.
OpenAgents containment must fail closed.

Do not copy Codex provider, account, or cloud authority.
OpenAgents already controls Codex through the supported executable and protocol.
Source reuse must not create a second Codex product.

## Zed: fork, do not embed

The [Zed teardown](../teardowns/2026-07-18-zed-teardown.md) supports one
strong conclusion.
Zed quality comes from one coherent project graph.
Rust alone does not create that quality.

GPUI is not a normal widget library.
It owns application state, tasks, focus, windows, input, and render state.
An attempt to embed GPUI in Electron would create two window and event authorities.
It would not provide the Zed editor and project graph.

Most valuable Zed code is also GPL-3.0-or-later:

- rope, text, language, buffer, and editor
- worktree, project, workspace, and project panel
- Git and review
- terminal
- agent and ACP thread surfaces
- collaboration

The Apache-licensed GPUI and `sum_tree` crates do not change this result.
Their full dependency closure needs a license audit.
They also do not provide the complete editor.

Direct Zed reuse is coherent only as a fork of the application.
That fork would replace the current shell.
It would also replace many release and accessibility proofs.

The isolated Zed test should:

1. Select a fresh exact Zed pin.
2. Keep the stock project and editor architecture.
3. Add one OpenAgents protocol client.
4. Add one Nostr project panel.
5. Keep OpenAgents product authority in the service contract.
6. Measure the same end-to-end journey as the current Desktop.

Do not copy this spike into `apps/openagents-desktop`.
Keep it in an isolated experiment until a decision.

A Zed fork can win only if the owner accepts:

- GPL distribution duties
- source and notice duties
- a new release and update system
- regular Zed upstream rebases
- replacement of Electron, Monaco, and Pierre assumptions
- a new accessibility proof
- a profile migration and rollback plan

A separate process can reduce technical dependency.
It does not automatically resolve license obligations.
Legal review must decide the distribution model.

## Comparison test

Three builds should run the same journey:

1. Current Electron and Effect Desktop.
2. Electron and Effect with the Rust Nostr engine.
3. The isolated Zed fork with OpenAgents and Nostr clients.

The journey is:

1. Attach a project and open a file.
2. Edit, save, create a conflict, and restart.
3. Start an agent turn in an isolated worktree.
4. Review and apply a versioned patch.
5. Run a test and record the result.
6. Publish a signed project post and reply.
7. Open a NIP-34 patch from the social view.
8. Disconnect the relay and continue local work.
9. Reconnect, replay, and deduplicate events.
10. Prove that no relay fact widened IDE authority.

Measure:

- cold start and time to input
- input, open, search, and replay p50, p95, and p99
- idle and active memory
- binary and update size
- 20,000-file and 100,000-file projects
- crash recovery and handle cleanup
- helper protocol drift
- signer and secret containment
- packaged accessibility
- release, signature, update, and rollback
- upstream rebase conflicts and patch size
- six supported build targets where required

The Zed test must include the editor accessibility tree.
The audited Zed build did not expose the custom editor in the macOS
accessibility tree.

## Decision gates

### Gate 1: provenance and contract

Before source import:

- select exact source commits
- list every copied file and digest
- record original and target paths
- retain license, copyright, and change notices
- produce an SBOM for each dependency closure
- define an update and deletion policy
- freeze one Effect Schema process contract

### Gate 2: Rust Nostr engine

The engine must pass:

- canonical event and signature vectors
- differential checks against `nostr-effect`
- NIP-42 authentication fixtures
- subscription replay and gap tests
- deduplication and replaceable-event tests
- offline outbox recovery
- corrupt-store and crash recovery
- bounded queue and overload tests
- signer isolation tests
- six-target package checks

Two Nostr implementations create a useful oracle.
They also create drift risk.
The fixture gate is mandatory.

### Gate 3: product authority

The test must prove these rules:

- a signed event does not become a command by itself
- relay membership does not grant file or process authority
- a relay acknowledgement does not become a product outcome
- Nostr and Khala stores have explicit and separate purposes
- a compromised helper cannot widen Effect policy
- the renderer never receives a seed or `nsec`

### Gate 4: shell decision

The Zed fork must win the common product journey by a material amount.
It must also have an acceptable rebase and release cost.
The owner must accept its license and product reset.

If these conditions do not hold, keep Electron.
Graduate only the Rust Nostr engine.

## Risks and reversal tests

| Risk | Reversal test |
| --- | --- |
| Rust becomes a second product core | Stop when Rust starts to define commands, approvals, projects, or receipts |
| Nostr and Khala become rival truths | Stop when one user action can settle differently in both stores |
| protocol implementations drift | Stop import when shared fixtures do not produce identical wire facts |
| native package work grows without control | Stop new helpers until the native component manifest proves six-target update and rollback |
| copied source creates upgrade debt | Stop when the owned patch is larger than the retained value |
| Zed fork creates continuous rebase work | Reject the fork when the patch budget or conflict rate exceeds the set limit |
| GPL duties do not fit the product | Reject direct Zed reuse after legal review |
| accessibility becomes worse | Reject the shell when the packaged editor journey fails the accessibility gate |
| Rust does not improve the product | Delete the helper when the common benchmark shows no material benefit |

## Recommended sequence

The next authority update should create one accepted evaluation plan.
It should not start a full migration.

The plan should contain these packets:

1. Native component manifest and provenance ledger.
2. Pure `oa-nostr-core` and `oa-nostr-events` fixture spike.
3. Supervised `oa-nostrd` archive, outbox, and relay spike.
4. One Nostr-first social IDE journey in the current Desktop.
5. One isolated Zed fork with the same journey.
6. Comparative evidence and owner disposition.
7. ProductSpec, AssuranceSpec, invariant, and roadmap revision only after the
   owner selects a result.

The two implementation spikes can proceed in parallel.
They must not share a mutable shell or release path.
One integration owner must control the common protocol and benchmark.

## Final assessment

The answer is yes, with a boundary.

OpenAgents can port selected Buzz Rust into the Desktop repository.
It should port a coherent Nostr protocol engine.
It should not copy the Buzz application or relay server.

OpenAgents can also use good Rust from Codex and Grok Build.
It should extract small mechanics with exact provenance.
It should not assemble a new megacore from upstream applications.

A Zed fork is credible.
It is credible only as a separately gated native Desktop product.
It is not a low-cost code import.

The strongest near-term shape is:

- Nostr-first at the identity and collaboration edge
- Effect-first for product semantics and authority
- Rust-first for Nostr mechanics and selected native systems work
- Zed as a real comparative fork, not an embedded library

That shape makes the product more social and more native.
It keeps one command and receipt authority.
The benchmark can now decide whether the final shell stays Electron or becomes
a Zed-derived native application.
