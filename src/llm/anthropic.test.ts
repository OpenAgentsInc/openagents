import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { toolToAnthropicDefinition } from "./anthropic.js";

describe("anthropic mapping", () => {
  test("converts Effect schema tool to Anthropic input_schema", () => {
    const tool = {
      name: "read",
      description: "Read a file",
      schema: S.Struct({ path: S.String }),
      execute: () => null as any,
    };

    const def = toolToAnthropicDefinition(tool);

    expect(def.name).toBe("read");
    expect(def.description).toBe("Read a file");
    expect((def.input_schema as any).properties.path.type).toBe("string");
  });
});
