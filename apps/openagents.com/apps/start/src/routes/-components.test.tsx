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

  test("server-renders representative catalog anchors", () => {
    const core = renderToStaticMarkup(<ComponentsPage selectedFamily="core" />);
    const tokens = renderToStaticMarkup(<ComponentsPage selectedFamily="tokens" />);
    const dom = renderToStaticMarkup(<ComponentsPage selectedFamily="render-dom" />);
    const native = renderToStaticMarkup(<ComponentsPage selectedFamily="render-rn" />);

    expect(core).toContain("Composer");
    expect(core).toContain("Table");
    expect(tokens).toContain("khalaTheme");
    expect(dom).toContain("makeDomRenderer");
    expect(native).toContain("makeReactNativeRenderer");
  });

  test("server-renders selected component families", () => {
    const core = renderToStaticMarkup(<ComponentsPage selectedFamily="core" />);
    const tokens = renderToStaticMarkup(<ComponentsPage selectedFamily="tokens" />);
    const training = renderToStaticMarkup(<ComponentsPage selectedFamily="training" />);

    expect(core).toContain("Effect Native core");
    expect(core).toContain("No renderer-specific markup");
    expect(tokens).toContain("One semantic token authority");
    expect(training).toContain("three-effect owns visuals");
  });
});
