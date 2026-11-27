# 19. Remember a fact, then revise it

> **Status: partly implemented.** OpenAgents implements `#authority` memory
> trust boundaries, entrypoint truncation, memory-fork sandboxing, Auto-Dream
> consolidation locks, and team secret scanning in `openagents/core/memory.coax`
> (adopted by `openagents/runtime/oa-worker.coax`), alongside the hash-chained
> remember/revise/forget event log in `coax/lib/agent/engine/memory-log.coax`
> and `coder/tui/memory.coax` (`coder remember`/`revise`/`forget`/`recall`).
> Desktop UI inspection/revision of remembered facts remains open (see
> [Chapter 42](ch042-beyond-the-desktop-app.md)).

An agent that cannot remember project conventions wastes turns rediscovering
them; an agent that remembers blindly poisons future turns with stale commands,
leaked secrets, or untrusted repository instructions. Memory therefore requires
two layers of `#authority` discipline:

1. **Runtime memory governance (`openagents/core/memory.coax`, compiled into
   `openagents/runtime/oa-worker.coax`):**
   - **Path trust boundary:** `flawed_auto_mem_dir_source_trusted` refuses
     `projectSettings` overrides of `autoMemoryDirectory` (preventing a hostile
     repo from redirecting memory writes into `~/.ssh`), and
     `flawed_validate_memory_path` rejects traversal sequences (`..`).
   - **Least-privilege memory forks (`flawed_mem_fork_tool_allowed`):**
     background memory extraction agents can edit files inside the memory
     directory but are blocked from writing outside it.
   - **Dual entrypoint budget (`flawed_truncate_memory_entrypoint`):** caps
     `MEMORY.md` at 200 lines and 25 KB snapping cleanly at newline boundaries.
   - **Auto-Dream 3-gate lock (`flawed_dream_eval_gates`):** coordinates
     periodic background memory consolidation under time, session-count, and
     stale-lock gates.
   - **Fail-closed team secret scanner (`flawed_team_mem_has_secret`):** blocks
     AWS keys, tokens, and private keys before shared team memory sync.
2. **Hash-chained fact ledger (`coax/lib/agent/engine/memory-log.coax` and
   `coder/tui/memory.coax`, backed by `psionic/lib/harness/memory.coax`):**
   - Persists `~/.openagents/coder/memory/events.jsonl` via one canonical codec
     (`mem_line` / `mem_unline`, checked by `ledger-profile.coax` against
     [`proto/coder/consent-memory-ledgers-v1.json`](../../proto/coder/consent-memory-ledgers-v1.json)).
   - Links every fact to either a cited document span (`MmSource.cite`) or a
     session event digest (`MmSource.event`).
   - Detects conflicting writes on the same key (`MmServe.conflict`) until
     explicitly settled with `mm_revise(..., MmReason.resolve, ...)`, refuses
     no-op identical revisions (`same`), marks cited facts `stale` the instant
     the underlying file bytes change, and scrubs forgotten fact bytes from the
     log (`mm_forget`) while keeping `mm_verify` and `mm_replay` intact.

## Profile

This chapter's profile is `openagents-memory-v1 (openagents/core/memory.coax + memory-log.coax); coder remember/revise/forget/recall on disk`:

- Untrusted `projectSettings` memory directory overrides, `..` path escapes,
  out-of-sandbox memory fork writes, and secret-bearing team entries are
  refused under `#authority`.
- Conflicting remembers surface `conflict` until explicitly resolved; stale
  citations refuse old-byte updates and recover only against the current
  passage bytes; `mm_forget` scrubs bytes (`holds_vim=false`, `holds_helix=false`)
  while preserving SHA-256 chain verification and exact table replay.

## Worked example

The runnable example [memory-lifecycle.coax](../examples/memory-lifecycle.coax)
exercises both `openagents/core/memory.coax` and the hash-chained event log
backing `coax/lib/agent/engine/memory-log.coax` and `coder/tui/memory.coax`.

