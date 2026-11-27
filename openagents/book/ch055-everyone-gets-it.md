# 55. Everyone gets it

> **Status: partly implemented.** Hash-pinned zero-recompilation linked
> weight-module composites ([`psionic/lib/module-v2-wave2.coax`](../../psionic/lib/module-v2-wave2.coax))
> and the `#authority` `shadow -> canary -> promoted` rollout gate with
> automatic rollback on canary divergence ([`psionic/lib/peer/release-promote.coax`](../../psionic/lib/peer/release-promote.coax))
> run on `cpu` with their runnable book example
> [`openagents/examples/pinned-defaults-rollout.coax`](../examples/pinned-defaults-rollout.coax)
> and committed debug/release transcript
> [`openagents/examples/pinned-defaults-rollout.out`](../examples/pinned-defaults-rollout.out).
> Remaining: `openagents/registry/install.coax`, `openagents/registry/defaults/`,
> and the in-app provenance browser UI.

## Premise

A release exists in the registry. This chapter gets it onto every agent:
installs are pinned by hash, and an adopted release reaches every OpenAgents
through a signed defaults update.

## Draft

[Episode 289](../../docs/transcripts/289.md): "If it's good, it gets added to
the registry, and then all OpenAgents will have your plugin." This is promise
P10, and it is two steps.

The first step is an install. You pick a release, and your agent fetches its
components and checks each against the hash in the signed release. What is
installed is exactly what was published. An install is pinned. It does not
move to a newer release unless you move it.

The second step is the defaults. The defaults are a package of their own: a
release that lists the members every agent starts with. When we adopt a member
(chapter 49), we publish a new defaults release that depends on the member's
release and cites the adoption record. Every agent that follows our defaults
picks it up on its next sync. That is how one person's plugin reaches
everyone.

The chain can be audited from either end. Start from a member in your agent
and you can walk back to the defaults release, the adoption record, the
validations and the original report. Start from an author's release and you
can see whether it was adopted and why.

Updates are safe to take. A run in progress keeps its lock. A defaults release
can be rolled back by publishing the previous list again. And a person can
follow different defaults, or none.

In the old tree this path carried one plugin to every Coder. The plan is for
it to carry every kind of member to every surface.

## What we build

A plan for what this chapter ships once the draft is approved:

- Install by hash: fetch, verify against the signed release, and pin.
- A defaults package and a sync that every surface runs.
- A provenance view: from any member back to its release, its adoption and its
  evidence.

It builds on what exists today:

