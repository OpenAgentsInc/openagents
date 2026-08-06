import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";

const themeSource = () =>
  readFileSync(fileURLToPath(new URL("../src/ui/theme.ts", import.meta.url)), "utf8");

describe("mobile design-token projection", () => {
  test("resolves attention and layout aliases from the shared package", () => {
    const source = themeSource();
    expect(source).toContain(
      'import { khalaNativeTheme } from "@openagentsinc/design-tokens/native"',
    );
    for (const role of [
      "attentionApproval",
      "attentionInput",
      "attentionWorking",
      "attentionFailed",
      "attentionDone",
    ]) {
      expect(source).toContain(`semantic.${role}`);
    }
    expect(source).toContain('khalaNativeTheme.spacing["4"]');
    expect(source).toContain('khalaNativeTheme.radius["2xl"]');
    expect(source).toContain("khalaNativeTheme.motion.reduced.durationFastMs");
  });

  test("keeps the mobile alias layer free of literal colors", () => {
    const source = themeSource();
    expect(source).not.toMatch(/#[0-9a-f]{3,8}\b/iu);
    expect(source).not.toMatch(/rgba?\(/iu);
  });
});
