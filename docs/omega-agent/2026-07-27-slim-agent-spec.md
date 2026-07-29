# Omega Agent slim-agent specification

- Status: specification under ProductSpec revision 2, 2026-07-27
- Owner: OpenAgents
- Companion: [slim-agent audit](./2026-07-27-slim-agent-audit.md)
- Omega source pin: `OpenAgentsInc/omega` `beb0e870b2`
- ProductSpec of record: `specs/omega/omega-agent.product-spec.md` at
  `spec_revision: 2`
- Admission state: the owner direction of 2026-07-27 set the basic-agent
  product shape, and revision 2 of the ProductSpec records it. Section 12
  records what the revision contains.
- Revision 3 amendment, 2026-07-27: a later owner direction on the same
  day admits a sixth tool, `plugin`, and ProductSpec revision 3 records
  the six-tool surface. The five-tool law in section 2 reads as the
  six-tool law under that revision. The
  [plugin tool specification](./2026-07-27-plugin-tool-spec.md) owns the
  sixth tool. Everything else in this specification stands.

This specification defines the slim first-party Omega Agent.
The agent has five tools: `read`, `write`, `edit`, `bash`, and `delegate`.
It has a short, measured system prompt.
It cannot destroy uncommitted work through its own tools without a
confirm.
It delegates to the other harnesses Omega runs as a first-class act.

## 1. Product statement

A person opens Omega and talks to one agent.
The agent reads, writes, edits, and runs commands, like the Pi coding
agent does.
When a task fits a different harness better, the agent hands the task to
that harness through the `delegate` tool and says so.
Every result names the executor that produced it.
The agent never destroys work the person did not ask it to destroy.

The slim agent is the native executor of Omega Agent.
ProductSpec revision 2 names it **the basic agent**: the one first-party
agent that works reliably for all people with no delegation at all.
It is not a new runtime.
It is the existing native loop in `crates/agent` with a five-tool
model-visible surface, a short prompt, a work-loss guard, and delegation
promoted to a first-class tool.

## 2. Design laws

The Omega program laws bind this specification unchanged, in particular:

1. Confirm on irreversible data loss, never on capability.
2. Every default change lands as a numbered delta in `OMEGA_DELTAS.md`
   with a mechanical check in `crates/omega_deltas`.
3. Omega never creates a second home for an external agent. No
   credential, home, or provider-configuration copy.
4. Provider prose never closes work. Only typed outcomes close work.
5. Routing and delegation decisions are typed and fail-closed. No keyword
   matching. No silent substitution.
6. `omega-effectd` stays the only Omega mutation path for run authority.

This specification adds two laws:

7. **The five-tool law.** The model-visible tool set of the slim profile
   is exactly `read`, `write`, `edit`, `bash`, and `delegate`.
   A sixth model-visible tool in the slim profile needs a new admission.
   ProductSpec revision 3 is that admission: it adds `plugin`, and the
   law continues as the six-tool law with the same rigidity.
8. **The work-loss law.** No tool of the slim agent discards uncommitted
   changes without a typed confirm, and every mutating tool call is
   preceded by a snapshot the agent itself can restore.

## 3. The five tools

The five tools are registrations over existing, tested machinery.
The turn loop, the `AgentTool` trait, streamed input, the action log,
checkpoints, tool-result artifacts, and the permission ladder all stay.

### 3.1 `read`

One tool reads every address the agent can hold.

The name and the ground contract come from the Pi source at
`packages/coding-agent/src/core/tools/read.ts` in the pinned `pi` clone.
Pi's `read` is not `read_file` under a shorter name, and it is not a
directory reader.
The exact Pi contract:

1. It reads text files and images (jpg, png, gif, webp, bmp), and it
   sends an image as an attachment, with a text note when the model has
   no image support.
2. It does not read a directory. A directory path fails, and listing
   belongs to `ls` and the shell.
3. Output truncates at the head to 2,000 lines or 50 KiB, the first
   limit that fires.
4. `offset` and `limit` select a 1-indexed line range, and every
   truncation marker names the exact next step: "Use offset=N to
   continue."
5. A single line above the byte limit gets a marker that names a `bash`
   fallback command for that line.
6. The tool prompt tells the model: use `read` to examine files instead
   of `cat` or `sed`.

The Omega `read` adopts that contract and widens the address space, not
the file semantics:

