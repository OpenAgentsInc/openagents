import { readFileSync } from "node:fs";
import path from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { TanStackLandingComparison } from "./tanstack";

describe("TanStack landing comparison", () => {
  test("renders the Astro landing snapshot through the minimal React boundary", () => {
    const html = renderToStaticMarkup(<TanStackLandingComparison />);

    expect(html).toContain("data-tanstack-landing-comparison");
    expect(html).toContain('class="site-shell"');
    expect(html).toContain("A serious place");
    expect(html).toContain("Conversation first.");
    expect(html).toContain("The work should survive the window.");
    expect(html).toContain("0.1.0-rc.12");
  });

  test("ships the frozen Astro comparison stylesheet", () => {
    const css = readFileSync(
      path.resolve(import.meta.dirname, "../../public/tanstack-landing.css"),
      "utf8",
    );

    expect(css).toContain("Generated from the exact Astro /astro landing CSS");
    expect(css).toContain(".site-shell");
    expect(css).toContain(".hero[data-astro-cid-lcdefpme]");
    expect(css).toContain(".workbench[data-astro-cid-lcdefpme]");
  });
});
