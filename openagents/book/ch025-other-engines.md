# 25. Other engines

> **Status: partly implemented.** The `#authority` engine availability,
> cooldown failover, and decision-door routing policies
> ([`coax/lib/agent/engine/availability-policy.coax`](../../coax/lib/agent/engine/availability-policy.coax),
> [`route-policy.coax`](../../coax/lib/agent/engine/route-policy.coax)),
> unified engine interface ([`engine.coax`](../../coax/lib/agent/engine/engine.coax)),
> native choice gating ([`coax/lib/agent/router/native-choice.coax`](../../coax/lib/agent/router/native-choice.coax)),
> and external engine workers ([`openagents/workers/agyworker.coax`](../../openagents/workers/agyworker.coax),
> [`coax/lib/agent/claude`](../../coax/lib/agent/claude),
> [`coax/lib/agent/codex`](../../coax/lib/agent/codex),
> [`coax/lib/agent/cursor`](../../coax/lib/agent/cursor)) run on `cpu`, grounded by
> [`openagents/examples/engine-availability-routing.coax`](../examples/engine-availability-routing.coax)
> and [`openagents/examples/engine-availability-routing.out`](../examples/engine-availability-routing.out),
> and are consumed by [`coax/lib/agent/engine/availability.coax`](../../coax/lib/agent/engine/availability.coax),
> [`coax/lib/agent/engine/route.coax`](../../coax/lib/agent/engine/route.coax),
> and [`coax/lib/agent/router/wire.coax`](../../coax/lib/agent/router/wire.coax).
> Remaining: cross-engine recorded-stream parity fixtures comparing every
> external adapter's result fields one-by-one against chapter 24's Coder member.

## Premise

Coder is one member. This chapter lets you name another: "do it with Codex"
sends the same task to a different engine, under the same grant and the same
record.

## Draft

[Episode 289](../../docs/transcripts/289.md) shows the phrase on screen: "Do
it with Codex." The front reads that you named an engine and honors it. Coder
then drives that engine and does not use its own loop.

The reason is neutrality. [Episode 278](../../docs/transcripts/278.md) argues
that an agent tied to no single provider should coordinate the competing model
families. The archive shows why. It integrates outside coding agents,
criticizes them, replaces them and returns to them, and [Episode
204](../../docs/transcripts/204.md) records what happens when a provider
changes its rules under a workflow you depend on. Your work should not belong
to one supplier.

It is also practical. Many people already pay for a coding agent. Driving the
one you have costs you nothing extra, and it lets you compare engines on your
own tasks.

Engines behave differently. They have different tools, prices, limits and ways
of failing. So the choice follows a policy you can read: the engine you named,
then what the computer's owner allows, then which engine has capacity. If an
engine is at its limit, the next one takes over, and the log records which
engine ran.

The old tree delegated to five engines. Today's Coax tree runs our own
unified [`openagents/core/`](../../openagents/core) native engine
(`oa-worker.coax`) and [`coax/lib/inky/`](../../coax/lib/inky) by default
(retiring `microcoder`, `#13983`), and delegates to Claude Code, Codex,
Cursor, and Antigravity (`agyworker.coax`, `#14024`, `#14031`) when the
person explicitly opts in (`#14071`). Chapter 26 gives each task a model tier.

## What we build

A plan for what this chapter ships once the draft is approved:

- An engine field in the plan, filled when the person names one.
- A policy for choosing and failing over between engines, with the choice
  logged.
- A check that every engine's result comes back in the same shape as Coder's
  own.

It builds on what exists today:

