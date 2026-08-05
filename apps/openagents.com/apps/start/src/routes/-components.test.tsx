import { existsSync } from "node:fs";
import path from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { ComponentsPage } from "./-components-page";

describe("Start components workbench routes", () => {
  test("server-renders the surviving component inventory", () => {
    const html = renderToStaticMarkup(<ComponentsPage />);

    expect(html).toContain('data-route="components"');
    expect(html).toContain("Component library");
    expect(html).toContain("Internal - design-system workbench");
    expect(html).toContain("Tokens");
    expect(html).toContain("Product workbench");
    expect(html).toContain("@openagentsinc/design-tokens");
    expect(html).toContain("@openagentsinc/ui/desktop-workbench");
  });

  test("the Effect Native families are retired, not silently blank (#9325)", () => {
    // Effect Native was deleted. Four of the seven families existed only to
    // exhibit it — the core catalog storybook, the DOM and React Native
    // renderer galleries, and the Khala visual-effects catalog. They are gone
    // from the index rather than reimplemented against a framework that no
    // longer exists, and their page modules are deleted.
    const html = renderToStaticMarkup(<ComponentsPage />);

    expect(html).not.toContain("Effect Native core");
    expect(html).not.toContain("DOM renderer");
    expect(html).not.toContain("React Native renderer");
    expect(html).not.toContain("Training grammar");
    expect(html).not.toContain("Khala UI");
    expect(html).not.toContain("@effect-native");

    for (const retired of ["-components-storybook-page.tsx", "-components-khala-page.tsx"]) {
      expect(existsSync(path.resolve(import.meta.dirname, retired))).toBe(false);
    }
  });

  test("a retired family id renders the shell without inventing content", () => {
    for (const family of ["core", "render-dom", "render-rn", "training", "khala"]) {
      const html = renderToStaticMarkup(<ComponentsPage selectedFamily={family} />);
      expect(html).toContain('data-route="components"');
      expect(html).not.toContain("data-storybook-family=");
      expect(html).not.toContain("data-khala-capability=");
    }
  });

  test("server-renders the token visual reference at its real values", () => {
    const tokens = renderToStaticMarkup(<ComponentsPage selectedFamily="tokens" />);

    expect(tokens).toContain('data-storybook-family="tokens"');
    expect(tokens).toContain("Color roles");
    expect(tokens).toContain("Type scale");
    expect(tokens).toContain("Spacing");
    expect(tokens).toContain("Radius");
    expect(tokens).toContain("@openagentsinc/design-tokens · visual reference");
  });

  test("server-renders command workbench variants as real shared components", () => {
    const html = renderToStaticMarkup(<ComponentsPage selectedFamily="workbench" />);
    expect(html).toContain('data-storybook-family="workbench"');
    expect(html).toContain('data-storybook-story="command-running"');
    expect(html).toContain('data-storybook-story="command-completed"');
    expect(html).toContain('data-storybook-story="command-failed"');
    expect(html).toContain('data-storybook-story="command-capped"');
    // 4 direct DesktopCommandCard stories (T4 #8861) + 1 more rendered
    // through dispatchWorkbenchItem's "command" branch for the "declined"
    // status, which has no distinct DesktopActivityStatus of its own
    // (issue 8870, epic 8857 T13 — see -components-workbench-page.tsx's
    // "Notices and long-tail rows" section).
    expect(html.match(/data-kind="commandExecution"/g)).toHaveLength(5);
    expect(html).toContain('data-storybook-story="dispatch-command-declined"');
    expect(html).toContain("Earlier output omitted");
    expect(html).toContain('data-storybook-story="file-turn-running"');
    expect(html).toContain('data-storybook-story="file-completed"');
    expect(html).toContain('data-storybook-story="file-failed"');
    expect(html).toContain('data-storybook-story="file-capped"');
    expect(html.match(/data-kind="fileChange"/g)).toHaveLength(4);
    expect(html).toContain("PATCH: RUNNING");
    expect(html).toContain("Diff truncated");
  });
});
