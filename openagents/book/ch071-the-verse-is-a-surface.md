# 71. The Metaverse is a surface

> **Status: partly implemented.** The `#authority` Metaverse & GameBuilder
> surface routing policy ([`coax/lib/agent/desktop/metaverse-route.coax`](../../coax/lib/agent/desktop/metaverse-route.coax),
> `#14038`–`#14046`), embedded Desktop Metaverse pane
> ([`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax)),
> and pinned Everglade session wire codec ([`everglade/shared/everglade-wire.coax`](../../everglade/shared/everglade-wire.coax))
> run on `cpu`, grounded by
> [`openagents/examples/metaverse-surface-routing.coax`](../examples/metaverse-surface-routing.coax)
> and [`openagents/examples/metaverse-surface-routing.out`](../examples/metaverse-surface-routing.out),
> and are consumed by [`apps/openagents-desktop/host-core.coax`](../../apps/openagents-desktop/host-core.coax),
> [`apps/openagents-desktop/desktop-core.coax`](../../apps/openagents-desktop/desktop-core.coax),
> [`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax),
> and [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax).
> Remaining: headless Chrome `web` `coax drive` cross-surface chat-panel
> continuation with `/chat` in one session and in-world Gym/wallet panels.

## Premise

The agent has lived in chat windows. This chapter puts it in a world: the
OpenAgents Metaverse is a real-time 3D place on the open web, and it is another surface of
the same runtime, not a separate product.

## Draft

[Episode 290](../../docs/transcripts/290.md) opens on openagents.com and says
it is not a slide deck, it is a playtest. The projects stand as towers on a
grid. You click "walk around" and you are in a small multiplayer world with
whoever else is there.

The reason is stated in the first minute. Working with agents should not feel
like arguing with a terminal. It should feel a little more like a video game:
a 3D world with 2D interfaces woven into it, where you have some fun while you
direct your agents.

The archive has circled this for years: a heads-up display in [Episode
111](../../docs/transcripts/111.md), a multiplayer workspace in [Episode
116](../../docs/transcripts/116.md), a game announced in [Episode
189](../../docs/transcripts/189.md), a walkable run board in [Episode
240](../../docs/transcripts/240.md), and a design for experience points and
classes in [Episode 284](../../docs/transcripts/284.md). The argument is
always that a game interface makes many agents, their work and their money
easy to see.

What changed is the engine. The OpenAgents Metaverse Engine is written in Coax, and it is built for the
web first so that a realm is a link you can share. Its own book builds it, and
Building Everglade builds a whole game on it.

In this book the Metaverse is a surface in the sense of chapter 10. It links the
same runtime. The agent you talk to in the world is the one you talk to in
chat, in the same conversation, with the same members, grants and wallet. A
panel in the world and a page on the web show the same records. On Desktop,
the **Embedded Metaverse & GameBuilder Pane** (`#14038`–`#14046`,
[`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax)
and [`apps/openagents-desktop/Window+Metaverse.swift`](../../apps/openagents-desktop/Window+Metaverse.swift))
already mounts live 3D worlds right beside the agent chat, exposes the
`OA_PACK_METAVERSE` tool pack (`MetaverseRoute`, `GameBuilder`, `#14057`), and
joins multiplayer sessions over the **GameBuilder Nostr Lobby** (`#14045`).

## What we build

A plan for what this chapter ships once the draft is approved:

- The agent runtime linked into the Metaverse, with chat as a panel in the world.
- World views of things this book built: the member list, the Gym board, and
  your wallet.
- One conversation that continues between the world and any other surface.

It builds on what exists today:

- [`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax) and [`apps/openagents-desktop/Window+Metaverse.swift`](../../apps/openagents-desktop/Window+Metaverse.swift) (embedded Desktop Metaverse & GameBuilder pane, `#14038`–`#14046`)
- [`openagents/core/members.coax`](../../openagents/core/members.coax) (`OA_PACK_METAVERSE` tool pack, `#14057`)
- [`metaverse-engine/README.md`](../../metaverse-engine/README.md)
- [`metaverse-engine/talk`](../../metaverse-engine/talk)
- [`metaverse-engine/ui`](../../metaverse-engine/ui)
- [`metaverse-engine/net`](../../metaverse-engine/net)
- [`verse`](../../metaverse)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`coax/lib/nostr/nipmv.coax`](../../coax/lib/nostr/nipmv.coax)
- [`metaverse-engine/book/README.md`](../../metaverse-engine/book/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A realm session | Text frames, described frame by frame in [`everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json) | Frames on a WebSocket or WebTransport | In use: one hand-packed codec, `everglade/shared/everglade-wire.coax`, pinned by that profile. The compliance rule allows this kind ([chapter 66](ch066-the-compliance-rule.md)) |
| Presence in a world | `openagents.verse.presence.v1alpha1` ([`presence.proto`](../../proto/openagents/verse/presence/v1alpha1/presence.proto)) | None yet. NIP-MV carries poses as kinds 23300 and 33301, unchanged | Schema file; no product consumer |

### The chapter's check

- **Source:** [`coax/lib/agent/desktop/metaverse-route.coax`](../../coax/lib/agent/desktop/metaverse-route.coax),
  [`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax),
  [`everglade/shared/everglade-wire.coax`](../../everglade/shared/everglade-wire.coax),
  [`openagents/examples/metaverse-surface-routing.coax`](../examples/metaverse-surface-routing.coax),
  and [`openagents/examples/metaverse-surface-routing.out`](../examples/metaverse-surface-routing.out)
  (remaining: headless Chrome `web` `coax drive` realm-to-`/chat` continuation).
- **Trace:** classifies first-party realm URLs (`https://openagents.com/realms/everglade` →
  `pane=1`, `kind=6`, `badge=RLM`, `ro=true`), GameBuilder play URIs
  (`openagents://gamebuilder/play/everglade` → `pane=1`, `kind=7`, `mode=2`,
  `badge=GB`), and `.glb` models (`badge=GLB`) into `MV_PANE_METAVERSE`, keeps
  plain notes in `MV_PANE_FILE`, downgrades when surface flags are off
  (`reason=flag_off`), and admits safe relative `.gltf` texture URIs.
- **Refusal:** path traversal (`../secrets/town.glb`) fails closed with
  `MV_PANE_REJECT`/`unsafe_path`; untrusted external URLs
  (`https://evil.example/payload.glb`) fail closed with
  `MV_PANE_REJECT`/`untrusted_external_url`; external-scheme, absolute, and
  traversing `.gltf` resource URIs refuse (`false`).
- **Independent check:** `coax/bin/coax run openagents/examples/metaverse-surface-routing.coax`
  and `--release` match [`openagents/examples/metaverse-surface-routing.out`](../examples/metaverse-surface-routing.out),
  alongside `#authority` checks in [`coax/lib/agent/desktop/metaverse-route.coax`](../../coax/lib/agent/desktop/metaverse-route.coax)
  and Everglade session wire profile checks against
  [`everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json).

Planned paths: none. The Metaverse surface is a realm route in
[`apps/openagents-web`](../../apps/openagents-web) over
[`metaverse-engine/talk`](../../metaverse-engine/talk).

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 0:00 to 2:30, a playtest, not a slide deck
  - [Episode 111, *Heads-Up Display*](../../docs/transcripts/111.md): a heads-up display
  - [Episode 116, *3D Multiplayer*](../../docs/transcripts/116.md): a multiplayer 3D workspace
  - [Episode 176, *Hand Tracking*](../../docs/transcripts/176.md)
  - [Episode 189, *Toward an Agentic MMORPG*](../../docs/transcripts/189.md): toward an MMORPG
  - [Episode 240, *The Metaverse*](../../docs/transcripts/240.md)
  - [Episode 283, *Coder for Gamers?*](../../docs/transcripts/283.md): gamers as an audience
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): a productive the Metaverse
- **Old code at `coder-v1.0.0`, crates:** `verse`, `verse-core`, `verse-engine`, `verse-host`, `verse-net`, `verse-world`, `openagents-web`
- **NIPs:** [NIP-MV](../../nostr/nips/openagents/NIP-MV.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Also:**
  - [Building the OpenAgents Metaverse Engine](../../metaverse-engine/book/README.md)
  - [Building Everglade](../../everglade/book/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P13

## Source listing

Run the Chapter 71 example in [`openagents/examples/metaverse-surface-routing.coax`](../examples/metaverse-surface-routing.coax) (committed transcript [`openagents/examples/metaverse-surface-routing.out`](../examples/metaverse-surface-routing.out)):

<!-- source: openagents/examples/metaverse-surface-routing.coax metaverse-surface-routing -->
```jai
msr_run :: () -> bool {
  // 1. Canonical routes with all desktop surface flags enabled.
  r_realm := mv_route_classify("https://openagents.com/realms/everglade", DC_FLAGS_ALL);
  r_play := mv_route_classify("openagents://gamebuilder/play/everglade", DC_FLAGS_ALL);
  r_glb := mv_route_classify("assets/scenes/town.glb", DC_FLAGS_ALL);
  r_txt := mv_route_classify("notes/readme.md", DC_FLAGS_ALL);
  print("realm pane=% kind=% badge=% ro=% reason=%\n",
    r_realm.pane, r_realm.kind, r_realm.badge, r_realm.read_only, r_realm.reason);
  print("play pane=% kind=% mode=% badge=% ro=% glb_badge=% txt_pane=%\n",
    r_play.pane, r_play.kind, r_play.mode, r_play.badge, r_play.read_only, r_glb.badge, r_txt.pane);

  // 2. Feature-flag downgrade (falls back to file pane with reason=flag_off) + safe relative glTF URI.
  r_gated := mv_route_classify("https://openagents.com/realms/everglade", 0);
  uri_ok := mv_gltf_external_uri_safe("textures/stone_albedo.png");
  print("gated pane=% reason=% safe_uri=%\n", r_gated.pane, r_gated.reason, uri_ok);

  // 3. Fail-closed security refusals: path traversal, untrusted external URL, unsafe glTF external URIs.
  r_trav := mv_route_classify("../secrets/town.glb", DC_FLAGS_ALL);
  r_ext := mv_route_classify("https://evil.example/payload.glb", DC_FLAGS_ALL);
  uri_http := mv_gltf_external_uri_safe("https://evil.example/a.bin");
  uri_abs := mv_gltf_external_uri_safe("/etc/passwd");
  uri_trav := mv_gltf_external_uri_safe("../secret.bin");
  print("refuse trav=(%,%) ext=(%,%) uri=(%,%,%)\n",
    r_trav.pane == MV_PANE_REJECT, r_trav.reason,
    r_ext.pane == MV_PANE_REJECT, r_ext.reason,
    !uri_http, !uri_abs, !uri_trav);

  return r_realm.pane == MV_PANE_METAVERSE && r_realm.kind == MV_KIND_REALM && r_realm.badge == "RLM" && r_realm.read_only &&
         r_play.pane == MV_PANE_METAVERSE && r_play.kind == MV_KIND_GAMEBUILDER && r_play.mode == MV_MODE_PLAY &&
         r_glb.pane == MV_PANE_METAVERSE && r_glb.kind == MV_KIND_MODEL && r_glb.badge == "GLB" &&
         r_txt.pane == MV_PANE_FILE && r_gated.pane == MV_PANE_FILE && r_gated.reason == "flag_off" &&
         uri_ok && r_trav.pane == MV_PANE_REJECT && r_trav.reason == "unsafe_path" &&
         r_ext.pane == MV_PANE_REJECT && r_ext.reason == "untrusted_external_url" &&
         !uri_http && !uri_abs && !uri_trav;
}
```

Output (`coax/bin/coax run openagents/examples/metaverse-surface-routing.coax`, identical in `--release`):

```text
realm pane=1 kind=6 badge=RLM ro=true reason=ok
play pane=1 kind=7 mode=2 badge=GB ro=true glb_badge=GLB txt_pane=0
gated pane=0 reason=flag_off safe_uri=true
refuse trav=(true,unsafe_path) ext=(true,untrusted_external_url) uri=(true,true,true)
ok=true
```