| Address form | Behavior | Replaces |
| --- | --- | --- |
| A file path | Line-numbered content, `offset` and `limit` ranges, continuation markers, image support, outline fallback for large files | `read_file` |
| A directory path | A typed refusal that names `bash` (`ls`) as the path | nothing. Listing is shell work |
| `tool:<tool_call_id>` | The full artifact behind a bounded tool-result preview | `read_tool_result_artifact` |
| A delegate session address | The bounded transcript of a subagent this thread spawned | `read_subagent_transcript` |
| A skill location from the prompt catalog | The skill body | `skill` |

Contract points:

1. Every read records the file modification time in the action log, so
   the staleness check in `edit` keeps working.
2. The parent-only access law for subagent transcripts
   (`OMEGA-DELTA-0060`) holds unchanged: the environment answers for the
   asking thread, and the tool cannot name another thread.
3. Bounds and truncation markers follow `OMEGA-DELTA-0111` and
   `OMEGA-DELTA-0121`: every bound that fires says so, and every printed
   address is one the model can spend.
   Pi's "Use offset=N to continue" marker satisfies the same law, and
   the Omega `read` emits the same continuation form on a truncated
   file read.
4. The directory refusal is a structured tool error with the `ls`
   pointer in its text, not a silent empty result.

### 3.2 `write`

Creates a file or replaces its full content.

1. Streamed input stays on, so the diff renders while the input arrives.
2. The dirty-buffer overwrite prompt stays: an overwrite of a buffer with
   unsaved user changes offers cancel, and cancel preserves the user's
   work.
3. Every write feeds the action log, so diff review, keep and reject, and
   checkpoint restore keep working.
4. Writes outside the project keep the sandbox path-grant flow.

### 3.3 `edit`

Applies exact old-text to new-text replacements to one file.

1. Streamed input stays on.
2. The staleness law is strict: when the file changed on disk after the
   agent's last `read`, the edit refuses with a typed staleness result,
   and the agent must read again.
   The refusal is a structured tool error, not a silent overwrite.
3. The dirty-buffer save-or-discard prompt stays.
4. Every edit feeds the action log.

### 3.4 `bash`

Runs a command in the project environment.

1. The fail-closed command-chain parser stays.
   Unparseable substitution or chaining in a protected command denies.
2. The hardcoded catastrophic-command denials stay and stay
   non-overridable.
3. Output is bounded through the tool-result artifact path: the event
   carries a preview that names what it withheld, and the full output is
   readable at its `tool:` address.
4. The sandboxed and unsandboxed variants keep one model-facing name,
   selected by the runtime as today.
5. The dirty-tree guard from section 5 applies.

`bash` is the capability surface for everything the slim set does not
name: search runs `rg`, file listing runs `ls`, fetch runs `curl`, git
runs `git`.
This is the Pi position, and the fork already holds the two properties
that make it safe where Pi is unsafe: the fail-closed parser and bounded
artifacts.

### 3.5 `delegate`

Hands a task to another executor and returns its final message with a
disclosure record.

Input contract:

| Field | Meaning |
| --- | --- |
| `executor` | The named target. One of the installed external agents (for example `codex-acp`, `claude-acp`, `exo`), `native`, or an engine lane name. |
| `task` | The instruction for the delegate. |
| `label` | A short human-readable purpose line for the panel. |
| `session` | Optional. A prior delegate session address, for a follow-up turn on the same delegate. |

Output contract:

1. The delegate's final message.
2. A typed `ExecutorDisclosure` record: runtime class, agent id,
   provider, model, and run reference where one applies
   (`OMEGA-DELTA-0101`).
3. A session address the parent can `read` for the bounded transcript
   and can pass back as `session`.

Laws, all already mechanical in the fork:

1. **Never substitute.** A request for `codex-acp` runs Codex or fails
   with a typed reason (`OMEGA-DELTA-0061`).
   `auto` is not a valid `executor` value at this revision.
   Executor choice above the tool belongs to the router and the person.
2. **Presence, not configuration.** Available external executors come
   from detection of what is installed (`OMEGA-DELTA-0095`,
   `crates/omega_agent_detect`).
3. **No second home.** The delegate runs on its own credentials and
   configuration. Omega copies nothing (`OMEGA-DELTA-0025`).
4. **Depth stays bounded.** `MAX_SUBAGENT_DEPTH = 1` holds: the root
   thread delegates, a delegate does not.
5. **Parent-only transcripts.** Only the spawning thread reads the
   delegate transcript (`OMEGA-DELTA-0060`).
