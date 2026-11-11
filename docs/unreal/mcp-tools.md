# UE 5.9 MCP tools

Everything here comes from reading the 5.9 source under
`Engine/Plugins/Experimental`. **Not yet verified at runtime**: tool names,
auto-start behavior under `-nullrhi`, and authentication beyond the origin
check. Run the checklist at the end once the editor is built.

## Pieces

| Plugin | Role |
|---|---|
| `ModelContextProtocol` ("Unreal MCP") | The MCP server that runs inside the editor. Enabled by default. |
| `ToolsetRegistry` | Registry that toolset plugins register into. The MCP server publishes what is registered. |
| `Toolsets/*` | About 50 plugins that each add a set of tools. |
| `AIAssistant` ("Unreal Assistant") | Epic's in-editor agent. Uses the same toolsets. Depends on `PythonScriptPlugin`, `EditorScriptingUtilities`, `FileSandbox`. |
| `MCPClientToolset` | Lets the editor connect out to other MCP servers. Disabled by default. |
| `AgentPluginToolset`, `AgentCapability` | Read Claude and Codex agent plugins and working-directory instruction files. Disabled by default. |

## The MCP server

- Default port `8000`, URL path `/mcp`.
- `bAutoStartServer` is `false`. Start the server from the editor, or turn the
  setting on.
- Settings: Editor Preferences, "Model Context Protocol". Fields include
  server port, URL path, `AdditionalAllowedOrigins`, maximum protocol version,
  `bEnableToolSearch` (default on), `bDumpMediaToDisk`, and user instructions.
- A project-level settings object holds project instructions. Both are sent to
  clients at initialization. Reconnect clients after changing them.
- Tool search lets a client search a catalog instead of loading every tool
  definition up front.
- The server checks the request `Origin` against an allowlist. Keep it on
  loopback. Do not widen `AdditionalAllowedOrigins` without a reason.

## Connect Claude Code

The plugin can write client config for Claude Code (`.mcp.json`), Codex
(`.codex/config.toml`), Cursor (`.cursor/mcp.json`), Gemini
(`.gemini/settings.json`) and VS Code (`.vscode/mcp.json`). Choose the client in
the "MCP Client" section of the settings.

Manual equivalent for Claude Code, with the server running:

```sh
claude mcp add --transport http unreal http://localhost:8000/mcp
```

The editor must be open with the server started.

## Connect out to other servers

`MCPClientToolset` takes a list of server configs (`FMCPServerConfig`):

- Transport: legacy SSE, Streamable HTTP, or local stdio (launches a process).
- Auth: none, bearer token, or OAuth 2.0 (authorization code with PKCE;
  dynamic client registration when no client ID is set).
- Failed connections retry with exponential backoff.

## What the toolsets cover

- **Actors and levels** (`EditorToolset`): transforms, labels, tags,
  components, parenting, `LookAt`, bounds, actor diffs, levels, assets,
  skeletal meshes, curves, string tables, undo history, PIE.
- **Blueprints** (`BlueprintToolsets`): a Blueprint Graph DSL, compile checks,
  undo capture.
- **Console and config**: CVars and Exec commands (`ConsoleToolset`), config
  sections (`ConfigSettingsToolset`), cheats.
- **Build and test**: Live Coding compile (`LiveCodingToolset`), automation test
  discovery and execution (`AutomationTestToolset`).
- **Content systems**: PCG, Niagara, MetaSound, Sequencer, Sequencer animation
  mixer, StateTree, Gameplay Ability System, Chaos cloth, physics, geometry,
  texture graph, MVVM, data registry, game features, gameplay tags, input,
  navmesh, world conditions, widget analysis.
- **Capture and performance**: viewport video recording, scene diagnostics,
  framebuffer and editor-camera capture (`SceneCaptureToolset`), CPU, GPU and
  frame-time profiling (`PerfToolset`), rewind debugger, Slate inspector.
- **Files** (`FileSystemToolset`): read files, write text, download HTTPS
  resources.
- **Python**: through `PythonScriptPlugin`, which `AIAssistant` requires.

## Not in 5.9

`Toolsets/UE59-DISABLED.md` in the engine lists toolsets that need UE6 APIs:
CommonUI, CaptureManager, all MetaHuman toolsets, MetaHumanGenerator, Fab, Mass
gameplay, CrashDiagnostics, and MutableAgentSkills. SemanticSearchToolset and
UMGToolSet are also absent; UMG tools now register from `UMGEditor`.

Known limitation: the AnimationAssistantToolset Python test
`test_native_sequencer_binding_edits` is skipped under `-nullrhi` because
opening a Level Sequence editor there crashes.

For us: Draconia and Mayor MetaHuman scenes get no MCP tooling from 5.9. Other
scene, Blueprint, capture and test work does.

## Runtime checklist

1. Launch the built editor and enable the server (or set `bAutoStartServer`).
2. Connect Claude Code and list the tool catalog. Record the real tool names
   here.
3. Check whether the server starts under `-nullrhi` for headless runs.
4. Check what authentication, if any, applies beyond the origin check.
5. Try a capture, an automation test run, and a Live Coding compile through MCP.

## Sources

- Epic forum post "Unreal Engine 5.9 MegaJam Preview", forums.unrealengine.com
- `Engine/Plugins/Experimental/ModelContextProtocol`, `ToolsetRegistry`,
  `Toolsets/*` in the 5.9 source
