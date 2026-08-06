import { describe, expect, test } from "vite-plus/test";

import { assertThemeContrast, themeContrastViolations } from "./contrast.ts";
import { defaultTheme, khalaTheme, ThemeSchema } from "./index.ts";
import { khalaNativeTheme, projectNativeTheme } from "./native.ts";
import { renderWebDesignTokens } from "./web.ts";

describe("cross-renderer semantic projections", () => {
  test("one semantic value reaches native and web projections", () => {
    const css = renderWebDesignTokens();
    const lightNativeTheme = projectNativeTheme(defaultTheme);

    expect(khalaNativeTheme.color.attentionApproval).toBe(khalaTheme.color.attentionApproval);
    expect(khalaNativeTheme.color.attentionInput).toBe(khalaTheme.color.attentionInput);
    expect(lightNativeTheme.color.attentionWorking).toBe(defaultTheme.color.attentionWorking);
    expect(css).toContain(`--oa-color-attention-approval: ${khalaTheme.color.attentionApproval};`);
    expect(css).toContain(`--oa-color-attention-input: ${defaultTheme.color.attentionInput};`);
    expect(css).toContain("--color-oa-attention-working: var(--oa-color-attention-working);");
  });

  test("reduced-motion projections remove every duration", () => {
    expect(Object.values(khalaNativeTheme.motion.reduced)).toContain(0);
    expect(khalaNativeTheme.motion.reduced.durationFastMs).toBe(0);
    expect(khalaNativeTheme.motion.reduced.durationEnterMs).toBe(0);
    expect(khalaNativeTheme.motion.reduced.durationExitMs).toBe(0);
    expect(khalaNativeTheme.motion.reduced.durationLoopMs).toBe(0);
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