6. **The panel can find it.** An external delegate session registers so
   the person can open it (`OMEGA-DELTA-0112`).
7. **Honest capacity failures.** A provider-capacity failure surfaces as
   its own class, for example `account_exhausted` or
   `account_rate_limited`, never as a generic execution error.
8. **Engine lanes are requests, not ownership.** A delegate call that
   names an engine lane sends a typed command over the framed
   `omega-effectd` protocol.
   The engine stays the run authority and the receipt source.
   When the engine is unavailable, the call fails closed with a typed
   reason.
9. **Cancellation propagates.** Cancel on the parent cancels running
   delegates.
10. **Khala stays reserved.** `delegate` targets local executors and
    engine lanes at this revision.
    Khala lanes arrive only through the reserved `OMEGA-AGENT-K1`
    packets with their own owner admission.
11. **Exo chains disclose fully.** Exo is an admitted delegate target,
    and Exo can itself host a vendor executor inside its sandboxes.
    A delegated Exo turn that hosts a vendor executor names the full
    chain in its disclosure record: Omega Agent, then Exo, then the
    hosted runtime and model (`OMEGA-AGENT-AC-19`).

`delegate` supersedes the model-facing `spawn_agent` name.
The implementation reuses `SpawnAgentTool`, `subagent_executor`, and
`external_subagent_sessions` directly.
The flag-gated sibling-thread tools (`create_thread`,
`list_agents_and_models`) stay out of the slim profile.
Independent parallel work remains a person-driven or engine-driven act at
this revision.

## 4. The system prompt contract

The slim prompt is a new template beside the inherited one, selected by
the slim profile.

1. The prompt contains, in order: identity, the communication contract,
   tool use for the five tools, the work-safety law, task execution, the
   delegation section, system information, the sandbox section when
   sandboxing is on, the skills catalog when skills exist, and the
   instruction-file section (`AGENTS.md`, project rules).
2. The prompt drops the upstream sections that exist for the wide tool
   set: the mermaid formatting essay, the grep-versus-find guidance, the
   LSP workflows, and the editor-specific instructions.
3. The upstream sentence "Keep user work safe. Do not overwrite, remove,
   or revert changes you did not make" stays, and section 5 gives it
   mechanical teeth.
4. **The prompt is measured.** The rendered template with an empty
   project context has a byte ceiling asserted by a mechanical check.
   The proposed ceiling at this revision is 8,192 bytes before the
   instruction-file and skills sections.
   A prompt growth past the ceiling is a deliberate delta, not drift.
5. Project context stays prompt-cache-stable: the context refreshes only
   when the model-visible value changes.

## 5. The work-loss guard

This section turns the
[oopsiewoopsies record](../oopsiewoopsies/2026-07-27-git-checkout-destroyed-uncommitted-work-twice.md)
into tool law.
The incident class: a file-scoped git restore through the shell destroyed
uncommitted work twice, and three warning signals passed unread.

### 5.1 The dirty-tree guard on `bash`

1. The guard classifies these command families as data-loss commands:
   `git checkout -- <path>`, `git checkout <ref> -- <path>`,
   `git restore`, `git stash` with `drop`, `pop`, or `clear`,
   `git reset --hard`, and `git clean` with `-f` or `-d`.
2. Before such a command runs, the guard asks the repository whether the
   working tree holds uncommitted changes in the affected scope.
3. A clean scope runs without friction.
   A dirty scope triggers the confirm-on-data-loss flow, and the prompt
   names the files whose changes the command would discard.
4. The decision is a typed result in the transcript either way.
   A guard that no-ops silently verifies nothing.
5. The classification uses the existing fail-closed command-chain
   parser.
   A chain the parser cannot decompose, containing one of these command
   names, denies with the parser's standing rule.
6. The guard is policy above the permission ladder, not a replacement:
   hardcoded denials and user rules keep their precedence.

### 5.2 Snapshot before mutation

1. The per-user-message git checkpoint stays and remains the coarse
   restore point.
2. The agent's own undo is always snapshot-based: restore reads a
   checkpoint the agent took, never the git index.
   The agent does not use `git checkout`, `git restore`, or `git stash`
   as an undo mechanism in its own reasoning.
   The prompt states this, and the guard enforces it against forgetting.
3. When the agent needs a mutate-verify-revert loop, for example
   mutation testing, the loop copies the file aside and copies it back.
   The skill text for verification work records this pattern.

### 5.3 Staleness and dirty buffers

