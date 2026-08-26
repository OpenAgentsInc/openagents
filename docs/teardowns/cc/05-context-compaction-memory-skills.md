# Teardown 05: Context Window Management, Compaction, Memory & Skills

**Claude Code** (`~/work/projects/repos/cc`) vs **OpenAgents Coder** (`packages/openagents-cli/src/`).
Note: the two counterpart filenames given in the task brief (`coder-history.ts`, `coder-token-economy.ts`) do not exist; the actual nearest neighbors are `coder-transcript.ts`, `coder-resume.ts`, `coder-thread.ts`, `coder-tool-budget.ts`, and `coder-memory.ts`. This report analyzes what is really there.

## Component / Subsystem Breakdown

| Concern | Claude Code | OpenAgents Coder |
|---|---|---|
| Prompt history | `history.ts` (464 ln) → `~/.claude/history.jsonl` | **absent** (resume replays server events instead) |
| Compaction | `services/compact/*` (~3,700 ln, 8 modules) | **absent entirely** |
| Micro-compaction | `microCompact.ts`, `apiMicrocompact.ts`, snip tool | per-result caps only (`coder-tool-budget.ts`) |
| Session scratch memory | `services/SessionMemory/sessionMemory.ts` (495 ln) | **absent** |
| Long-term memory | `memdir/` (MEMORY.md + daily logs, ~1,700 ln) | `coder-memory.ts` + `memory/` (signed engram ledger, ~3,200 ln) |
| Memory recall | `findRelevantMemories.ts` (LLM selector) | knowledge rail attach (`coder-knowledge.ts`), no selector |
| Skills | `skills/loadSkillsDir.ts` (1,086 ln) + bundled | `coder-skills.ts` (383 ln) |
| Durable transcript | `utils/messages.ts` serialization | `coder-transcript.ts` (server-pushed event log) |

## Claude Code Implementation Details

**Compaction is a layered state machine**, not one mechanism:

1. **Threshold math** (`autoCompact.ts`): effective context window = advertised window minus `min(maxOutputTokens, 20_000)` reserved for the summary itself (calibrated on p99.99 summary output of 17,387 tokens). Warning/error buffers and a blocking limit (`window − MANUAL_COMPACT_BUFFER`) drive UI state via `calculateTokenWarningState`. Env (`DISABLE_AUTO_COMPACT`, `CLAUDE_CODE_AUTO_COMPACT_WINDOW`) and user config gate it; recursion guards refuse compaction when `querySource` is `session_memory` or `compact` (forked agents would deadlock); feature flags route around *reactive* compact (catching API 413s after the fact) and *context collapse* (90% commit / 95% blocking-spawn headroom management).
2. **Full compaction** (`compact.ts`, 1,705 ln): runs the summary through a *forked agent* with tools disabled (`NO_TOOLS_PREAMBLE`), `<analysis>`-tagged reasoning, then `<summary>` extraction (`formatCompactSummary`). Post-compact restoration re-injects bounded context: ≤5 files / 50k tokens total / 5k each; skills ≤25k / 5k each; plan-mode and async-agent attachments. Boundary annotation preserves `tool_use`/`tool_result` pairing invariants. A circuit breaker (`MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES`) stops doomed retry loops; `RecompactionInfo` detects compaction-of-a-compacted-chain.
3. **Partial compaction**: three prompt variants (recent-window, up-to-boundary, base), with index adjustment that survives orphaned `tool_use_id`s split across same-`message.id` blocks.
4. **Micro-compaction** (`microCompact.ts`): time-based clearing of old results for a fixed `COMPACTABLE_TOOLS` set (Read/Bash/Grep/Glob/WebSearch/WebFetch/Edit/Write), image downsampling (2,000-token cap), and a cache-safe variant that stages edits so the Anthropic prompt cache is not invalidated wholesale (`promptCacheBreakDetection.ts` notifies consumers). The **snip tool** injects `[id:...]` tags so the model can excise messages itself, feeding freed tokens back into threshold estimates (`snipTokensFreed`).
5. **Session memory**: a post-sampling hook forks a subagent every N tool calls to maintain a markdown notes file without interrupting the main loop, tracking extraction token spend and init/update thresholds separately.

**Prompt history** (`history.ts`): append-only JSONL under the config home (cap 100 entries), reverse-read via `readLinesReverse`, advisory lockfile, batched flush with cleanup-registry drain. Large pastes are content-hashed out to a paste store and referenced inline as `[Pasted text #N +X lines]`; `expandPastedTextRefs` rehydrates them on submit. `removeLastFromHistory` implements an undo protocol (pending-buffer pop, else timestamp skip-set) so Esc-rewind doesn't double-show text in Up-arrow history. Tmux-spawned verification sessions opt out via env.

**Memory (`memdir/`)**: `MEMORY.md` is a capped live index (200 lines **and** 25 KB — dual truncation with explicit warnings naming which cap fired), backed by typed memory files (user/project/how-to references with frontmatter, drift caveats, "what not to save" guidance) and date-named daily logs (`logs/YYYY/MM/DD.md`) distilled by a nightly `/dream` skill. Recall is a two-stage pipeline: `scanMemoryFiles` builds a header manifest, then a cheap Sonnet `sideQuery` picks ≤5 relevant files, excluding `MEMORY.md` (already loaded), filtering against recently used tools while keeping their gotcha-docs, and de-duplicating already-surfaced paths.

