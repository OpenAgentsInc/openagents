# 27. Townspeople

> **Status: partly implemented.** Everglade's shared 28-person city roster and
> server-owned personas live in
> [`everglade/npc/people-words.coax`](../npc/people-words.coax) and
> [`everglade/world/people.coax`](../world/people.coax); Everglade II's
> data-driven speakers and Pansopher live in
> [`everglade/client/speakers.coax`](../client/speakers.coax),
> [`everglade/client/npc.coax`](../client/npc.coax), and
> [`everglade/world/pansopher.coax`](../world/pansopher.coax); and bounded
> dual-door guard-guide routing lives in
> [`everglade/npc/guard-guide.coax`](../npc/guard-guide.coax),
> [`everglade/npc/guard-guide-game.coax`](../npc/guard-guide-game.coax), and
> [`everglade/npc/guard-door.coax`](../npc/guard-door.coax), exercised by
> [`everglade/examples/townspeople-roster-guide.coax`](../examples/townspeople-roster-guide.coax).
> Daily multi-stop NPC schedules and large dynamic crowds walking through
> Everglade II's city clearing remain to be built.

This chapter answers the player's question: "Who are all these people in town,
and why doesn't a crowd drop my frame rate?"

A city of empty buildings feels abandoned. Filling streets with dozens of
townspeople creates two problems at once: every person needs an identity that
both the client and server agree on, and drawing or simulating everyone in town
must fit inside the 16.6 ms frame budget and 30 Hz room tick.

## What this chapter teaches

- How client and server share one roster of named townspeople so personas never
  drift and the browser never supplies system prompts.
- How static speakers render from pinned content tables with zero room
  replication traffic, while turning naturally toward nearby players.
- How authored dialogue trees stay closed (every branch leads back or exits)
  without dangling node indices.
- How remote NPC direction requests enforce strict eligibility gates, dual-door
  response verification (`elephant-1` and `gemini-decision-1`), and an 8-second
  deadline before admission.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Shared 28-person roster | Names, roles, greetings, city knowledge, and slug IDs (`EG_PEOPLE = 28`, `eg_person_of`, `eg_words_role`) | [`everglade/npc/people-words.coax`](../npc/people-words.coax) |
| Client ask payload & memory | Builds JSON requests over `metaverse-engine/talk/wire.coax` naming only the person's ID and visit stamp | [`everglade/world/people.coax`](../world/people.coax) |
| Data-driven speakers | Places `EC_STYLE_STANDS` NPCs from pinned content tables, turns toward players within 9 m, culls past 110 m | [`everglade/client/speakers.coax`](../client/speakers.coax) |
| Authored talk tree (Pansopher) | Closed 6-line, 15-choice dialogue tree validated by `talk_tree_whole` and driven by `everglade/client/npc.coax` | [`everglade/world/pansopher.coax`](../world/pansopher.coax), [`everglade/client/npc.coax`](../client/npc.coax) |
| Guard-guide observation & doors | Projects local facts into bounded `GgObservation`, verifies Elephant/Gemini wire shapes, enforces 8,000 ms deadline | [`everglade/npc/guard-guide.coax`](../npc/guard-guide.coax), [`everglade/npc/guard-guide-game.coax`](../npc/guard-guide-game.coax), [`everglade/npc/guard-door.coax`](../npc/guard-door.coax) |

**What remains to ship:** Everglade II currently places Pansopher, the Warden
speaker, the patrolling path guard, and wild creatures in the glade and forest
road. The 28-person roster serves the city persona table and v1 layout; daily
time-of-day routines (walking between homes, stalls, and the inn) and 50+
dynamic walking townspeople in Everglade II's city clearing remain to be built
alongside crowd LOD instancing.

## The method

### One roster, owned by the server

`everglade/npc/people-words.coax` declares all 28 townspeople (including three
cats—Pebble, Soot, and Wisp—whose empty `EG_ROLE` entry resolves to
`EG_CAT_ROLE`). Both the client (`everglade/world/people.coax`) and the API
load the exact same file. When a player talks to Marta (`eg_person_of("marta")
== 5`), the browser sends only `"npc":"marta"` and the conversation turns; the
server looks up Marta's role, greeting, and city knowledge locally so a modified
client can never rewrite an NPC's persona or inject instructions.