- [`psionic/lib/module-v2-wave2.coax`](../../psionic/lib/module-v2-wave2.coax) (zero-recompilation linked composites with exact SHA-256 verification)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/crypto`](../../coax/lib/crypto)
- [`coder/tui/plugins.coax`](../../coder/tui/plugins.coax)
- [`psionic/lib/peer/release-promote.coax`](../../psionic/lib/peer/release-promote.coax) (`#authority` `shadow -> canary -> promoted` state machine with automatic rollback on canary divergence) and [`apps/openagents-cli/executor.coax`](../../apps/openagents-cli/executor.coax) ([W6.3 H7 verification record](../../docs/verification/executor-network-effect-h7.md))

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A pinned install | `openagents.common.v1` locks ([spec 9.2](../../docs/protocol/openagents-protocol-spec.md#92-locks)) | A file on the device | Spec sketch; no `.proto` file |
| The defaults update | `openagents.extension.v1` `ComponentSet`, `Registry` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Nostr: a signed release | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** [`psionic/lib/module-v2-wave2.coax`](../../psionic/lib/module-v2-wave2.coax),
  [`psionic/lib/peer/release-promote.coax`](../../psionic/lib/peer/release-promote.coax),
  [`openagents/examples/pinned-defaults-rollout.coax`](../examples/pinned-defaults-rollout.coax),
  and the committed stdout transcript [`openagents/examples/pinned-defaults-rollout.out`](../examples/pinned-defaults-rollout.out)
  (remaining: `openagents/registry/install.coax` and `openagents/registry/defaults/`).
- **Trace:** links `wire-gate -> patch-rebase` into a 7-layer composite with
  zero recompilation, reloads by exact SHA-256 (`out=1,28,102`), advances a
  qualified candidate through `shadow -> canary (10%) -> promoted (100%)`
  (`active=2000 pct=100`).
- **Refusal:** flipping 1 bit of the serialized weights refuses deserialization
  with `EvStatus.digest` (`tamper_refused=true`); a candidate below 100% exact
  replay is blocked before canary (`refuse_inexact=true`); 1 canary divergence
  automatically rolls back to the pinned target (`restored_active=1000`).
- **Independent check:** `coax/bin/coax run openagents/examples/pinned-defaults-rollout.coax`
  and `--release` produce identical transcripts matching
  [`openagents/examples/pinned-defaults-rollout.out`](../examples/pinned-defaults-rollout.out),
  beside `coax/bin/coax test psionic/lib/peer/release-promote.coax` and
  `coax/bin/coax test psionic/lib/module-v2-wave2.coax`.

Shipped paths: [`openagents/registry/install.coax`](../../openagents/registry/install.coax) (`#14060`, SHA-256 hash-pinned inert install, active-run lock pin, monotone signed defaults rollout/rollback).

Planned paths: `openagents/registry/defaults/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, all OpenAgents will have your plugin
  - [Episode 211, *Autopilot Online*](../../docs/transcripts/211.md): automatic upgrades
  - [Episode 256, *Release Candidate*](../../docs/transcripts/256.md): release candidates
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `ext-eval`, `discovery`
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-DEPLOY](../../nostr/nips/openagents/NIP-DEPLOY.md)
- **Also:**
  - `packages/coder-defaults` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10

## Open questions for the owner

1. Does a defaults update install on its own, or does each person confirm it?

## Source listing

Run the Chapter 55 example in [`openagents/examples/pinned-defaults-rollout.coax`](../examples/pinned-defaults-rollout.coax) (committed transcript [`openagents/examples/pinned-defaults-rollout.out`](../examples/pinned-defaults-rollout.out)):

<!-- source: openagents/examples/pinned-defaults-rollout.coax pinned-defaults-rollout -->
```jai
pdr_run :: () -> bool {
  // 1. Link wire-gate -> patch-rebase with ZERO recompilation, verify exact SHA-256 reload & 1-byte tamper refusal.
  la: [MV_MAX_LAYERS] EvLayerDesc; ha: [MV_MAX_HEADS] EvHeadDesc; wa: [65536] f64;
  lb: [MV_MAX_LAYERS] EvLayerDesc; hb: [MV_MAX_HEADS] EvHeadDesc; wb: [65536] f64;
  lc: [MV_MAX_LAYERS] EvLayerDesc; hc: [MV_MAX_HEADS] EvHeadDesc; wc: [131072] f64;
  m_wg := mv_compile_wire_gate(la[..], ha[..], wa[..]);
  m_pr := mv_compile_patch_rebase(lb[..], hb[..], wb[..]);
  wire := [7] int.[0, 1, 2, 3, 4, 5, 6];
  m_comp := mv_link_models(m_wg, m_pr, wire[..], lc[..], hc[..], wc[..]);

  sb: [131072] u8;
  ser := ev_serialize(m_comp, sb[..]);
  lr: [MV_MAX_LAYERS] EvLayerDesc; hr: [MV_MAX_HEADS] EvHeadDesc; wr: [131072] f64;
  reloaded := ev_deserialize(sb[0..<ser.written], ser.digest, true, lr[..], hr[..], wr[..]);
  inp := [10] int.[2, 16, 64, 10, 4, 7, 25, 3, 2, 100];
  out: [3] int;
  ev_ok := reloaded.model.status == EvStatus.ok && mv_eval_model(reloaded.model, inp[..], out[..]) == EvStatus.ok;
  sb[ser.written - 1] ^= 1;
  tampered := ev_deserialize(sb[0..<ser.written], ser.digest, true, lr[..], hr[..], wr[..]);
  print("composite_layers=% reload_ok=% out=%,%,% tamper_refused=%\n",
    m_comp.n_layers, ev_ok, out[0], out[1], out[2], tampered.model.status == EvStatus.digest);

  // 2. Healthy #authority defaults rollout: shadow -> canary (10%) -> promoted (100%).
  r: [1] RpRelease;
  r[0] = rp_init(1000);
  q := RpQual.{
    sig_ok = true, digest_ok = true,
    base_pass_bps = 6800, cand_pass_bps = 7400, min_lift_bps = 500,
    exact_replay_bps = 10000, crash_rate_bps = 5, max_crash_bps = 10,
  };
  s_can := rp_enter_canary(r[..], 2000, 1000, 10, q);
  s_pro := rp_finalize_canary(r[..], 100, 50, 0, 5, 10);
  print("healthy_canary=% healthy_promote=% active=% pct=%\n",
    s_can == RpStatus.ok, s_pro == RpStatus.ok, r[0].active_sha, r[0].canary_pct);

  // 3. Refusal: non-100% exact replay blocked before canary; 1 canary divergence auto-rolls back to pinned target.
  r_bad: [1] RpRelease;
  r_bad[0] = rp_init(1000);
  q_inexact := q; q_inexact.exact_replay_bps = 9999;
  refuse_inexact := rp_enter_canary(r_bad[..], 2001, 1000, 10, q_inexact) == RpStatus.exactness_failed;

  r_rb: [1] RpRelease;
  r_rb[0] = rp_init(1000);
  _ := rp_enter_canary(r_rb[..], 2002, 1000, 10, q);
  rb_st := rp_finalize_canary(r_rb[..], 100, 50, 1, 5, 10);
  print("refuse_inexact=% rollback_triggered=% restored_active=%\n",
    refuse_inexact, rb_st == RpStatus.canary_unhealthy && r_rb[0].channel == RpChannel.rolled_back, r_rb[0].active_sha);

  return ev_ok && out[0] == 1 && out[1] == 28 && out[2] == 102 &&
         tampered.model.status == EvStatus.digest &&
         s_pro == RpStatus.ok && r[0].active_sha == 2000 &&
         refuse_inexact && r_rb[0].active_sha == 1000;
}
```

Output (`coax/bin/coax run openagents/examples/pinned-defaults-rollout.coax`, identical in `--release`):

```text
composite_layers=7 reload_ok=true out=1,28,102 tamper_refused=true
healthy_canary=true healthy_promote=true active=2000 pct=100
refuse_inexact=true rollback_triggered=true restored_active=1000
ok=true
```
