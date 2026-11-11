# Unreal Engine 6 (UE6)

Research notes on Unreal Engine 6, written 2026-11-11. Every claim has a link.
Anything we could not confirm against a primary source is marked **unverified**.
Epic's own pages returned HTTP 403 to our fetcher on 2026-11-11, so most
quotes below come from press coverage of Epic's statements, not Epic's pages.

## Summary

- Epic plans UE6 as one engine that merges UE5 and Unreal Editor for Fortnite
  (UEFN). Early Access is targeted for the end of 2027
  ([GameFromScratch](https://gamefromscratch.com/unreal-deprecating-blueprints-and-actors/)).
- In June 2026 Epic said Actors and Blueprints would ship in UE6 and be
  deprecated later. On 2026-11-03 at Unreal Fest Tokyo, Epic changed the plan:
  Actors, Blueprints and the C++ gameplay framework are removed from the start
  of Early Access
  ([Inven Global](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6),
  [GameBusiness.jp](https://www.gamebusiness.jp/article/2026/11/05/28559.html)).
- Gameplay moves to Verse and Scene Graph (entities and components). Content
  creation moves to MCP and Python.
- Epic says AI-assisted tools will convert Blueprints and C++ gameplay code to
  Verse, and that Fortnite is the first conversion.
- The UE 5.9 source we built already contains UE6 toolsets at a pinned
  changelist. That is the part of UE6 we can use today.

## What Epic has said

| Date | Statement | Source |
|---|---|---|
| 2026-06-17 (Unreal Fest Chicago, State of Unreal) | UE6 merges UE5 and UEFN. Early Access targeted for end of 2027, full release 12 to 18 months later. Actors and Blueprints stay in the first version and are deprecated once the new framework is mature. (One German source gives May; the date is from most sources.) | [GameFromScratch](https://gamefromscratch.com/unreal-deprecating-blueprints-and-actors/), [Automaton](https://automaton-media.com/articles/newsjp/20260618-450312/) |
| 2026-06-22 | Epic blog "The road to Unreal Engine 6" (Marcus Wassmer) describes Scene Graph, a gameplay framework built from scratch on Verse. We could not read the page ourselves (HTTP 403); the content is reported second-hand. | [Epic](https://www.unrealengine.com/news/the-road-to-ue-6) (not read), [Digit](https://www.digit.in/features/gaming/epic-is-rebuilding-unreal-engine-from-the-ground-up-heres-everything-thats-changing-with-ue6.html/amp/) |
| 2026-11-03 (Unreal Fest Tokyo keynote) | Tim Sweeney announces Blueprints, Actors and the C++ gameplay framework are removed from UE6, not deprecated. He calls it a "painful change" and apologizes. Slides say "Remove". | [Inven Global](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6), [GameBusiness.jp](https://www.gamebusiness.jp/article/2026/11/05/28559.html) |
| 2026-11-03 | Sweeney posts on X about building an engine more powerful than UE5, easier than Unity, and best for AI acceleration, and about a "giant tangle of blueprints and verse, actors and entities". | [X post](https://x.com/TimSweeneyEpic/status/2117836715813716324) (**unverified**: we could not fetch it; the text comes from the owner's screenshots) |
| 2026-11-03 | Sweeney, on X: leading models "saw little Verse organically during training, but that will change by January." | [Inven Global](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6) |

The June and November statements conflict. The November one is later and is
the current plan.

## Removed and new

| Removed from UE6 (as announced 2026-11-03) | New in UE6 |
|---|---|
| Actors | Entities and components (Scene Graph) |
| Blueprints | Verse programming |
| C++ gameplay framework | Verse gameplay APIs |
| | MCP and Python for content creation |

C++ stays in lower engine layers such as rendering and physics
([Inven Global](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6),
[GameBusiness.jp](https://www.gamebusiness.jp/article/2026/11/05/28559.html)).
Inven reports MCP and Python as part of the slide; GameBusiness.jp does not
mention them, so treat MCP and Python as single-source. Verse's implementation
is public on GitHub but "not intended for general adoption at this point" as of
June ([GameFromScratch](https://gamefromscratch.com/unreal-deprecating-blueprints-and-actors/)).

## Migration plan

- **AI-assisted conversion.** Epic plans tools to convert Blueprints and C++
  gameplay code to Verse. The output format is not decided
  ([Inven Global](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6)).
- **Fortnite as proof.** Fortnite is moving thousands of Blueprints and its
  remaining C++ gameplay code to Verse. Sweeney says Epic is "feeling the pain
  first" (same source). That Fortnite has "the most Blueprints" is from the
  owner's screenshot; **unverified** elsewhere.
- **Early Access.** End of 2027, without Actors and Blueprints
  (same source). Full release 12 to 18 months later
  ([GameFromScratch](https://gamefromscratch.com/unreal-deprecating-blueprints-and-actors/)).
- **UE5 stays an option.** Projects in production can stay on UE5
  ([Inven Global](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6)).
  GameFromScratch (June) says Epic planned no further official UE5 release
  after 5.8, though a 5.9 was possible; 5.9 is now in preview.

## Evidence in the UE 5.9 source

Checked read-only in `/Users/christopherdavid/UnrealEngine-5.9/src/Engine`:

- `Plugins/Experimental/Toolsets/UE59-DISABLED.md` says the folder "holds the
  UE6 toolsets at one pinned changelist" and lists ten toolsets that 5.9
  cannot build because they need UE6 engine APIs (CommonUI, CaptureManager,
  MetaHuman, Fab, MassGameplay, CrashDiagnostics, MutableAgentSkills).
- Source comments cite UE6 changelists, for example `//UE6/Main 59595023`
  (navigation invokers) and `55795088` (the Frozen to Passive rename). So UE6
  is a live `//UE6/Main` stream that Epic backports tooling from.
- `MassGameplayToolset` exists but is disabled in 5.9. It needs
  `Mass/Debugger.h` and the `MassEntityConfigAsset` trait API. Mass is
  Epic's existing entity-component-system (ECS) framework and is a likely
  base for entity work. That link is our inference, not an Epic statement.
- `Plugins/Experimental/ToolsetRegistry`, `PythonScriptPlugin` and about 50
  toolset plugins match the "MCP + Python" slide. See [mcp-tools.md](mcp-tools.md).
- No Scene Graph plugin is in the 5.9 tree (a search for `*scenegraph*`
  found nothing). Verse appears only in two test plugins
  (`Plugins/Tests/VerseModuleIndependenceA` and `B`). Scene Graph and
  Verse gameplay APIs are not usable in 5.9.

## What it means for OpenAgents

- **Draconia, Mayor (C++ and Blueprints).** These stay on UE5.x. Nothing in
  UE6 helps them before late 2027, and a port means rewriting gameplay in
  Verse. Avoid adding new Blueprint-heavy systems we would have to convert.
  Keep gameplay logic thin and data-driven where we can.
- **Desert Blow web game.** No direct impact; it does not depend on UE6.
  (Our description of it comes from the task brief, not a code check.)
- **MCP workflow.** UE6 makes MCP a first-class path, and 5.9 already ships
  it. Our current 5.9 MCP setup is the right place to learn the tool surface.
  MetaHuman toolsets are not in 5.9, so Draconia and Mayor MetaHuman work
  gets no agent tooling until the UE6 API lands.
- **Verse.** The UE6 gameplay language is Verse. Learning it now (through
  UEFN docs) is the cheapest preparation. Epic expects AI models to know
  Verse by January 2027, which also helps our agents.
- **AI-assisted migration** is the same kind of work our agents already do.
  We can pilot it by converting one small Blueprint to Verse once Epic ships
  tooling.

## Risks and open questions

- Epic reversed its June plan in five months. Further changes are possible.
- Conversion output format and tool quality are undecided.
- Developer confidence is shaky: replies to Sweeney's post and a petition to
  keep Blueprint scripting exist
  ([change.org](https://www.change.org/p/keep-blueprint-scripting-support-in-unreal-engine-6),
  [forum thread](https://forums.unrealengine.com/t/they-want-to-remove-blueprint-in-ue6/2729352)).
- Verse is Epic-controlled and its public status is "not for general adoption".
- Will Early Access run existing UE5 projects? Unknown.
- Does Scene Graph build on Mass, or replace it? Unknown.
- Licensing and royalty terms for UE6: unknown.
- Whether the 2027 date holds: unknown.

## Sources

- [Inven Global: Epic will remove Blueprints and Actors from UE6](https://www.invenglobal.com/articles/28098/epic-games-will-remove-blueprints-and-actors-from-unreal-engine-6)
- [GameBusiness.jp, 2026-11-05](https://www.gamebusiness.jp/article/2026/11/05/28559.html)
- [GameFromScratch, 2026-06-23](https://gamefromscratch.com/unreal-deprecating-blueprints-and-actors/)
- [Epic: The road to UE 6](https://www.unrealengine.com/news/the-road-to-ue-6) (403, not read)
- [Epic: State of Unreal 2026](https://www.unrealengine.com/news/state-of-unreal-2026-top-news-from-the-show) (403, not read)
- [Automaton, 2026-06-18](https://automaton-media.com/articles/newsjp/20260618-450312/)
- [Digit](https://www.digit.in/features/gaming/epic-is-rebuilding-unreal-engine-from-the-ground-up-heres-everything-thats-changing-with-ue6.html/amp/)
- [Tim Sweeney on X](https://x.com/TimSweeneyEpic/status/2117836715813716324) (unverified)
- [Epic forum thread](https://forums.unrealengine.com/t/they-want-to-remove-blueprint-in-ue6/2729352)
- UE 5.9 source tree at `/Users/christopherdavid/UnrealEngine-5.9/src`
