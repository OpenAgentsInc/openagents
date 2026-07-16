import type { DesktopToolCallCardProps } from "./tool-call-card.tsx";

/**
 * Fixture set for `DesktopToolCallCard` (T7 #8864, epic #8857 Wave 2).
 *
 * Covers all four `callKind`s across running/completed/failed states, with
 * and without args, and with and without a result — the acceptance matrix
 * named in the issue. Kept discoverable here (not inlined into a route) so
 * the T13 `/components` workbench-family gallery lane (#8870) can import it
 * directly instead of re-deriving fixture data.
 */
export type DesktopToolCallCardFixture = Readonly<{
  name: string;
  props: DesktopToolCallCardProps;
}>;

export const desktopToolCallCardFixtures: ReadonlyArray<DesktopToolCallCardFixture> = [
  {
    name: "mcp / running / with args / no result yet",
    props: {
      itemKey: "fixture-mcp-running",
      toolKind: "mcp",
      status: "running",
      tool: "search_issues",
      server: "github",
      args: [
        { key: "repo", value: "OpenAgentsInc/openagents" },
        { key: "query", value: "is:open label:bug" },
      ],
      progressMessage: "Searching issues…",
    },
  },
  {
    name: "mcp / completed / with args + result",
    props: {
      itemKey: "fixture-mcp-completed",
      toolKind: "mcp",
      status: "completed",
      tool: "search_issues",
      server: "github",
      args: [
        { key: "repo", value: "OpenAgentsInc/openagents" },
        { key: "query", value: "is:open label:bug" },
      ],
      resultSnippet:
        "3 open issues found:\n#8861 command card\n#8862 file-change card\n#8863 reasoning streaming",
      durationMs: 842,
    },
  },
  {
    name: "mcp / completed / opaque blob arg suppressed",
    props: {
      itemKey: "fixture-mcp-blob",
      toolKind: "mcp",
      status: "completed",
      tool: "upload_asset",
      server: "assets",
      args: [
        { key: "filename", value: "screenshot.png" },
        {
          key: "payload",
          value: "aGVsbG93b3JsZDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejEyMzQ1Njc4OTA=",
        },
      ],
      resultSnippet: "Uploaded.",
      durationMs: 210,
    },
  },
  {
    name: "mcp / failed",
    props: {
      itemKey: "fixture-mcp-failed",
      toolKind: "mcp",
      status: "failed",
      tool: "search_issues",
      server: "github",
      args: [{ key: "repo", value: "OpenAgentsInc/openagents" }],
      errorMessage: "Connector rate limit exceeded.",
      durationMs: 1_204,
    },
  },
  {
    name: "mcp / completed / with appContext badge",
    props: {
      itemKey: "fixture-mcp-app-context",
      toolKind: "mcp",
      status: "completed",
      tool: "create_task",
      server: "linear",
      args: [{ key: "title", value: "Fix flaky test" }],
      resultSnippet: "Created task ENG-482.",
      durationMs: 356,
      appContext: "Linear · Engineering",
    },
  },
  {
    name: "dynamic / running / no args",
    props: {
      itemKey: "fixture-dynamic-running",
      toolKind: "dynamic",
      status: "running",
      tool: "waitAgent",
      namespace: "collab",
    },
  },
  {
    name: "dynamic / completed / with args + result",
    props: {
      itemKey: "fixture-dynamic-completed",
      toolKind: "dynamic",
      status: "completed",
      tool: "spawnAgent",
      namespace: "collab",
      args: [{ key: "prompt", value: "Audit the reasoning delta pipeline" }],
      resultSnippet: "Spawned protocol-scout.",
      durationMs: 118,
    },
  },
  {
    name: "dynamic / failed / no result",
    props: {
      itemKey: "fixture-dynamic-failed",
      toolKind: "dynamic",
      status: "failed",
      tool: "sendMessage",
      namespace: "collab",
      args: [{ key: "receiver", value: "timeline-builder" }],
      errorMessage: "Target thread already closed.",
    },
  },
  {
    name: "web / running",
    props: {
      itemKey: "fixture-web-running",
      toolKind: "web",
      status: "running",
      tool: "webSearch",
      query: "TanStack Start route preloading",
    },
  },
  {
    name: "web / completed / with results",
    props: {
      itemKey: "fixture-web-completed",
      toolKind: "web",
      status: "completed",
      tool: "webSearch",
      query: "TanStack Start route preloading",
      resultCount: 6,
      durationMs: 640,
    },
  },
  {
    name: "web / completed / no results",
    props: {
      itemKey: "fixture-web-empty",
      toolKind: "web",
      status: "completed",
      tool: "webSearch",
      query: "an unusually specific query with zero hits",
      resultCount: 0,
      durationMs: 302,
    },
  },
  {
    name: "image / completed / view with path",
    props: {
      itemKey: "fixture-image-view",
      toolKind: "image",
      status: "completed",
      tool: "imageView",
      path: "/Users/dev/Desktop/workroom-geometry.png",
    },
  },
  {
    name: "image / completed / generation with prompt + path",
    props: {
      itemKey: "fixture-image-generation",
      toolKind: "image",
      status: "completed",
      tool: "imageGeneration",
      resultSnippet: "A flat-lit instrument-panel dashboard, square corners, hairline borders.",
      path: "/Users/dev/Library/Application Support/OpenAgents/generated/panel-01.png",
      durationMs: 3_150,
    },
  },
  {
    name: "image / running / generation, no result yet",
    props: {
      itemKey: "fixture-image-generation-running",
      toolKind: "image",
      status: "running",
      tool: "imageGeneration",
    },
  },
];
