import { describe, expect, test } from "vite-plus/test";

import { assertThemeContrast, themeContrastViolations } from "./contrast.ts";
import { defaultTheme, khalaTheme, ThemeSchema } from "./index.ts";
import { renderWebDesignTokens } from "./web.ts";

describe("web semantic projections", () => {
  test("semantic values reach the web projection", () => {
    const css = renderWebDesignTokens();

    expect(css).toContain(`--oa-color-attention-approval: ${khalaTheme.color.attentionApproval};`);
    expect(css).toContain(`--oa-color-attention-input: ${defaultTheme.color.attentionInput};`);
    expect(css).toContain("--color-oa-attention-working: var(--oa-color-attention-working);");
  });

  test("the reduced-motion projection removes loop duration", () => {
    expect(renderWebDesignTokens()).toContain("--oa-motion-loop: 0ms;");
  });
});

describe("WCAG AA token gate", () => {
  test.each([
    ["khalaTheme", khalaTheme],
    ["defaultTheme", defaultTheme],
  ] as const)("%s has no normal-text contrast violations", (_name, theme) => {
    expect(themeContrastViolations(theme)).toEqual([]);
  });

  test("fails a build when a text-bearing token loses contrast", () => {
    const invalid = ThemeSchema.make({
      ...khalaTheme,
      color: {
        ...khalaTheme.color,
        textBody: khalaTheme.color.background,
      },
    });

    expect(() => assertThemeContrast("invalidFixture", invalid)).toThrow(/textBody.*4\.5:1/);
  });
});