### Static speakers cost zero room bandwidth

An NPC who stands at a stall or glade edge and never walks or fights doesn't
need a 30 Hz replication row from the room server. `everglade/client/speakers.coax`
reads `EC_STYLE_STANDS` troops directly from the pinned content tables, places
each speaker at its point, turns its yaw toward you within `E2SP_TURN_AT`
(9.0 m), and skips drawing past `E2SP_FAR` (110.0 m)—matching the room's 110 m
interest radius so distant crowds never consume draw calls or bone updates.

### Bounded AI decisions with dual-door admission

When a player asks Bram the gate guard for directions, `guard-guide-game.coax`
projects only local gameplay facts—never raw chat, coordinates, or saved quest
histories—into `GgObservation`. Any blocking state (active combat, talking,
editing, altered world geometry) immediately blocks eligibility (`ggp_eligible`,
`ggp_route`). When a request goes out with `"model":"auto"`, `gg_decode` admits
only responses from two pinned doors (`GDR_ELEPHANT` or `GDR_GEMINI`) whose
five option probabilities sum to `1.0 ± 1e-6`, and `gg_admit` rechecks the
current legal bitmask (`gg_legal`) and strict `GG_TIMEOUT` (8,000 ms) deadline.

## Worked trace: asking Bram for directions

```text
Facts         followed=0 (waystone), stage=1, guard=0 (Bram), calm, near, unedited world
Projection    ggp_project(f) -> goal=2 ("waystone"), eligible=true, destination_available=true
Baseline      gg_baseline(obs) = 2 ("east_gate")
Legal mask    gg_legal(obs) = 1 | (1 << 2) = 5  (only 0 "quiet" and 2 "east_gate" are legal)
Wire decode   GG_RESPONSE        -> ok=true, action=2, door=1 (GDR_ELEPHANT)
              GG_GEMINI_RESPONSE -> ok=true, action=2, door=2 (GDR_GEMINI)
Admission     elapsed=420 ms (< 8000 ms), action=2 in legal mask -> admitted
Refusals      combat=true   -> ggp_eligible = false
              edit_count=1  -> ggp_route = false
              elapsed=8000  -> gg_admit.ok = false (timeout is exclusive)
              action=4      -> gg_admit.ok = false (west_gate not in legal mask 5)
```

## Runnable example

[`everglade/examples/townspeople-roster-guide.coax`](../examples/townspeople-roster-guide.coax)
runs the 28-person roster lookup, Pansopher's talk tree validation, dual-door
guard-guide decoding, and four explicit refusal paths (committed transcript
[`everglade/examples/townspeople-roster-guide.out`](../examples/townspeople-roster-guide.out)):

