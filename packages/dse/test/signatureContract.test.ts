import { test, expect } from "bun:test";
import { Schema } from "effect";

import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";
import * as SignatureContract from "../src/signatureContract.js";

test("SignatureContract.exportContractV1 produces JSON schema + prompt IR", () => {
  const In = Schema.Struct({ message: Schema.String });
  const Out = Schema.Struct({ handle: Schema.NonEmptyString });

  const sig = Signature.make({
    id: "@openagents/test/ExtractHandle.v1",
    input: In,
    output: Out,
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You extract a handle."),
        PromptIR.instruction("Return JSON only."),
        PromptIR.outputJsonOnly()
      ]
    }
  });

  const exported = SignatureContract.exportContractV1(sig);

  expect(exported.format).toBe("openagents.dse.signature_contract");
  expect(exported.signatureId).toBe(sig.id);
  expect((exported.inputSchemaJson as any).type).toBe("object");
  expect(exported.promptIr.version).toBe(1);
});