- [`coder/tui/oa-worker.coax`](../../openagents/runtime/oa-worker.coax) and [`coax/lib/agent/router/native-choice.coax`](../../coax/lib/agent/router/native-choice.coax) (native default + opt-in external delegation gating, `#14071`)
- [`coax/lib/agent/claude`](../../coax/lib/agent/claude)
- [`coax/lib/agent/codex`](../../coax/lib/agent/codex)
- [`coax/lib/agent/cursor`](../../coax/lib/agent/cursor)
- [`coder/tui/agyworker.coax`](../../openagents/workers/agyworker.coax)
- [`coax/lib/agent/tier.coax`](../../coax/lib/agent/tier.coax)
- [`coax/lib/agent/engine/availability-policy.coax`](../../coax/lib/agent/engine/availability-policy.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| What each engine speaks | Theirs: Claude Code stream-json, Codex JSON-RPC, ACP ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md)) | Their own | External. We register each format with a fixture and write no schema for it |
| The engine named in the plan | A field of the plan (chapter 14); the engines we know are `openagents.session.v1` `Adapter` ([spec 18.3](../../docs/protocol/openagents-protocol-spec.md#183-openagentssessionv1)) | With the plan | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** [`coax/lib/agent/engine/availability-policy.coax`](../../coax/lib/agent/engine/availability-policy.coax),
  [`coax/lib/agent/engine/route-policy.coax`](../../coax/lib/agent/engine/route-policy.coax),
  [`openagents/examples/engine-availability-routing.coax`](../examples/engine-availability-routing.coax),
  and [`openagents/examples/engine-availability-routing.out`](../examples/engine-availability-routing.out)
  (remaining: end-to-end recorded session replay parity across all engine adapters).
- **Trace:** parses engine allowlists (`def=15`, `pair=3`, `all=31`), honors
  explicit preference `"codex"` (`pick=codex`), fails over to `claude` when
  Codex is blocked by cooldown, keeps `antigravity` deferred unless alone or
  preferred, maps difficulty tiers `0..4` (`haiku`, `sonnet`, `opus`), and
  admits loopback and opt-in HTTPS decision doors.
- **Refusal:** duplicate or unknown allowlists (`codex,codex`, `unknown`,
  `cursor`) return `0` and never widen; corrupted >7-day cooldown resets
  exclude the engine; invalid masks return `-1`; unopted remote, plaintext
  remote HTTP, and credential-bearing URLs are refused by `cr_endpoint`.
- **Independent check:** `coax/bin/coax run openagents/examples/engine-availability-routing.coax`
  and `--release` match [`openagents/examples/engine-availability-routing.out`](../examples/engine-availability-routing.out),
  alongside exhaustive subset `#test` and `#forall` URL byte-admission checks
  in [`availability-policy.coax`](../../coax/lib/agent/engine/availability-policy.coax)
  and [`route-policy.coax`](../../coax/lib/agent/engine/route-policy.coax).

Planned paths: none. The engines are
[`coax/lib/agent/engine/`](../../coax/lib/agent/engine): one interface
(`engine.coax`), the catalog and the route. Each becomes a member through
chapter 13's member list, with no engine code in `openagents/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00, "Do it with Codex"
  - [Episode 278, *Coder Commands Codex & Claude*](../../docs/transcripts/278.md): commanding Codex and Claude
  - [Episode 185, *Hello Claude Code*](../../docs/transcripts/185.md): first Claude Code integration
  - [Episode 193, *Codex & Claude Code On Your Phone*](../../docs/transcripts/193.md): Codex and Claude Code from the phone
  - [Episode 204, *DO NOT BREAK USERSPACE*](../../docs/transcripts/204.md)
  - [Episode 243, *Khala in OpenCode*](../../docs/transcripts/243.md): orchestration inside OpenCode
  - [Episode 244, *Codex in Khala*](../../docs/transcripts/244.md): Codex inside the orchestrator
- **Old code at `coder-v1.0.0`, crates:** `coder-delegate`, `codex-transport`, `claude_agent_sdk`, `acp-client`, `coder-engine-status`
- **NIPs:** [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md), [NIP-WORK](../../nostr/nips/openagents/NIP-WORK.md), [NIP-PRG](../../nostr/nips/openagents/NIP-PRG.md)
- **Also:**
  - Coder book chapters [10](../../coder/book/ch010-drive-claude-code.md), [11](../../coder/book/ch011-drive-codex.md), [13](../../coder/book/ch013-choose-an-engine.md) and [38](../../coder/book/ch038-drive-cursor.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3

## Open questions for the owner

1. Which engines does 1.0 name? The Coax tree drives Claude Code, Codex and
   Cursor. The old tree also drove Grok Build, OpenCode and Devin.

## Source listing

Run the Chapter 25 example in [`openagents/examples/engine-availability-routing.coax`](../examples/engine-availability-routing.coax) (committed transcript [`openagents/examples/engine-availability-routing.out`](../examples/engine-availability-routing.out)):

<!-- source: openagents/examples/engine-availability-routing.coax engine-availability-routing -->
```jai
ear_run :: () -> bool {
  // 1. Allowlist parsing and explicit Codex selection ("do it with Codex").
  def_mask := ca_allowed("");
  pair_mask := ca_allowed("claude,codex");
  all_mask := ca_allowed("antigravity,claude,codex,micro,psionic");
  pick_codex := ca_pick(ca_index("codex"), pair_mask);

  // 2. Cooldown failover: Codex is rate-limited until t=180; at t=150 it is blocked,
  // so healthy mask drops Codex and fails over to Claude; at t=180 Codex recovers.
  codex_cool := ca_blocked(100, 180, 150);
  codex_ready := !ca_blocked(100, 180, 180);
  failover := ca_pick(ca_index("codex"), pair_mask & (31 ^ (1 << ca_index("codex"))));
  agy_only := ca_pick(-1, 16);
  agy_deferred := ca_pick(-1, 17);
  print("masks def=% pair=% all=% pick=% failover=% agy=(only=%,deferred=%)\n",
    def_mask, pair_mask, all_mask, ear_name(pick_codex), ear_name(failover),
    ear_name(agy_only), ear_name(agy_deferred));

  // 3. Auto-routing verbs, difficulty model mapping, and loopback vs remote door admission.
  auto_run := cr_auto("run", "");
  auto_fork := cr_auto("fork", "");
  m_low := cr_model("claude", 0);
  m_mid := cr_model("claude", 2);
  m_hi := cr_model("claude", 4);
  ep_local := cr_endpoint("http://127.0.0.1:8793/v1/systemone", false);
  ep_remote_ok := cr_endpoint("https://api.openagents.com/v1/systemone", true);
  print("route auto_run=% fork_keeps_parent=% claude_tiers=%,%,% ep=(local=%,remote=%)\n",
    auto_run, !auto_fork, m_low, m_mid, m_hi, ep_local, ep_remote_ok);

  // 4. Fail-closed refusals: bad/duplicate allowlists never widen, out-of-range cooldown blocks,
  // invalid mask returns -1, and remote/plaintext/credentialed endpoints refuse.
  ref_dup := ca_allowed("codex,codex") == 0;
  ref_unk := ca_allowed("unknown") == 0 && ca_allowed("cursor") == 0;
  ref_corrupt_cd := ca_blocked(100, 604901, 604901);
  ref_mask := ca_pick(0, 47) == -1;
  ref_remote_off := !cr_endpoint("https://api.openagents.com/v1/systemone", false);
  ref_http_remote := !cr_endpoint("http://api.openagents.com/v1/systemone", true);
  ref_creds := !cr_endpoint("https://user@api.openagents.com/v1/systemone", true);
  print("refuse dup=% unk=% corrupt_cd=% mask=% remote_off=% http=% creds=%\n",
    ref_dup, ref_unk, ref_corrupt_cd, ref_mask, ref_remote_off, ref_http_remote, ref_creds);

  return def_mask == 15 && pair_mask == 3 && all_mask == 31 &&
         pick_codex == 1 && codex_cool && codex_ready && failover == 0 &&
         agy_only == 4 && agy_deferred == 0 &&
         auto_run && !auto_fork && m_low == "haiku" && m_mid == "sonnet" && m_hi == "opus" &&
         ep_local && ep_remote_ok &&
         ref_dup && ref_unk && ref_corrupt_cd && ref_mask &&
         ref_remote_off && ref_http_remote && ref_creds;
}
```

Output (`coax/bin/coax run openagents/examples/engine-availability-routing.coax`, identical in `--release`):

```text
masks def=15 pair=3 all=31 pick=codex failover=claude agy=(only=antigravity,deferred=claude)
route auto_run=true fork_keeps_parent=true claude_tiers=haiku,sonnet,opus ep=(local=true,remote=true)
refuse dup=true unk=true corrupt_cd=true mask=true remote_off=true http=true creds=true
ok=true
```