**Skills (`loadSkillsDir.ts`)**: directory (`name/SKILL.md`) and single-file formats; rich frontmatter (`allowed-tools`, `disable-model-invocation`, `context: inline|fork`, `agent`, `model`, `hooks`, `whenToUse`, `argumentHint`); memoized command cache with explicit invalidation (`clearSkillCaches`); dynamic directory registration mid-session (`addSkillDirectories`, `activateConditionalSkillsForPaths`, conditional-skill counting); frontmatter token estimation for context accounting; programmatic bundled skills whose auxiliary `files` extract to disk on first invocation so Read/Grep work identically to disk skills.

## OpenAgents Coder Implementation State

**There is no compaction.** Nothing in `src/` matches "compact" except display formatting. The architecture instead *prevents* bloat and *defers* survival:

- `coder-tool-budget.ts` (190 ln) caps each tool result per model family (exhaustive `Record<ToolFamily, FamilyBudget>`, so adding a family forces a budget decision), with an explicit `charactersPerToken` (3.6 default) and fail-closed truncation: a cut result states it was cut, by how much, against which budget — echoing INVARIANTS.md. This addresses the *cause* micro-compaction treats (oversized results) but never removes accumulated turns.
- `coder-transcript.ts` persists every entry to the server event log with ordered queueing, retries, and one-time persistent-failure notice; `coder-resume.ts` replays both UI entries and wire messages (paged `GET /threads/{id}/events`), marking standing context already-delivered so replay isn't paid for twice. Durability lives off-process rather than in a summary.
- Tier switching (`coder-tiers.ts`, `coder-session.ts`) hands the wire transcript to the next source, which recomposes its own system anchor — a lane-change escape hatch, not a shrink.
- `coder-thread.ts` meters turn usage against server-grant limits (`remaining`, `thread_quota_reached`) — this is *quota*, not context-window management; nothing tracks tokens against the active model's window.
- Round-cap backstop on tool loops ("was six") bounds runaway turns.

**Memory is cryptographically stronger but differently shaped.** `coder-memory.ts` + vendored `memory/` (2,645 ln) implement an append-only JSONL ledger of NIP-AE-shaped engrams at `~/.openagents/memory/engrams.jsonl`: secp256k1 Schnorr signatures (key reused from the HMAC era, migrated), verified event ids, supersession chains for corrections, redaction gate before signing, pure/idempotent projections rebuilt from the log (never stored), a sync queue that can never block or fail a turn, subagent outcome harvest into parent heuristics (#227), and a bounded redacted advisory block seeded into child prompts (#226). What's missing relative to `memdir/`: no session-scoped scratch notes, no capped entrypoint index, no daily-log/dream distillation loop, no LLM relevance selector over the ledger (projection/ranking exist, but nothing asks "which memories matter for *this* query"), and nothing injects ledger content into the main session's own standing context — only children receive it.

**Skills** share the format (SKILL.md, YAML frontmatter, description-as-selection-key) but implement the cheaper half of the contract: catalog rides in the `skill` tool description; bodies load on demand (progressive disclosure, same idea as CC's ToolSearch). Three directories, nearest-first, first-claim-wins with builtins shipped last so repos/users can shadow them; hand-rolled folded/literal block scalar parsing; `/skills` toggle; `auto:` field. Missing: `allowed-tools` scoping, hook attachment, `context: fork` execution, `agent` binding, `disable-model-invocation`, dynamic mid-session directories, conditional path activation, and frontmatter token estimation. Bundled skills ship as plain files beside compiled output, not extracted-with-files definitions.

**`device-authorization-store.ts`** (assigned counterpart) is unrelated to context: an Effect-gen file store of pending OAuth device-flow grants keyed by origin at `~/.config/openagents/device-authorizations.json`, Schema-validated v1, atomic rename writes, in-memory test layer. Solid; no gap worth porting from CC.

## Gap Analysis

The decisive architectural trade-off: **CC spends ~5,500 lines managing a window it owns; Coder owns no window** — context composition belongs to whichever lane answers (hosted proxy pins the model; local Ollama lane composes its own anchor). Consequences: (a) a long Coder thread simply degrades or hits the vendor wall with no recovery path; (b) Coder's signed-ledger memory is more durable and tamper-evident than anything in `memdir/`, yet reaches neither the main loop nor recall-at-query-time; (c) skills lack permission/fork semantics, so a skill cannot safely elevate tools or isolate context; (d) resume-via-replay substitutes for prompt history but offers no cross-*session* input reuse (no paste store equivalent).

## Actionable Porting Recommendations

1. **Port the autocompact skeleton first** (highest value): `getEffectiveContextWindowSize` (window − reserved-summary), threshold/warning state machine, and the consecutive-failure circuit breaker into a new `coder-compact.ts`. Trigger from `ThreadReplySource` between rounds using `turnUsage.promptTokens`.
2. Implement `compactConversation` as a **delegate child** (reuse `coder-delegate.ts`) with tools withheld and a `<summary>`-extracting prompt adapted from `services/compact/prompt.ts`; rebuild wire transcript as `[summarized]` + tail, preserving `tool_use`/`tool_result` pairing via `adjustIndexToPreserveAPIInvariants` logic.
3. Add **post-compact restore budgets** (files/skills/plans with per-item and total caps) — constants and structure port directly.
4. Port `removeLastFromHistory`'s undo semantics and the paste-ref store into a small `~/.openagents/coder-history.jsonl` (cap 100) for Up-arrow continuity across sessions.
5. Bridge memory into the main loop: render top-ranked projected heuristics as a bounded standing-context block (the child-facing `buildSubagentMemoryContext` already exists — generalize it), and add a `findRelevantMemories`-style selector; on the local tier it can run against the free Ollama lane.
6. Skill parity: add `allowed-tools` enforcement in `coder-tools.ts`, `disable-model-invocation`, and frontmatter token estimation; defer `context: fork` until delegation is cheap enough to fork per skill.
