# Unreal Engine 5.9 from source

Notes on building Unreal Engine (UE) 5.9 from source on the owner's Mac, and on
the Model Context Protocol (MCP) tools that ship inside it. Written 2026-11-11.

| Doc | What it covers |
|---|---|
| [build-from-source.md](build-from-source.md) | Disk, clone, setup, build, uninstall of 5.7 and 5.8 |
| [mcp-tools.md](mcp-tools.md) | The built-in MCP server, the toolset plugins, and how we connect to them |
| [ue6.md](ue6.md) | What Epic has said about Unreal Engine 6, what it removes, and what it means for us |
| [2026-11-11-learning-from-unreal-5-9.md](2026-11-11-learning-from-unreal-5-9.md) | What Aethyr should adapt from UE 5.9, system by system, ideas only and never code |
| [web-games-in-the-verse.md](web-games-in-the-verse.md) | Hosting web-built (Wasm) Unreal games as guest worlds in the Verse: the creator's workflow, hosting, the bridge, legal, the PoC plan and owner decisions |
| [2026-11-12-splice-unreal-vs-aethyr-session-audit.md](2026-11-12-splice-unreal-vs-aethyr-session-audit.md) | Why two agent sessions building the same game came out so different in Unreal and in Aethyr, with rules for the next Unreal session |

## Summary

- Epic announced UE 5.9 as a MegaJam Preview on 2026-11-09. It is the first UE
  release built mostly with agentic workflows. There is no 5.9 release tag yet.
  We build the `5.9` branch of `github.com/EpicGames/UnrealEngine`.
- The Launcher installs of 5.7 and 5.8 are gone. 5.8 was removed on
  2026-11-11. 5.7 was not installed.
- Source lives at `/Users/christopherdavid/UnrealEngine-5.9/src`, outside any
  repo checkout.
- UE 5.9 includes an MCP server (`ModelContextProtocol` plugin) and about 50
  toolset plugins that expose editor functions to AI clients such as Claude
  Code.
- The MetaHuman toolsets are **not** in 5.9. Draconia and Mayor MetaHuman work
  gets no agent tooling from this release.
