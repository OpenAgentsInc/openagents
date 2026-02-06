import { JSONSchema, Schema } from "effect";

import {
  Params,
  PromptIR,
  Signature,
  SignatureContract,
  type DseSignature
} from "@openagentsinc/dse";

export type DseModuleContractExportV1 = {
  readonly format: "openagents.dse.module_contract";
  readonly formatVersion: 1;
  readonly moduleId: string;
  readonly description: string;
  readonly signatureIds: ReadonlyArray<string>;
};

const BootstrapMessage = Schema.Struct({
  message: Schema.String.annotations({
    description: "The user's raw message text."
  })
}).annotations({
  description: "A single user message."
});

const Handle = Schema.NonEmptyString.annotations({
  description: "What to call the user (handle/nickname)."
});

const AgentName = Schema.NonEmptyString.annotations({
  description: "What the user should call the agent."
});

const Vibe = Schema.NonEmptyString.annotations({
  description: "One short phrase describing the vibe."
});

const defaultParams = {
  ...Params.emptyParamsV1,
  decode: { mode: "strict_json", maxRepairs: 0 }
} satisfies Params.DseParamsV1;

export const signatures = {
  bootstrap_extract_user_handle: Signature.make({
    id: "@openagents/autopilot/bootstrap/ExtractUserHandle.v1",
    input: BootstrapMessage,
    output: Schema.Struct({
      handle: Handle
    }).annotations({
      description: "Extracted handle for the user."
    }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are a strict information extraction module for Autopilot bootstrap."
        ),
        PromptIR.instruction(
          "Given the user's message, extract what Autopilot should call the user.\n" +
            "- Output MUST be JSON only.\n" +
            "- If the user provides multiple options, pick the most explicit handle.\n" +
            "- If the message is empty/meaningless, pick \"Unknown\"."
        ),
        PromptIR.fewShot([
          {
            id: "ex1",
            input: { message: "Call me Ada." },
            output: { handle: "Ada" }
          },
          {
            id: "ex2",
            input: { message: "Chris" },
            output: { handle: "Chris" }
          }
        ]),
        PromptIR.outputJsonSchema(
          JSONSchema.make(
            Schema.Struct({
              handle: Handle
            })
          )
        )
      ]
    },
    defaults: { params: defaultParams }
  }),

  bootstrap_extract_agent_name: Signature.make({
    id: "@openagents/autopilot/bootstrap/ExtractAgentName.v1",
    input: BootstrapMessage,
    output: Schema.Struct({
      name: AgentName
    }).annotations({
      description: "Extracted name for the agent."
    }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are a strict information extraction module for Autopilot bootstrap."
        ),
        PromptIR.instruction(
          "Given the user's message, extract the agent name the user wants to use.\n" +
            "- Output MUST be JSON only.\n" +
            "- If the user gives a sentence, extract the name-like phrase.\n" +
            "- Keep it short.\n" +
            "- If missing, use \"Autopilot\"."
        ),
        PromptIR.fewShot([
          {
            id: "ex1",
            input: { message: "Call yourself Kranox." },
            output: { name: "Kranox" }
          },
          {
            id: "ex2",
            input: { message: "Autopilot" },
            output: { name: "Autopilot" }
          }
        ]),
        PromptIR.outputJsonSchema(
          JSONSchema.make(
            Schema.Struct({
              name: AgentName
            })
          )
        )
      ]
    },
    defaults: { params: defaultParams }
  }),

  bootstrap_extract_agent_vibe: Signature.make({
    id: "@openagents/autopilot/bootstrap/ExtractAgentVibe.v1",
    input: BootstrapMessage,
    output: Schema.Struct({
      vibe: Vibe
    }).annotations({
      description: "Extracted vibe phrase."
    }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are a strict information extraction module for Autopilot bootstrap."
        ),
        PromptIR.instruction(
          "Given the user's message, extract a short vibe phrase.\n" +
            "- Output MUST be JSON only.\n" +
            "- The vibe should be 2-8 words.\n" +
            "- If the user provides multiple adjectives, keep the most salient ones."
        ),
        PromptIR.fewShot([
          {
            id: "ex1",
            input: { message: "Be calm and direct. Like a terminal." },
            output: { vibe: "calm, direct, terminal-like" }
          }
        ]),
        PromptIR.outputJsonSchema(
          JSONSchema.make(
            Schema.Struct({
              vibe: Vibe
            })
          )
        )
      ]
    },
    defaults: { params: defaultParams }
  })
} satisfies Record<string, DseSignature<any, any>>;

export const modules: ReadonlyArray<DseModuleContractExportV1> = [
  {
    format: "openagents.dse.module_contract",
    formatVersion: 1,
    moduleId: "@openagents/autopilot/BootstrapFlow.v1",
    description:
      "Bootstrap flow module: extract user handle, agent name, and vibe using DSE signatures, then persist via Blueprint tools.",
    signatureIds: [
      signatures.bootstrap_extract_user_handle.id,
      signatures.bootstrap_extract_agent_name.id,
      signatures.bootstrap_extract_agent_vibe.id
    ]
  }
];

export function signatureContractsExport() {
  return Object.values(signatures).map((s) =>
    SignatureContract.exportContractV1(s as DseSignature<any, any>)
  );
}

export function moduleContractsExport() {
  return modules;
}
