import { assertThemeContrast } from "../src/contrast.ts";
import { defaultTheme, khalaTheme } from "../src/index.ts";

assertThemeContrast("khalaTheme", khalaTheme);
assertThemeContrast("defaultTheme", defaultTheme);
console.log("design-token WCAG AA contrast checks passed");