The `edit` staleness refusal and the dirty-buffer prompts from section 3
are part of this guard and land with it.

### 5.4 Delegate coverage

A delegate executor runs outside these tools and outside this guard.
The disclosure record makes that visible: work a delegate performed is
attributed to the delegate.
The guard law applies to the slim agent's own five tools at this
revision.
An extension of dirty-tree protection to delegated executors is a
separate packet against the harness lane, not a silent scope claim here.

## 6. What leaves the model's view

The slim profile is a new profile in `assets/settings/default.json`, and
it becomes the default profile of the slim agent.
No tool is deleted.
The current wide profile stays available under an explicit name for
people who want the editor-integrated tool set.

| Current tool | Destination |
| --- | --- |
| `read_file`, `read_tool_result_artifact`, `read_subagent_transcript`, `skill` | Folded into `read` |
| `write_file` | Renamed surface `write` |
| `edit_file` | Renamed surface `edit` |
| `terminal` (and sandboxed twin) | Renamed surface `bash` |
| `spawn_agent` | Renamed and widened surface `delegate` |
| `grep`, `find_path`, `list_directory` | `bash` (`rg`, `fd`, `ls`) |
| `fetch` | `bash` (`curl`). The sandbox host-grant flow stays on the shell path |
| `search_web` | Dropped from the slim profile. The tool already refuses non-cloud providers, and Omega ships no cloud provider, so it is dead weight in the fork today |
| `diagnostics` | Wide profile only at this revision. Section 13 records the cost |
| `create_directory`, `delete_path`, `move_path`, `copy_path` | `bash` (`mkdir`, `rm`, `mv`, `cp`), under the section 5 guard and the standing hardcoded denials |
| `rename_symbol`, `find_references`, `go_to_definition`, `get_code_actions`, `apply_code_action` | Already feature-flag-gated off. Wide profile only |
| `create_thread`, `list_agents_and_models` | Already feature-flag-gated off. Not in the slim profile |
| MCP tools (`context_server`) | Off in the slim profile: `enable_all_context_servers: false`. The crate stays for the wide profile |

Naming is a registration concern, not a rewrite: the model-facing names
`read`, `write`, `edit`, `bash`, `delegate` register over the existing
tool implementations.
Rust type names do not need to change, which keeps the upstream rebase
surface small.

## 7. Relationship to the router

The router landed and stays.
Routing and delegation are two different questions:

1. The router answers: which executor owns this thread.
   Per-thread pins, fail-closed decisions, the route journal, and the
   disclosure line stay exactly where they landed (`OMEGA-DELTA-0029`,
   `0033`, `0035`, `0055`).
2. `delegate` answers: which executor runs this subtask, inside a turn
   the slim agent owns.

This is the compose reading from the audit, and it is the recommendation
of this specification.
The collapse reading, which retires per-thread routing in favor of
delegation only, deletes landed delta-checked machinery for no capability
gain at this revision, and this specification rejects it.
The owner can overrule on the admission.

The executor-class enum stays closed at three.
`delegate` introduces no fourth class: an external delegate is the
external ACP class, and an engine delegate is the engine-lane class.

## 8. Settings and profiles

1. The slim profile ships in `assets/settings/default.json` with exactly
   the five tools and `enable_all_context_servers: false`.
2. The slim profile becomes `default_profile`.
   The change is a numbered delta with a check.
3. The current write profile stays, renamed to say what it is, for
   example `editor`.
4. `agent.default_model` stays `google/gemini-3.6-flash` at this
   revision.
   The shape record notes that no delta check protects the model string.
   That check should land with this program.
5. The restricted-workspace downgrade path is vestigial in Omega
   (`OMEGA-DELTA-0001`) and needs no slim variant.

## 9. Deltas required

Each row lands as a numbered delta in `OMEGA_DELTAS.md` with a mechanical
check in `crates/omega_deltas`, and every check is watched failing before
it is trusted.
Numbers come from the delta cleanup lane, per the shape record's standing
finding on duplicate identifiers.

