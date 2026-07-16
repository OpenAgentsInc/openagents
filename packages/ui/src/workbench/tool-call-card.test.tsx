import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { dispatchWorkbenchItem, type WorkbenchToolCallDispatchItem } from "./dispatch.tsx";
import { desktopToolCallCardFixtures } from "./tool-call-card.fixtures.ts";
import { DesktopToolCallCard } from "./tool-call-card.tsx";

describe("DesktopToolCallCard", () => {
  test("every fixture renders without throwing and carries its item key + kind", () => {
    for (const fixture of desktopToolCallCardFixtures) {
      const html = renderToStaticMarkup(<DesktopToolCallCard {...fixture.props} />);
      expect(html, fixture.name).toContain(`data-timeline-key="${fixture.props.itemKey}"`);
      expect(html, fixture.name).toContain(`data-status="${fixture.props.status}"`);
    }
  });

  test("mcp: title is `SERVER · TOOL`, args render as a KEY: VALUE table", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        args={[
          { key: "repo", value: "OpenAgentsInc/openagents" },
          { key: "query", value: "is:open" },
        ]}
        itemKey="mcp-1"
        server="github"
        status="completed"
        tool="search_issues"
        toolKind="mcp"
      />,
    );
    expect(html).toContain("github · search_issues");
    expect(html).toContain(">repo<");
    expect(html).toContain(">OpenAgentsInc/openagents<");
    expect(html).toContain(">query<");
    expect(html).toContain(">is:open<");
  });

  test("mcp: falls back to the bare tool name when no server is present", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard itemKey="mcp-2" status="completed" tool="ping" toolKind="mcp" />,
    );
    expect(html).toContain(">ping<");
    expect(html).not.toContain("· ping");
  });

  test("dynamic: title is `NAMESPACE · TOOL`, same args table treatment as mcp", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        args={[{ key: "prompt", value: "Audit the pipeline" }]}
        itemKey="dyn-1"
        namespace="collab"
        status="completed"
        tool="spawnAgent"
        toolKind="dynamic"
      />,
    );
    expect(html).toContain("collab · spawnAgent");
    expect(html).toContain(">prompt<");
    expect(html).toContain(">Audit the pipeline<");
  });

  test("web: shows the query prominently and the result count", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        itemKey="web-1"
        query="TanStack Start route preloading"
        resultCount={6}
        status="completed"
        tool="webSearch"
        toolKind="web"
      />,
    );
    expect(html).toContain("Web search");
    expect(html).toContain("TanStack Start route preloading");
    expect(html).toContain("6 RESULTS");
    expect(html).toContain(">Results<");
    expect(html).toContain(">6<");
  });

  test("web: singular result count reads RESULT, not RESULTS", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        itemKey="web-2"
        query="one hit query"
        resultCount={1}
        status="completed"
        tool="webSearch"
        toolKind="web"
      />,
    );
    expect(html).toContain("1 RESULT");
    expect(html).not.toContain("1 RESULTS");
  });

  test("image: shows the honest path row and prompt/result snippet, never a real preview", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        itemKey="img-1"
        path="/Users/dev/generated/panel.png"
        resultSnippet="A flat instrument panel."
        status="completed"
        tool="imageGeneration"
        toolKind="image"
      />,
    );
    expect(html).toContain("Image generation");
    expect(html).toContain(">Path<");
    expect(html).toContain("/Users/dev/generated/panel.png");
    expect(html).toContain("A flat instrument panel.");
    expect(html).not.toContain("<img");
  });

  test("image: imageView tool name renders the Image view title", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        itemKey="img-2"
        path="/Users/dev/generated/panel.png"
        status="completed"
        tool="imageView"
        toolKind="image"
      />,
    );
    expect(html).toContain("Image view");
  });

  test("duration renders right-aligned as an uppercase MS value", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        durationMs={842.6}
        itemKey="dur-1"
        status="completed"
        tool="ping"
        toolKind="mcp"
      />,
    );
    expect(html).toContain("843MS");
  });

  test("error renders in the muted-failure row, not the default result row", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        errorMessage="Connector rate limit exceeded."
        itemKey="err-1"
        status="failed"
        tool="search_issues"
        toolKind="mcp"
      />,
    );
    expect(html).toContain('class="oa-react-tool-error"');
    expect(html).toContain("Connector rate limit exceeded.");
  });

  test("opaque base64-class blob args never render raw", () => {
    const blob = "aGVsbG93b3JsZDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejEyMzQ1Njc4OTA=";
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        args={[{ key: "payload", value: blob }]}
        itemKey="blob-1"
        status="completed"
        tool="upload"
        toolKind="mcp"
      />,
    );
    expect(html).not.toContain(blob);
    expect(html).toContain("[blob omitted]");
  });

  test("opaque blob result snippets never render raw either", () => {
    const blob = "aGVsbG93b3JsZDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejEyMzQ1Njc4OTA=";
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        itemKey="blob-2"
        resultSnippet={blob}
        status="completed"
        tool="upload"
        toolKind="mcp"
      />,
    );
    expect(html).not.toContain(blob);
    expect(html).toContain("[blob omitted]");
  });

  test("running mcp call shows a progress tick when McpToolCallProgress data is present", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        itemKey="prog-1"
        progressMessage="Searching issues…"
        status="running"
        tool="search_issues"
        toolKind="mcp"
      />,
    );
    expect(html).toContain('class="oa-react-tool-progress"');
    expect(html).toContain("Searching issues…");
  });

  test("appContext renders as a badge when present", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        appContext="Linear · Engineering"
        itemKey="appctx-1"
        status="completed"
        tool="create_task"
        toolKind="mcp"
      />,
    );
    expect(html).toContain('class="oa-react-tool-badge"');
    expect(html).toContain("Linear · Engineering");
  });

  test("with no args/result/error, running shows a waiting placeholder and completed shows an honest empty row", () => {
    const running = renderToStaticMarkup(
      <DesktopToolCallCard itemKey="empty-1" status="running" tool="noop" toolKind="mcp" />,
    );
    expect(running).toContain("Waiting for result…");
    const completed = renderToStaticMarkup(
      <DesktopToolCallCard itemKey="empty-2" status="completed" tool="noop" toolKind="mcp" />,
    );
    expect(completed).toContain("No additional detail recorded.");
  });

  test("legacy passthrough props (body/summary/meta/label) still render as-is for existing consumers", () => {
    const html = renderToStaticMarkup(
      <DesktopToolCallCard
        body={<p>legacy body</p>}
        itemKey="legacy-1"
        label="Legacy label"
        meta="legacy-meta"
        status="completed"
        summary="legacy summary"
        toolKind="web"
      />,
    );
    expect(html).toContain("Legacy label");
    expect(html).toContain("legacy summary");
    expect(html).toContain("legacy-meta");
    expect(html).toContain("legacy body");
    expect(html).not.toContain("Web search");
  });

  test("dispatchWorkbenchItem's toolCall branch (#8864) passes the structured payload straight through", () => {
    const item: WorkbenchToolCallDispatchItem = {
      kind: "toolCall",
      source: "codex",
      callKind: "mcp",
      tool: "search_issues",
      server: "github",
      args: [{ key: "repo", value: "OpenAgentsInc/openagents" }],
      resultSnippet: "3 open issues found.",
      durationMs: 842,
      status: "completed",
    };
    const html = renderToStaticMarkup(
      dispatchWorkbenchItem(item, { itemKey: "dispatch-toolcall-1" }),
    );
    expect(html).toContain("github · search_issues");
    expect(html).toContain(">repo<");
    expect(html).toContain("OpenAgentsInc/openagents");
    expect(html).toContain("3 open issues found.");
    expect(html).toContain("842MS");
    expect(html).toContain('data-timeline-key="dispatch-toolcall-1"');
  });
});
