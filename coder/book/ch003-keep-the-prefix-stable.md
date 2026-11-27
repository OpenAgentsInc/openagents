# 3. Keep the prefix stable

> **Status: partly implemented.** OpenAgents Core implements context freezing,
> 3-branch system-prompt cache splitting, 12-dimension cache-break forensics,
> capability-pack prompt profiles, and 5-minute cloud cache TTL expiry
> detection on CPU. End-to-end server-side KV-cache hit-rate ablations across
> multi-turn coding sessions remain open (see [Chapter 12](ch012-a-psionic-served-model.md)
> and [Psionic chapter 176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md)).

A prefix cache pays for itself only when turn `N+1` starts with the exact same
bytes as turn `N`. Moving a timestamp, re-sorting MCP tools, or re-reading
`git status` mid-session silently invalidates tens of thousands of cached
tokens and turns a 200 ms cached prefill into a multi-second cold prefill.

OpenAgents enforces prefix stability in three `#authority` modules compiled into
[`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax):

- **`openagents/core/prompt_context.coax`:** freezes `git status`, user
  instructions, and the session date into `FlawedContextCache` on turn 1;
  `flawed_ctx_check_date_rollover` detects midnight rollovers without mutating
  the cached prefix mid-session, and `oa_build_prompt_profile` emits compact,
  capability-gated prompt profiles (`OA_CAP_CODER`, `OA_CAP_WALLET`, etc.) so
  unused packs never bloat the prefix.
- **`openagents/core/prompt_cache.coax`:** splits system prompt blocks across
  the `FLAWED_SBLK_BOUNDARY` sentinel into `FLAWED_STRAT_GLOBAL`,
  `FLAWED_STRAT_TOOL_BASED`, or `FLAWED_STRAT_ORG_FALLBACK`, then runs two-phase
  12-dimension cache-break forensics (`flawed_cbd_record` + `flawed_cbd_check`)
  to distinguish client mutations (`FLAWED_BREAK_CLIENT`) from 5-minute/1-hour
  TTL expiry or server eviction.
- **`coax/lib/agent/engine/cache-policy.coax`:** models the 5-minute cloud
  prompt-cache TTL (`CP_CACHE_TTL_DEFAULT_MS = 300000`) and signals
  `CP_CACHE_EXPIRED_COMPACT` when an idle gap expires the cache on a large
  context (`>= 16,000` tokens), enabling honest idle compaction without extra
  model calls.

## Profile

This chapter's profile is `openagents-prompt-cache-v1, bounded CPU`:

- Frozen `FlawedContextCache` snapshots survive midnight date rollovers until an
  explicit compaction or session clear.
- Deferred MCP tools keep `FLAWED_STRAT_GLOBAL` caching intact without forcing
  per-org fallback.
- Cache-break forensics attributes client-side `tools_hash` drift to
  `FLAWED_BREAK_CLIENT` (bitmask `FLAWED_CBD_TOOLS = 2`).
- Cloud TTL classification separates `warm`, `expired`, and `expired_compact`
  at the exact 300,000 ms and 16,000-token boundaries.

## Worked example

The runnable example [prompt-cache.coax](../examples/prompt-cache.coax) runs
context freezing, `oa_build_prompt_profile`, wire prefix splitting, two-turn
cache-break forensics, and cloud TTL expiry detection directly against the real
product modules.

<!-- source: coder/examples/prompt-cache.coax example -->
```jai
main :: () {
  st: [1] FlawedSessionState;
  _ = flawed_state_init(st[..], "/repo", "claude-opus-4-7");
  ctx: [1] FlawedContextCache;
  _ = flawed_ctx_snapshot_git(ctx[..], "M openagents/core/prompt_cache.coax", true, false, true);
  _ = flawed_ctx_snapshot_user(ctx[..], st[..], "# OpenAgents Rules", "2026-11-26", false);

  // Midnight date rollover leaves cached prefix intact until explicit boundary clear
  rolled := flawed_ctx_check_date_rollover(ctx[..], "2026-11-27");
  print("context freeze: git_cached=% user_cached=% midnight_rollover_detected=%\n",
    ctx[0].git_cached, ctx[0].user_ctx_cached, rolled);

  // Compact capability-pack prompt profile for minimal local / cloud turns
  pb: [1024] u8;
  pn := oa_build_prompt_profile("macOS", OA_CAP_CODER | OA_CAP_WALLET, pb[..]);
  print("prompt profile bytes=%\n", pn);

  // Assemble tool pool with 1 built-in + 1 deferred MCP -> keeps global cache scope
  bi: [1] FlawedToolSpec;
  bi[0] = flawed_tool_defaults("Read", FLAWED_TOOL_SRC_BUILTIN);
  mcps: [1] FlawedToolSpec;
  mcps[0] = flawed_tool_defaults("mcp__fs__stat", FLAWED_TOOL_SRC_MCP);
  mcps[0].should_defer = true;
  pctx: [1] FlawedToolPermContext;
  pool: [1] FlawedToolPool;
  _ = flawed_assemble_tool_pool(pool[..], pctx[..], bi[..], mcps[..]);
  needs_tb := flawed_needs_tool_based_cache_marker(pool[..]);

  sb: [4] FlawedSysBlock;
  sb[0] = FlawedSysBlock.{kind = FLAWED_SBLK_BILLING, tag_id = 1};
  sb[1] = FlawedSysBlock.{kind = FLAWED_SBLK_CONTENT, tag_id = 2};
  sb[2] = FlawedSysBlock.{kind = FLAWED_SBLK_BOUNDARY, tag_id = 3};
  sb[3] = FlawedSysBlock.{kind = FLAWED_SBLK_CONTENT, tag_id = 4};
  w := flawed_split_sys_prompt_prefix(sb[..], 4, flawed_should_use_global_cache_scope(true, false), needs_tb);
  print("wire split: strategy=% needs_tool_marker=% blocks=%\n", strat_name(w.strategy), needs_tb, w.count);

  // 2-phase cache-break forensics: turn 1 warm hit; turn 2 mutating tools_hash -> CLIENT break
  tr: [1] FlawedCacheBreakTracker;
  s1 := FlawedPromptStateSnap.{system_hash = 101, tools_hash = 202, global_strategy = w.strategy};
  s2 := s1;
  s2.tools_hash = 203;
  m1 := flawed_cbd_record(tr[..], 1, s1);
  c1 := flawed_cbd_check(tr[..], 1, 50000, 1000, false);
  m2 := flawed_cbd_record(tr[..], 1, s2);
  c2 := flawed_cbd_check(tr[..], 1, 10000, 1000, false);
  print("forensics turn 1: diff_mask=% break=%\n", m1, break_name(c1.reason));
  print("forensics turn 2: diff_mask=% break=%\n", m2, break_name(c2.reason));

  // Cloud prompt-cache 5-minute TTL + idle compaction eligibility (coax/lib/agent/engine/cache-policy.coax)
  t_warm := cp_cache_status(1000, 200000, 0, 12000, 24000, true);
  t_exp := cp_cache_status(1000, 301000, 0, 12000, 8000, true);
  t_cmp := cp_cache_status(1000, 301000, 0, 12000, 24000, true);
  print("cloud ttl: 199s=% 300s_8k=% 300s_24k=%\n", ttl_name(t_warm), ttl_name(t_exp), ttl_name(t_cmp));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/prompt-cache.coax
```

```text
context freeze: git_cached=true user_cached=true midnight_rollover_detected=true
prompt profile bytes=463
wire split: strategy=global needs_tool_marker=false blocks=3
forensics turn 1: diff_mask=0 break=none
forensics turn 2: diff_mask=2 break=client
cloud ttl: 199s=warm 300s_8k=expired 300s_24k=expired_compact
```

The checked fixture is [prompt-cache.out](../examples/prompt-cache.out).

## What remains

- Live multi-turn server-side prefix-cache hit-rate and TTFT benchmarks across
  local `psionic-inkling-serve` (32K KV) and cloud providers on a shared coding
  suite.

## Done means

- [x] Context freezing, wire prefix splitting, cache-break forensics, and cloud
  TTL expiry pass `#authority` checks and are adopted by `openagents/runtime/oa-worker.coax`.
- [x] `coder/examples/prompt-cache.coax` matches `coder/examples/prompt-cache.out`
  in debug and release.
- [ ] Server-side prefix-cache hit rate and prefill savings are benchmarked
  end-to-end across a multi-turn coding suite.
