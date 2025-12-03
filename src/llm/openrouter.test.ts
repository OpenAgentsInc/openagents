import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { toolToOpenRouterDefinition } from "./openrouter.js";

describe("openrouter mapping", () => {
  test("converts Effect schema tool to OpenRouter function definition", () => {
    const tool = {
      name: "read",
      label: "read",
      description: "Read a file",
      schema: S.Struct({ path: S.String }),
      execute: () => null as any,
    };

    const def: any = toolToOpenRouterDefinition(tool);

    expect(def.type).toBe("function");
    expect(def.function?.name).toBe("read");
    expect(def.function?.description).toBe("Read a file");
    expect(def.function?.parameters).toHaveProperty("properties.path");
  });
});
