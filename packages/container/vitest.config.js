import { effectDefaults } from "@effect/vitest";
import { defineConfig } from "vitest/config";
export default defineConfig({
    test: {
        ...effectDefaults,
        coverage: {
            provider: "v8",
            reporter: ["text", "json", "html"],
            exclude: ["node_modules/", "dist/", "test/"],
        },
    },
});
//# sourceMappingURL=vitest.config.js.map