<!-- source: everglade/examples/townspeople-roster-guide.coax townspeople-roster-guide -->
```jai
townspeople_run :: () -> bool {
  // 1. Shared 28-person city roster: client and server resolve identical slug IDs & roles.
  marta := eg_person_of("marta");
  anselm := eg_person_of("brother-anselm");
  cat := eg_person_of("pebble");
  nobody := eg_person_of("nobody");
  cat_is_cat := eg_words_role(cat) == EG_CAT_ROLE;
  print("people=% marta=% anselm=% pebble=% cat_role=% unknown_refused=%\n",
    EG_PEOPLE, marta, anselm, cat, cat_is_cat, nobody == -1);

  // 2. Authored talk tree (Pansopher): every branch closes cleanly without dangling nodes.
  choices := pan.PANSOPHER_CHOICES;
  tree_ok := talk_tree_whole(pan.PANSOPHER_LINES, choices[..]);
  offered: [TALK_TREE_CHOICES] int;
  n0 := talk_tree_offered(choices[..], 0, offered[..]);
  to1 := talk_tree_pick(choices[..], 0, 0);
  to_end := talk_tree_pick(choices[..], 0, 3);
  print("tree_whole=% root_choices=% pick0_line=% pick3_end=%\n",
    tree_ok, n0, to1, to_end == TALK_TREE_END);

  // 3. Bounded guard-guide observation projection and dual-door verification (Elephant + Gemini).
  f := GgpFacts.{
    followed = 0, stage = 1, guard = 0, help_requested = true,
    gameplay = true, foreground = true, player_alive = true,
    guard_calm = true, guard_alive = true, guard_full_health = true,
    guard_visible = true, guard_near = true,
    world_known = true, target_present = true, destination_present = true
  };
  obs := ggp_project(f);
  base := gg_baseline(obs);
  legal := gg_legal(obs);
  el := gg_decode(GG_RESPONSE);
  gm := gg_decode(GG_GEMINI_RESPONSE);
  ad_el := gg_admit(obs, el, 420);
  ad_gm := gg_admit(obs, gm, 610);
  print("eligible=% goal=% base=% legal=% elephant_door=% gemini_door=% admit_el=% admit_gm=%\n",
    obs.eligible, obs.goal, base, legal, el.door, gm.door,
    ad_el.ok && ad_el.action == 2, ad_gm.ok && ad_gm.action == 2);

  // 4. Refusals: player in combat blocks offer; edited world blocks route; 8000 ms deadline
  //    and illegal out-of-mask actions are refused by gg_admit.
  f_combat := f; f_combat.combat = true;
  f_edited := f; f_edited.edit_count = 1;
  late := gg_admit(obs, el, GG_TIMEOUT);
  wrong := gg_admit(obs, GgAnswer.{ok = true, action = 4, door = GDR_ELEPHANT}, 100);
  print("refuse_combat=% refuse_edited=% refuse_timeout=% refuse_illegal=%\n",
    !ggp_eligible(f_combat), !ggp_route(f_edited), !late.ok, !wrong.ok);

  return EG_PEOPLE == 28 && marta == 5 && anselm == 17 && cat == 4 && cat_is_cat &&
         nobody == -1 && tree_ok && n0 == 4 && to1 == 1 && to_end == TALK_TREE_END &&
         obs.eligible && base == 2 && legal == 5 &&
         el.ok && el.door == GDR_ELEPHANT && gm.ok && gm.door == GDR_GEMINI &&
         ad_el.ok && ad_gm.ok && !ggp_eligible(f_combat) && !ggp_route(f_edited) &&
         !late.ok && !wrong.ok;
}
```

Output (`coax/bin/coax run everglade/examples/townspeople-roster-guide.coax`, identical in `--release`):

```text
people=28 marta=5 anselm=17 pebble=4 cat_role=true unknown_refused=true
tree_whole=true root_choices=4 pick0_line=1 pick3_end=true
eligible=true goal=2 base=2 legal=5 elephant_door=1 gemini_door=2 admit_el=true admit_gm=true
refuse_combat=true refuse_edited=true refuse_timeout=true refuse_illegal=true
ok=true
```

## The checks

```shell
coax/bin/coax test --library everglade/npc/people-words.coax
coax/bin/coax test --library everglade/world/pansopher.coax
coax/bin/coax test --library everglade/npc/guard-guide.coax
coax/bin/coax test --library everglade/npc/guard-guide-game.coax
coax/bin/coax run everglade/examples/townspeople-roster-guide.coax
```

## Mistakes

- **Sending persona prompts from the browser.** Earlier chat prototypes sent an
  NPC's description and city facts from the page, letting client edits rewrite
  character personas or leak drift between client and server. `#13261` moved
  personas into shared `everglade/npc/people-words.coax` and enforced that
  `eg_ask_body` transmits only the person's slug ID.
- **Cross-door field mixing.** Allowing an Elephant model name alongside
  Gemini's service block (or vice versa) would misattribute latency and quality
  metrics. `guard-guide.coax` binds each door's model, identity, provider, and
  number format together and tests every cross-door swap for refusal.

## What comes next

Townspeople need something to do with you: give quests, hand out gear, and
remember what happened last time. [Chapter 28](ch028-interactions.md) covers
content-driven quests, the item ledger, and NPC memory.
