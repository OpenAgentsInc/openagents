import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

const cssPath = fileURLToPath(new URL("./effect-native-theme.css", import.meta.url));

// Aiur retains Effect Native's historical variable names while consuming the
// generated shared web projection directly. This guard rejects copied color
// literals and missing aliases.
describe("effect-native-theme.css aliases the shared web projection", () => {
  const css = readFileSync(cssPath, "utf8");

  const cases: ReadonlyArray<readonly [string, string]> = [
    ["--en-color-background", "--oa-color-background"],
    ["--en-color-surface", "--oa-color-surface"],
    ["--en-color-surfaceRaised", "--oa-color-surface-raised"],
    ["--en-color-border", "--oa-color-border"],
    ["--en-color-borderStrong", "--oa-color-border-strong"],
    ["--en-color-accent", "--oa-color-accent"],
    ["--en-color-accentHover", "--oa-color-accent-hover"],
    ["--en-color-info", "--oa-color-info"],
    ["--en-color-textPrimary", "--oa-color-text-primary"],
    ["--en-color-textMuted", "--oa-color-text-muted"],
    ["--en-color-textFaint", "--oa-color-text-faint"],
    ["--en-color-success", "--oa-color-success"],
    ["--en-color-warning", "--oa-color-warning"],
    ["--en-color-danger", "--oa-color-danger"],
  ];

  test("imports the generated token CSS", () => {
    expect(css).toContain("@import '@openagentsinc/design-tokens/web.css';");
  });

  test("contains no copied color literals", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b/iu);
  });

  test.each(cases)("%s aliases %s", (legacyName, sharedName) => {
    expect(css).toContain(`${legacyName}: var(${sharedName});`);
  });
});
