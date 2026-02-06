import { test, expect } from "bun:test";
import { Schema } from "effect";

import * as Tool from "../src/tool.js";

test("Tool.inputJsonSchema generates a JSON schema root", () => {
  const Input = Schema.Struct({
    handle: Schema.NonEmptyString.annotations({
      description: "What to call the user."
    })
  }).annotations({
    description: "Set a user handle."
  });

  const contract = Tool.make({
    name: "user_set_handle",
    description: "Set the user's handle.",
    input: Input
  });

  const schema = Tool.inputJsonSchema(contract);

  expect(schema).toBeTruthy();
  expect((schema as any).type).toBe("object");
  expect((schema as any).properties?.handle?.type).toBe("string");
});