| Proposed delta | Content | Check sketch |
| --- | --- | --- |
| Slim profile is the default | The default profile of the slim agent holds exactly `read`, `write`, `edit`, `bash`, `delegate` | Parse `default.json`, assert the profile tool set equals the five names |
| The slim prompt is measured | The rendered slim template respects the byte ceiling | Render with empty context, assert the ceiling |
| The dirty-tree guard exists | File-scoped git restore commands confirm on a dirty tree | Unit tests over the classifier: each family, chained forms, clean-tree pass-through, unparseable-chain denial |
| The guard cannot be silent | Every guard decision is a typed transcript result | Assert the decision type renders in the transcript path |
| `delegate` never substitutes | A named executor runs or the call fails | The existing `OMEGA-DELTA-0061` check extends to the `delegate` registration |
| `read` keeps parent-only transcript access | The fold does not widen `OMEGA-DELTA-0060` | The existing check extends to the `read` registration |
| `search_web` leaves the slim surface | The dead tool is not model-visible in the slim profile | Assert absence in the profile allowlist |
| The default model has a check | `google/gemini-3.6-flash` is asserted as a string, closing the shape-record gap | Assert provider and model in `default.json` |

## 10. Packet plan

The program extends the `OMEGA-AGENT` ledger.
Every packet cites the revised ProductSpec.
Wave numbering follows the master delegation plan's style: packets in one
wave can run in parallel when they do not share hot files.
The live claim ledger is the omega issue set under epic omega#112.

| Packet | Issue | Goal | Depends on |
| --- | --- | --- | --- |
| SLIM-00 | complete | ProductSpec revision 2, landed | none |
| SLIM-01 | omega#113 | The slim profile, the five-tool registration, and the tool-name surface | SLIM-00 |
| SLIM-02 | omega#114 | The slim system prompt template with the byte-ceiling check | SLIM-01 |
| SLIM-03 | omega#115 | The `read` fold: files, artifacts, transcripts, skill bodies under one tool | SLIM-01 |
| SLIM-04 | omega#116 | The dirty-tree guard and the typed guard transcript | SLIM-00, parallel to SLIM-01 |
| SLIM-05 | omega#117 | The `delegate` surface: rename, disclosure record in the result, session addresses, engine-lane command path | SLIM-01 |
| SLIM-06 | omega#118 | Behavior contracts and the delta ledger sweep for the program | SLIM-01 through SLIM-05 |

Hot files: `assets/settings/default.json` (SLIM-01), the prompt templates
(SLIM-02), `crates/agent/src/tools.rs` (SLIM-01, SLIM-03, SLIM-05).
One agent owns a hot file at a time.

## 11. Acceptance criteria

- **SLIM-AC-01:** The slim profile exposes exactly five tools to the
  model: `read`, `write`, `edit`, `bash`, `delegate`.
- **SLIM-AC-02:** The rendered slim prompt with empty project context is
  within the admitted byte ceiling, and a mechanical check asserts it.
- **SLIM-AC-03:** A file-scoped git restore command against a dirty
  scope does not run without a typed confirm, and the confirm names the
  files at risk.
- **SLIM-AC-04:** A `delegate` call for a named executor either runs
  that executor or fails with a typed reason. No substitution occurs.
- **SLIM-AC-05:** Every `delegate` result carries a typed
  `ExecutorDisclosure` record and a readable session address.
- **SLIM-AC-06:** `read` on a delegate session address obeys the
  parent-only law.
- **SLIM-AC-07:** An `edit` against a file that changed after the last
  `read` refuses with a typed staleness result.
- **SLIM-AC-08:** No packet deletes a registered tool, forks
  `AcpThread`, adds a durable run store to GPUI, or copies an external
  agent's credentials or home.
- **SLIM-AC-09:** The engine stays the run authority for every
  engine-lane delegation, and an unavailable engine fails closed with a
  typed reason.
- **SLIM-AC-10:** Every default change in this program is a numbered
  delta with a mechanical check that was watched failing.

Falsifiers, stated so the program can be caught:

1. A sixth tool appears in the slim profile without a new admission.
2. A guard decision that does not render in the transcript.
3. A `delegate` that silently ran a different executor than named.
4. A prompt-size regression that lands without a delta.
5. An implementation packet that lands before the SLIM-00 admission.

## 12. The ProductSpec revision

Revision 1 admits Omega Agent as a router that owns no execution.
Revision 2 landed on 2026-07-27, on the owner direction of that day, and
does exactly this:

1. It keeps the router shape and every routing acceptance criterion.
2. It names the basic agent — the slim five-tool native executor — as
   the default executor of Omega Agent, with the five-tool law
   (`OMEGA-AGENT-AC-15`) and the work-loss law (`OMEGA-AGENT-AC-20`) as
   acceptance criteria.
