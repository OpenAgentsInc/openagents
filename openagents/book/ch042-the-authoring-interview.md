# 42. The authoring interview

> **Status: partly implemented.** Conversational capability authoring (`#14059`)
> runs on `cpu` across the missing-capability router trigger
> ([`coax/lib/agent/router/policy.coax`](../../coax/lib/agent/router/policy.coax)),
> `#authority` `OA_PACK_CAPABILITY` permission and slug guards
> ([`openagents/core/members.coax`](../../openagents/core/members.coax)),
> and reusable `SKILL.md` drafting, verdict card rendering, and live next-turn
> Skills scanning ([`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax)),
> grounded by [`openagents/examples/capability-authoring-interview.coax`](../examples/capability-authoring-interview.coax)
> and [`openagents/examples/capability-authoring-interview.out`](../examples/capability-authoring-interview.out),
> and wired directly into [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax).
> Remaining: dedicated interactive approval-sheet UI (`coax drive` trace) and
> third-party OAuth vault binding in `openagents/eval/author/`.

## Premise

Chapter 41 writes a plugin by hand. This chapter lets a person who can't write
code get one by answering questions: the agent interviews you, drafts the
plugin and its tests, and runs them.

## Draft

[Episode 289](../../docs/transcripts/289.md) shows the exchange. You ask:
"Turn my meeting notes into Linear tickets." The agent answers that we can't
do that yet, and asks whether you want to build it as a plugin. You say yes.
It asks what the plugin should do and what it should not do. You say: one
ticket per action. It drafts tests and asks whether they look right. You say
they do. It runs them.

This is promise P9, and it changes who can extend the agent. The front already
knows when it has no member for a request. That moment is where a missing
capability is found. Most products apologize there. This one offers to build.

The interview is a program (chapter 27) that Coder runs. Its questions are
chosen to produce a test set. "What should it do?" becomes cases where the
plugin must act. "What shouldn't it do?" becomes cases where it must stay
quiet. Your answers are the specification, and the tests are your answers made
exact.

The output is a package: the plugin, its descriptor, the grant it needs and
its test set. The agent shows you the tests before it runs anything, because a
plugin that passes tests you didn't mean is worse than none.

If the tests pass, you have a working plugin for yourself. If you want
everyone to have it, the package goes on to the evidence chapters (43 to 52) and
then to the registry in chapter 54.

The agent has improved itself from chat before. [Episode
87](../../docs/transcripts/087.md) is a first upgrade, and in [Episode
192](../../docs/transcripts/192.md) OpenAgents changes its own code.

## What we build

A plan for what this chapter ships once the draft is approved:

- The offer: when no member fits, the front proposes building one.
- The interview program: questions, a drafted test set you approve, a drafted
  plugin, and a run.
- A package at the end that chapter 45 can measure without changes.

It builds on what exists today:

- [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax) (`ce_write_draft_ex`, `ce_parse_skill_meta`, `ce_format_verdict_card`, live per-turn Skills scanner, `#14059`)
- [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax) (`#authority` slug validation and promotion gate)
- [`openagents/core/members.coax`](../../openagents/core/members.coax) (`#authority` `OA_PACK_CAPABILITY`: `CapabilityDraft`, `CapabilityEval`, `CapabilityList`)
- [`coax/lib/agent/router/policy.coax`](../../coax/lib/agent/router/policy.coax) (missing-capability detection activating `capability_author`)
- [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax) (Episode 289 P5/P9/P15 conversational authoring interview and immediate next-turn local Skills activation)
- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/vault.coax`](../../coax/lib/nostr/vault.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The drafted test set and plugin | `openagents.eval.v1` `EvalDraft` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | In the conversation result (chapter 19) | Spec sketch; no `.proto` file |
| A login to another service | `openagents.vault.v1` `VaultObject` ([spec 15.3](../../docs/protocol/openagents-protocol-spec.md#153-openagentsvaultv1)) | Nostr: ciphertext in a kind 3188 envelope (NIP-VAULT) | Spec sketch; no `.proto` file. `coax/lib/nostr/vault.coax` builds the `user` tier |

### The chapter's check

- **Source:** [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax),
  [`openagents/core/members.coax`](../../openagents/core/members.coax),
  [`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax),
  [`openagents/examples/capability-authoring-interview.coax`](../examples/capability-authoring-interview.coax),
  and [`openagents/examples/capability-authoring-interview.out`](../examples/capability-authoring-interview.out)
  (remaining: interactive `coax drive` interview sheet in `openagents/eval/author/`).
- **Trace:** detects a missing capability for `"Turn my meeting notes into Linear tickets."`
  (`pack0=capability_author`, `pack1=general`, `hint=custom_api_tool`,
  `reason=missing_capability`), allows `CapabilityList` in `PLAN`, allows
  `CapabilityDraft` once approved (`draft_approved=allow`), parses drafted
  `SKILL.md` frontmatter (`name=linear-meeting-tickets`), and renders both
  promoted and rejected `#authority` verdict cards (`pass_card=true`, `fail_card=true`).
- **Refusal:** an unapproved `CapabilityDraft` stops at `ask` (`draft_unapproved=ask`);
  `CapabilityEval` in non-interactive `PLAN` mode is refused with `deny`; and
  path-traversal (`../escape`) or newline-injected (`linear-tickets\nbad`) slugs
  are refused with `deny` even in `BYPASS`.
- **Independent check:** `coax/bin/coax run openagents/examples/capability-authoring-interview.coax`
  and `--release` match [`openagents/examples/capability-authoring-interview.out`](../examples/capability-authoring-interview.out),
  alongside `#test` suites in [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax),
  [`openagents/core/members.coax`](../../openagents/core/members.coax) (`0`
  surviving mutants), and `oa-worker --selftest-capability` (`#14059`).

Planned paths: `openagents/eval/author/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:30, building a missing plugin from chat
  - [Episode 87, *Meta-Agent's First Upgrade*](../../docs/transcripts/087.md): an agent's first upgrade
  - [Episode 192, *OpenAgents Upgrades Itself*](../../docs/transcripts/192.md)
  - [Episode 245, *Khala Code*](../../docs/transcripts/245.md): plugins derived from real work
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `openagents-chat`, `brainstorm-client`, `plugin-action-items`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-VAULT](../../nostr/nips/openagents/NIP-VAULT.md)
- **Also:**
  - `plugins/meeting-action-items` and `plugins/brainstorm` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P9

## Open questions for the owner

1. Who pays for the compute an authoring run uses: the person, us for a first
   run, or the first paid use of the plugin?
2. A plugin that files tickets needs the person's login to another service.
   Does 1.0 include a vault for such logins, or do first plugins avoid them?

## Source listing

Run the Chapter 42 example in [`openagents/examples/capability-authoring-interview.coax`](../examples/capability-authoring-interview.coax) (committed transcript [`openagents/examples/capability-authoring-interview.out`](../examples/capability-authoring-interview.out)):

<!-- source: openagents/examples/capability-authoring-interview.coax capability-authoring-interview -->
```jai
cai_run :: () -> bool {
  // 1. Router detects missing capability and activates `capability_author`.
  q: [1] Rtr2PlanRequest;
  p: [1] Rtr2Plan;
  w: [1] Rtr2Work;
  q[0] = Rtr2PlanRequest.{
    text = "Turn my meeting notes into Linear tickets.",
    local_inkling_ready = true,
    cloud_ready = true
  };
  rtr2_decide_mock(
    q[..],
    "primary_pack=general,compute_tier=cloud_fast,explicit_delegate=none,difficulty=2,effort=medium,missing_capability=950,missing_capability_kind=custom_api_tool",
    0, p[..], w[..]
  );
  print("detect ok=% pack0=% pack1=% hint=% reason=%\n",
    p[0].ok, p[0].capability_packs[0], p[0].capability_packs[1],
    p[0].missing_capability_hint, p[0].reasons[0]);

  // 2. #authority capability-pack permissions and slug safety refusals.
  b_list := oa_member_tool_permission("CapabilityList", "", OA_MP_PMODE_PLAN, false, false);
  b_draft_ask := oa_member_tool_permission("CapabilityDraft", "linear-meeting-tickets", OA_MP_PMODE_DEFAULT, false, false);
  b_draft_ok := oa_member_tool_permission("CapabilityDraft", "linear-meeting-tickets", OA_MP_PMODE_DEFAULT, false, true);
  b_eval_plan := oa_member_tool_permission("CapabilityEval", "linear-meeting-tickets", OA_MP_PMODE_PLAN, false, false);
  b_traversal := oa_member_tool_permission("CapabilityDraft", "../escape", OA_MP_PMODE_BYPASS, true, true);
  b_newline := oa_member_tool_permission("CapabilityEval", "linear-tickets\nbad", OA_MP_PMODE_BYPASS, true, true);
  print("perms list_plan=% draft_unapproved=% draft_approved=% eval_plan=% refuse_traversal=% refuse_newline=%\n",
    cai_beh_name(b_list), cai_beh_name(b_draft_ask), cai_beh_name(b_draft_ok),
    cai_beh_name(b_eval_plan), cai_beh_name(b_traversal), cai_beh_name(b_newline));

  // 3. Parse drafted SKILL.md frontmatter and render #authority promotion verdict cards.
  md := "---\nname: linear-meeting-tickets\ndescription: Turn meeting notes into Linear tickets\n---\n\n# linear-meeting-tickets\n";
  nb: [64] u8; nn: [1] int;
  db: [256] u8; dn: [1] int;
  ce_parse_skill_meta(md, nb[..], nn[..], db[..], dn[..]);
  sname := cast(string) nb[0..<nn[0]];
  sdesc := cast(string) db[0..<dn[0]];

  v_pass := "{\"slug\":\"linear-meeting-tickets\",\"promoted\":true,\"verdict_code\":0,\"verdict\":\"PROMOTED: candidate passed two-arm eval and held-out gate\",\"baseline_passed\":1,\"candidate_passed\":2,\"baseline_heldout_passed\":1,\"candidate_heldout_passed\":1,\"candidate_latency_ms\":42,\"atif_verified\":true,\"steps_digest\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"evidence_path\":\"/skills/linear-meeting-tickets/evidence.json\"}";
  v_fail := "{\"slug\":\"broken-skill\",\"promoted\":false,\"verdict_code\":3,\"verdict\":\"REJECTED: candidate regressed below baseline\",\"baseline_passed\":2,\"candidate_passed\":1,\"baseline_heldout_passed\":1,\"candidate_heldout_passed\":0,\"candidate_latency_ms\":19,\"atif_verified\":true,\"steps_digest\":\"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210\",\"evidence_path\":\"/exp/broken-skill-rejected.json\"}";
  card: [2048] u8;
  cn1 := ce_format_verdict_card(v_pass, card[..]);
  c1 := cast(string) card[0..<cn1];
  pass_card_ok := cn1 > 0 && ce_contains_ci(c1, "PROMOTED") && ce_contains_ci(c1, "next turn") && ce_contains_ci(c1, "0123456789abcdef");

  cn2 := ce_format_verdict_card(v_fail, card[..]);
  c2 := cast(string) card[0..<cn2];
  fail_card_ok := cn2 > 0 && ce_contains_ci(c2, "REJECTED") && ce_contains_ci(c2, "not installed") && ce_contains_ci(c2, "fedcba9876543210");
  print("draft name=% desc=% pass_card=% fail_card=%\n", sname, sdesc, pass_card_ok, fail_card_ok);

  return p[0].ok && p[0].capability_packs[0] == "capability_author" &&
         p[0].missing_capability_hint == "custom_api_tool" &&
         b_list == OA_MP_PBEH_ALLOW && b_draft_ask == OA_MP_PBEH_ASK && b_draft_ok == OA_MP_PBEH_ALLOW &&
         b_eval_plan == OA_MP_PBEH_DENY && b_traversal == OA_MP_PBEH_DENY && b_newline == OA_MP_PBEH_DENY &&
         sname == "linear-meeting-tickets" && sdesc == "Turn meeting notes into Linear tickets" &&
         pass_card_ok && fail_card_ok;
}
```

Output (`coax/bin/coax run openagents/examples/capability-authoring-interview.coax`, identical in `--release`):

```text
detect ok=true pack0=capability_author pack1=general hint=custom_api_tool reason=missing_capability
perms list_plan=allow draft_unapproved=ask draft_approved=allow eval_plan=deny refuse_traversal=deny refuse_newline=deny
draft name=linear-meeting-tickets desc=Turn meeting notes into Linear tickets pass_card=true fail_card=true
ok=true
```