<!-- source: coder/examples/memory-lifecycle.coax example -->
```jai
main :: () {
  // Part A: OpenAgents Core memory governance (`openagents/core/memory.coax`)
  // Project settings cannot hijack autoMemoryDirectory; team memory blocks leaked secrets
  proj_dir_ok := flawed_auto_mem_dir_source_trusted(FLAWED_SET_SRC_PROJECT);
  user_dir_ok := flawed_auto_mem_dir_source_trusted(FLAWED_SET_SRC_USER);
  path_safe := flawed_validate_memory_path("/home/u/.claude/projects/openagents/memory");
  path_escape := flawed_validate_memory_path("/home/u/.ssh/../.ssh");
  fork_edit := flawed_mem_fork_tool_allowed(FLAWED_MTOOL_EDIT, false, false, true, true);
  fork_write_outside := flawed_mem_fork_tool_allowed(FLAWED_MTOOL_WRITE, false, true, true, true);
  dream_verdict := flawed_dream_eval_gates(true, 24, 24, FLAWED_DREAM_SCAN_COOLDOWN_MS, 5, 5, FLAWED_DREAM_LOCK_STALE_MS, true);
  secret_caught := flawed_team_mem_has_secret("token AKIA0000000000000000");
  print("core memory: proj_dir=% user_dir=% path_ok=% path_escape=% fork_edit=% fork_outside=% dream_run=% secret_caught=%\n",
    proj_dir_ok, user_dir_ok, path_safe, path_escape, fork_edit, fork_write_outside,
    dream_verdict == FLAWED_DREAM_RUN, secret_caught);

  // Part B: Hash-chained event log (`memory-log.coax` / `coder remember`, `revise`, `forget`)
  ev: [16] MmEvent;
  by: [512] u8;
  fs: [16] MmFact;
  l := [1] MmLog.[MmLog.{events = ev[..], bytes = by[..]}];
  t := [1] MmTable.[MmTable.{facts = fs[..]}];
  ids := MC_IDS; hd := MC_HEADS; old := MC_OLD; new := MC_NEW;
  p := mc_passages(ids[..], hd[..], old[..]);
  q := mc_passages(ids[..], hd[..], new[..]);
  me := MC_ME;

  // 1. Remember a cited build fact + two conflicting user editor facts
  s1 := mm_remember(l[..], t[..], mc_cite(old[0], 0, 23), p);
  s2 := mm_remember(l[..], t[..], mc_event("editor", MmScope.user, 1, "vim", 161, 0, 1000), p);
  s3 := mm_remember(l[..], t[..], mc_event("editor", MmScope.user, 1, "helix", 162, 0, 1000), p);
  lk_conflict := mm_look(t[0], "editor", MmScope.user, me, 3, p);
  print("remember: build=% ed1=% ed2=% look_editor=%\n",
    mm_st(s1), mm_st(s2), mm_st(s3), mm_sv(lk_conflict.serve));

  // 2. Resolve conflict by explicit revision; duplicate identical revision is refused
  s4 := mm_revise(l[..], t[..], mc_event("editor", MmScope.user, 1, "helix", 163, 0, 1000), MmReason.resolve, p);
  s4_dup := mm_revise(l[..], t[..], mc_event("editor", MmScope.user, 1, "helix", 163, 0, 1000), MmReason.correction, p);
  print("revise: resolve=% duplicate=%\n", mm_st(s4), mm_st(s4_dup));

  // 3. Cited README changes -> old fact goes stale; revising from old bytes is refused; new bytes succeed
  lk_stale := mm_look(t[0], "build", MmScope.project, me, 5, q);
  s5_old := mm_revise(l[..], t[..], mc_cite(old[0], 0, 23), MmReason.update, q);
  s5_new := mm_revise(l[..], t[..], mc_cite(new[0], 0, 30), MmReason.update, q);
  lk_fresh := mm_look(t[0], "build", MmScope.project, me, 6, q);
  print("citation drift: before_update=% stale_revise=% fresh_revise=% after_update=%\n",
    mm_sv(lk_stale.serve), mm_st(s5_old), mm_st(s5_new), mm_sv(lk_fresh.serve));

  // 4. Forget scrubs bytes from the log, tombstones the key, and keeps the hash chain verifiable
  s6 := mm_forget(l[..], t[..], "editor", MmScope.user, 1, q);
  lk_gone := mm_look(t[0], "editor", MmScope.user, me, 7, q);
  v := mm_verify(l[0]);
  gs: [16] MmFact;
  u := [1] MmTable.[MmTable.{facts = gs[..]}];
  rp := mm_replay(l[0], u[..]);
  print("forget & replay: forget=% look_editor=% holds_vim=% holds_helix=% verify=% replay=% same=%\n",
    mm_st(s6), mm_sv(lk_gone.serve), mc_holds(l[0], "vim"), mc_holds(l[0], "helix"),
    mm_st(v.status), mm_st(rp.status), mc_same_table(t[0], u[0]));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/memory-lifecycle.coax
```

```text
core memory: proj_dir=false user_dir=true path_ok=true path_escape=false fork_edit=true fork_outside=false dream_run=true secret_caught=true
remember: build=ok ed1=ok ed2=ok look_editor=conflict
revise: resolve=ok duplicate=same
citation drift: before_update=stale stale_revise=source fresh_revise=ok after_update=served
forget & replay: forget=ok look_editor=forgotten holds_vim=false holds_helix=false verify=ok replay=ok same=true
```

The checked fixture is [memory-lifecycle.out](../examples/memory-lifecycle.out).

## Live CLI & protocol fixture

`coder remember`, `coder revise`, `coder forget`, and `coder recall` operate on
`~/.openagents/coder/memory/events.jsonl` through
[`coder/tui/memory.coax`](../../coder/tui/memory.coax) and
[`coax/lib/agent/engine/memory-log.coax`](../../coax/lib/agent/engine/memory-log.coax),
gating outbound recalled facts through [Chapter 18](ch018-consent-before-sharing.md)'s
consent ledger before injecting them into any worker (`oa-worker`, Claude Code,
Codex, Cursor, or Antigravity).

## What remains

- Desktop UI pane for inspecting, revising, and forgetting stored memories
  alongside consent grants.

## Done means

- [x] `openagents/core/memory.coax` trust, truncation, fork-sandbox, Auto-Dream,
  and secret-scanning gates pass `#authority` checks and run in `oa-worker.coax`.
- [x] `memory-log.coax` and `coder/tui/memory.coax` pass `ledger-profile.coax`
  against `proto/coder/consent-memory-ledgers-v1.json`.
- [x] `coder/examples/memory-lifecycle.coax` matches
  `coder/examples/memory-lifecycle.out` in debug and release.
- [ ] Desktop UI exposes live memory inspection, revision, and forgetting.
