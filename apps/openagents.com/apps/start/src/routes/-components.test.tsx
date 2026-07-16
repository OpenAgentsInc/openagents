import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { ComponentsPage } from "./-components-page";

describe("Start components workbench routes", () => {
  test("server-renders the active Effect Native inventory", () => {
    const html = renderToStaticMarkup(<ComponentsPage />);

    expect(html).toContain('data-route="components"');
    expect(html).toContain("Component library");
    expect(html).toContain("Internal - design-system workbench");
    expect(html).toContain("Effect Native core");
    expect(html).toContain("Tokens");
    expect(html).toContain("DOM renderer");
    expect(html).toContain("React Native renderer");
    expect(html).toContain("Training grammar");
    expect(html).toContain("Khala UI");
    expect(html).toContain("Product workbench");
  });

  test("server-renders the complete non-audio Khala visual catalog", () => {
    const html = renderToStaticMarkup(<ComponentsPage selectedFamily="khala" />);
    const capabilities = html.match(/data-khala-capability=/g) ?? [];

    expect(html).toContain('data-khala-workbench="complete"');
    expect(html).toContain('data-khala-capability-count="30"');
    expect(capabilities).toHaveLength(30);
    expect(html).toContain('data-khala-audio="excluded"');
    expect(html).toContain("radial-dial");
    expect(html).toContain("signal-separator");
    expect(html).toContain("31 easing curves");
    expect(html).toContain('data-khala-canvas="dots"');
    expect(html).toContain('data-khala-canvas="grid-lines"');
    expect(html).toContain('data-khala-canvas="moving-lines"');
    expect(html).toContain('data-khala-canvas="puffs"');
  });

  test("server-renders the complete Effect Native storybook instead of metadata", () => {
    const core = renderToStaticMarkup(<ComponentsPage selectedFamily="core" />);
    const components = core.match(/data-storybook-component=/g) ?? [];
    const stories = core.match(/data-storybook-story=/g) ?? [];

    expect(core).toContain('data-storybook-family="core"');
    expect(components).toHaveLength(79);
    expect(stories).toHaveLength(108);
    expect(core).toContain('data-storybook-story="button-primary"');
    expect(core).toContain('data-storybook-story="button-secondary"');
    expect(core).toContain('data-storybook-story="button-ghost"');
    expect(core).toContain('data-storybook-story="composer-basic"');
    expect(core).toContain('data-storybook-story="transcript-basic"');
    expect(core).toContain('data-storybook-story="diff-view-basic"');
    expect(core).toContain('data-storybook-story="modal-open"');
    expect(core).toContain("Inspect typed view");
    expect(core).toContain("background-color:#05070d");
    expect(core).toContain("<select");
    expect(core).toContain("aria-pressed=");
  });

  test("server-renders visual token and renderer family workbenches", () => {
    const tokens = renderToStaticMarkup(<ComponentsPage selectedFamily="tokens" />);
    const dom = renderToStaticMarkup(<ComponentsPage selectedFamily="render-dom" />);
    const native = renderToStaticMarkup(<ComponentsPage selectedFamily="render-rn" />);
    const training = renderToStaticMarkup(<ComponentsPage selectedFamily="training" />);

    expect(tokens).toContain('data-storybook-family="tokens"');
    expect(tokens).toContain("Color roles");
    expect(tokens).toContain("Type scale");
    expect(tokens).toContain("Spacing");
    expect(tokens).toContain("Radius");
    expect(dom).toContain('data-storybook-family="render-dom"');
    expect(dom.match(/data-storybook-story=/g)).toHaveLength(108);
    expect(native).toContain('data-storybook-family="render-rn"');
    expect(native.match(/data-storybook-story=/g)).toHaveLength(108);
    expect(training).toContain('data-storybook-family="training"');
    expect(training).toContain('data-storybook-story="graph-figure-basic"');
    expect(training).toContain('data-storybook-story="timeline-basic"');
  });

  test("server-renders command workbench variants as real shared components", () => {
    const html = renderToStaticMarkup(<ComponentsPage selectedFamily="workbench" />);
    expect(html).toContain('data-storybook-family="workbench"');
    expect(html).toContain('data-storybook-story="command-running"');
    expect(html).toContain('data-storybook-story="command-completed"');
    expect(html).toContain('data-storybook-story="command-failed"');
    expect(html).toContain('data-storybook-story="command-capped"');
    expect(html.match(/data-kind="commandExecution"/g)).toHaveLength(4);
    expect(html).toContain("Earlier output omitted");
  });
});