3. It adds out-of-box reliability as a criterion: the baseline journey
   completes on the default `google/gemini-3.6-flash` direct provider
   with no harness installed (`OMEGA-AGENT-AC-16`, `AC-17`).
4. It restates `OMEGA-AGENT-AC-04` unchanged: the executor-class set
   stays exactly three.
5. It adds the `delegate` disclosure obligation as a record obligation
   (`OMEGA-AGENT-AC-18`), and names Exo as an admitted delegate target
   with the full-chain disclosure rule (`OMEGA-AGENT-AC-19`).
6. It leaves the Omega Nostr identity signing question open and
   owner-reserved, unchanged from revision 1.

## 13. Considerations and risks

1. **Rebase pressure.** The five-tool surface is registration and
   profile policy, so the upstream diff stays small.
   The one real divergence is the slim prompt template, which is a new
   file, not an edit to the upstream template.
2. **Model compatibility.** Some models lean on dedicated search tools
   and degrade when search must go through a shell.
   The eval harness in `crates/eval_cli` should compare the slim profile
   against the wide profile on the same tasks before the default flips.
   A default-profile flip that skips this comparison is a guess.
3. **The diagnostics cost.** Dropping `diagnostics` from the slim set
   loses the one tool that reads editor truth the shell cannot see
   cheaply.
   Three mitigations exist, in preference order.
   The person switches to the wide profile.
   A later admission adds a diagnostics section to tool results on
   edited files.
   A sixth tool is admitted deliberately.
   This specification holds the five-tool law and records the cost.
4. **Prompt-injection surface of `delegate`.** A delegated executor
   reads repository content the parent did not vet.
   The existing boundaries hold: the delegate returns one final message,
   transcripts are bounded and parent-only, and no delegate output closes
   work without typed outcomes.
   Community-sourced or untrusted-origin work must not route onto
   full-allow local executors, per the standing risk register.
5. **The `bash` fetch path.** Folding `fetch` into `curl` moves URL
   access from a permission-gated tool to the shell.
   Under sandboxing, the host-grant flow already covers shell network
   access.
   Unsandboxed, Omega already chose allow-by-default for capability.
   The guard laws of this specification are about data loss, not network
   reach.
   A network-posture change is out of scope here.
6. **Skills stay load-bearing.** The skill catalog in the prompt plus
   `read` on skill locations replaces the dynamic `skill` tool.
   The skill discovery machinery, precedence, and budgets stay unchanged.
   Self-extension in the Pi sense arrives through skills the agent
   writes with `write`, which the existing skill system already supports.
7. **MCP posture.** The slim profile ships with context servers off,
   for the Pi reason: session-start tool loading breaks cache reuse and
   grows the prompt.
   The wide profile keeps MCP.
   This is profile policy, not crate removal.
8. **Zero base is the natural host.** Zero-base mode is the default
   Omega window: one thread and a composer.
   The slim agent is the right default face for that surface, and the
   wide profile remains one selector away in the full editor.
9. **Delegation observability.** Only the final message returns to the
   parent, which keeps parent context small but hides delegate progress.
   The panel can already find external delegate sessions
   (`OMEGA-DELTA-0112`).
   A live delegate status in the thread view is UI work that can follow
   without a change to the tool contract.
10. **The disclosure record must stay a record.** The shape record's
    condition binds `delegate` output directly: typed fields first, label
    rendering second, so a later Nostr-signed record needs a signer, not
    a rewrite.
11. **Documentation language.** Program documentation lands in
    `docs/omega-agent/` in this repository and in the omega repository's own
    docs where the fork's discipline requires. These internal records can use
    normal technical language and do not run the public STE checks.

## 14. Open owner questions

1. **Admission.** Answered. The owner direction of 2026-07-27 set the
   basic-agent shape, and ProductSpec revision 2 records it with the
   five-tool law and the work-loss law.
2. **Compose versus collapse.** This specification recommends keeping
   the router beside the slim executor. Does the owner accept the
   compose reading?
3. **Diagnostics.** Does the owner accept the diagnostics cost in the
   slim profile, or admit a sixth tool now?
4. **Delegate targets at v1.** This specification includes engine lanes
   in the first `delegate` surface. The owner can cut v1 to external
   executors only and land engine lanes as a follow-up packet.
5. **The wide profile name.** `editor` is proposed. The owner can name
   it otherwise.
6. **The prompt ceiling.** 8,192 bytes is proposed. The owner can set a
   different number. The check enforces the admitted number.
