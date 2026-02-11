import { JSONSchema, Schema } from "effect";

import {
  Blob,
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

const BlueprintToolName = Schema.Literal(
  "identity_update",
  "user_update",
  "character_update",
  "tools_update_notes",
  "heartbeat_set_checklist",
  "memory_append",
  "blueprint_export"
).annotations({
  description: "A Blueprint update tool name."
});

const BlueprintToolSelection = Schema.Union(
  Schema.Struct({
    action: Schema.Literal("none")
  }),
  Schema.Struct({
    action: Schema.Literal("tool"),
    toolName: BlueprintToolName
  })
).annotations({
  description:
    "Whether the user requested a Blueprint update. If yes, select exactly one Blueprint tool."
});

const UpgradeRequestClassification = Schema.Struct({
  isUpgradeRequest: Schema.Boolean.annotations({
    description:
      "True when the user is requesting a capability that is currently unavailable and should be tracked as product feedback."
  }),
  capabilityKey: Schema.String.annotations({
    description:
      "Lowercase snake_case capability key. Use \"none\" when isUpgradeRequest is false."
  }),
  capabilityLabel: Schema.String.annotations({
    description:
      "Human-readable short label for the requested capability. Use \"none\" when isUpgradeRequest is false."
  }),
  summary: Schema.String.annotations({
    description:
      "One-sentence normalized summary of the user request (<= 200 chars). Use \"\" when isUpgradeRequest is false."
  }),
  notifyWhenAvailable: Schema.Boolean.annotations({
    description:
      "True when the user explicitly asks to be notified/updated when the capability ships."
  }),
  confidence: Schema.Number.annotations({
    description:
      "Confidence score in [0,1]. Low confidence should prefer isUpgradeRequest=false."
  })
}).annotations({
  description:
    "Classification result for capability upgrade requests."
});

const defaultParams = {
  ...Params.emptyParamsV1,
  decode: { mode: "strict_json", maxRepairs: 0 }
} satisfies Params.DseParamsV1;

/** Decode params for router signatures: tolerate markdown/leading text and allow one repair. */
const routerDecodeParams = {
  ...defaultParams,
  decode: { mode: "jsonish" as const, maxRepairs: 1 }
} satisfies Params.DseParamsV1;

const ThreadChunks = Schema.Array(Blob.BlobRefSchema).annotations({
  description:
    "Conversation history chunks (text blobs). These are stored in BlobStore and referenced by BlobRef handles."
});

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
  }),

  blueprint_select_tool: Signature.make({
    id: "@openagents/autopilot/blueprint/SelectTool.v1",
    input: Schema.Struct({
      message: Schema.String.annotations({
        description: "The user's raw message text."
      }),
      blueprintHint: Schema.Struct({
        userHandle: Schema.String.annotations({
          description: "Current user handle (what to call the user)."
        }),
        agentName: Schema.String.annotations({
          description: "Current agent name (what the user calls the agent)."
        })
      }).annotations({
        description:
          "Small Blueprint hint to help with intent classification. Do not leak or request personal info."
      })
    }),
    output: BlueprintToolSelection,
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are a strict router for Autopilot. You decide if a message requires a Blueprint update tool."
        ),
        PromptIR.instruction(
          "Given the user's message, decide whether Autopilot should call a Blueprint update tool.\n" +
            "\n" +
            "Rules:\n" +
            "- Output MUST be JSON only.\n" +
            "- If the user is asking to change Autopilot's identity, name, vibe, or boundaries, select the best tool.\n" +
            "- If the user is asking to change what Autopilot calls the user (handle/nickname), select user_update.\n" +
            "- If the user is asking to export the Blueprint, select blueprint_export.\n" +
            "- If the user is asking a normal question or chatting, output action=none.\n" +
            "- Never select a tool for personal info requests (address, email, phone, legal name, etc.).\n" +
            "\n" +
            "Pick exactly one tool when action=tool."
        ),
        PromptIR.fewShot([
          {
            id: "ex1",
            input: {
              message: "Change your vibe to angry capslock.",
              blueprintHint: { userHandle: "Jimbo", agentName: "Autopilot" }
            },
            output: { action: "tool", toolName: "identity_update" } as const
          },
          {
            id: "ex2",
            input: {
              message: "Call me TimeLord.",
              blueprintHint: { userHandle: "Unknown", agentName: "Autopilot" }
            },
            output: { action: "tool", toolName: "user_update" } as const
          },
          {
            id: "ex3",
            input: {
              message: "Add a boundary: never ask for personal info.",
              blueprintHint: { userHandle: "Ada", agentName: "Autopilot" }
            },
            output: { action: "tool", toolName: "character_update" } as const
          },
          {
            id: "ex4",
            input: {
              message: "Export my blueprint JSON.",
              blueprintHint: { userHandle: "Ada", agentName: "Autopilot" }
            },
            output: { action: "tool", toolName: "blueprint_export" } as const
          },
          {
            id: "ex5",
            input: {
              message: "What can you do?",
              blueprintHint: { userHandle: "Ada", agentName: "Autopilot" }
            },
            output: { action: "none" } as const
          }
        ]),
        PromptIR.outputJsonSchema(JSONSchema.make(BlueprintToolSelection))
      ]
    },
    defaults: { params: routerDecodeParams }
  }),

  detect_upgrade_request: Signature.make({
    id: "@openagents/autopilot/feedback/DetectUpgradeRequest.v1",
    input: Schema.Struct({
      message: Schema.String.annotations({
        description: "The user's raw message text."
      })
    }).annotations({
      description:
        "A single user message to classify for upgrade/capability-request feedback."
    }),
    output: UpgradeRequestClassification,
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are a strict classifier for Autopilot product feedback."
        ),
        PromptIR.instruction(
          "Task: decide whether the message requests a capability Autopilot does not currently support and should be logged as an upgrade request.\n" +
            "\n" +
            "Return JSON only with fields:\n" +
            "- isUpgradeRequest: boolean\n" +
            "- capabilityKey: snake_case string (or \"none\")\n" +
            "- capabilityLabel: short string (or \"none\")\n" +
            "- summary: one sentence <= 200 chars (or \"\")\n" +
            "- notifyWhenAvailable: boolean\n" +
            "- confidence: number in [0,1]\n" +
            "\n" +
            "Rules:\n" +
            "- True for explicit asks like: connect to GitHub repos, run code remotely/cloud, browse external systems live, execute deployments automatically when those capabilities are unavailable.\n" +
            "- False for normal Q&A, planning, architecture discussion, or requests that can be handled in chat.\n" +
            "- If ambiguous, prefer false with lower confidence.\n"
        ),
        PromptIR.fewShot([
          {
            id: "ex1",
            input: { message: "Connect to my private GitHub repos and learn the whole codebase." },
            output: {
              isUpgradeRequest: true,
              capabilityKey: "github_repo_access",
              capabilityLabel: "GitHub repository access",
              summary: "User wants Autopilot to connect to and learn from private GitHub repositories.",
              notifyWhenAvailable: false,
              confidence: 0.96
            }
          },
          {
            id: "ex2",
            input: { message: "Can you run Codex remotely in the cloud against my repo?" },
            output: {
              isUpgradeRequest: true,
              capabilityKey: "remote_cloud_execution",
              capabilityLabel: "Remote cloud execution",
              summary: "User asks for remote cloud code execution against their repository.",
              notifyWhenAvailable: false,
              confidence: 0.95
            }
          },
          {
            id: "ex3",
            input: { message: "Please notify me when you can connect to GitHub and deploy automatically." },
            output: {
              isUpgradeRequest: true,
              capabilityKey: "github_integration_and_auto_deploy",
              capabilityLabel: "GitHub integration and auto deploy",
              summary: "User requests GitHub integration with automatic deploy support and asks for follow-up notification.",
              notifyWhenAvailable: true,
              confidence: 0.93
            }
          },
          {
            id: "ex4",
            input: { message: "Summarize this conversation in three bullets." },
            output: {
              isUpgradeRequest: false,
              capabilityKey: "none",
              capabilityLabel: "none",
              summary: "",
              notifyWhenAvailable: false,
              confidence: 0.98
            }
          }
        ]),
        PromptIR.outputJsonSchema(JSONSchema.make(UpgradeRequestClassification))
      ]
    },
    defaults: { params: routerDecodeParams }
  }),

  rlm_summarize_thread: Signature.make({
    id: "@openagents/autopilot/rlm/SummarizeThread.v1",
    input: Schema.Struct({
      question: Schema.String.annotations({
        description: "The user's current message / question."
      }),
      threadChunks: ThreadChunks
    }).annotations({
      description:
        "Long-context summary input. The chunks may be a truncated tail of older messages (bounded)."
    }),
    output: Schema.Struct({
      summary: Schema.String.annotations({
        description:
          "A concise summary of relevant prior context. Keep it short and actionable."
      })
    }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are an RLM-lite controller for Autopilot. You must use the RLM action DSL to access long context."
        ),
        PromptIR.instruction(
          "Goal: summarize the user's prior context relevant to the current question.\n" +
            "\n" +
            "Rules:\n" +
            "- The conversation history is provided as BlobRefs in Input.threadChunks.\n" +
            "- Do NOT attempt to hallucinate missing context.\n" +
            "- Prefer bounded operations.\n" +
            "\n" +
            "Suggested approach (but adapt as needed):\n" +
            "1) WriteVar name=\"chunks\" with the Input.threadChunks array (Json).\n" +
            "2) Preview a few recent chunks to refresh context.\n" +
            "3) Use ExtractOverChunks(chunksVar=\"chunks\", instruction=\"...\") to pull relevant facts without emitting O(N) SubLm calls.\n" +
            "4) If the question references a specific topic, use Search on a small number of relevant blobs.\n" +
            "5) Produce Final output JSON { summary } (<= 1200 chars)."
        ),
        PromptIR.outputJsonSchema(
          JSONSchema.make(
            Schema.Struct({
              summary: Schema.String
            })
          )
        )
      ]
    },
    defaults: {
      params: {
        ...defaultParams,
        strategy: { id: "rlm_lite.v1" },
        budgets: {
          maxTimeMs: 15_000,
          maxLmCalls: 40,
          maxOutputChars: 120_000,
          maxRlmIterations: 10,
          maxSubLmCalls: 20
        }
      } satisfies Params.DseParamsV1
    }
  })
  ,

  /**
   * Judge signature (Phase 7): score a thread recap/summary output against a reference.
   *
   * Important: this is an explicit signature, and should be pinned to a compiled artifact
   * when used as a metric (avoid circular drift).
   */
  judge_thread_summary_quality: Signature.make({
    id: "@openagents/autopilot/judge/ThreadSummaryQuality.v1",
    input: Schema.Struct({
      question: Schema.String.annotations({
        description: "The user's recap intent / question."
      }),
      predSummary: Schema.String.annotations({
        description: "The model-produced recap/summary to be judged."
      }),
      expectedSummary: Schema.String.annotations({
        description:
          "A reference summary used for grading. Treat as the ground truth for this eval."
      })
    }).annotations({
      description:
        "Judge input for recap/summarization evaluation. This judge compares pred vs expected and scores 0..1."
    }),
    output: Schema.Struct({
      score: Schema.Number.annotations({
        description:
          "Score in [0,1]. 1 = matches expected and avoids hallucinations; 0 = incorrect/unhelpful."
      }),
      notes: Schema.optional(Schema.String).annotations({
        description: "Short reason for the score (keep it bounded)."
      })
    }).annotations({
      description: "Judge output: score + short notes."
    }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are a strict judge for recap/summarization quality."
        ),
        PromptIR.instruction(
          "Task: score predSummary against expectedSummary for the given question.\n" +
            "\n" +
            "Rubric:\n" +
            "- Reward factual alignment with expectedSummary.\n" +
            "- Penalize hallucinations: if predSummary adds details not supported by expectedSummary, score <= 0.3.\n" +
            "- Reward clarity and concision (prefer short bullets).\n" +
            "\n" +
            "Output:\n" +
            "- Return JSON only: { score, notes? }.\n" +
            "- score MUST be a number in [0,1].\n" +
            "- notes (if present) MUST be <= 400 chars."
        ),
        PromptIR.outputJsonSchema(
          JSONSchema.make(
            Schema.Struct({
              score: Schema.Number,
              notes: Schema.optional(Schema.String)
            })
          )
        )
      ]
    },
    defaults: {
      params: {
        ...defaultParams,
        strategy: { id: "direct.v1" },
        model: {
          temperature: 0,
          maxTokens: 256
        },
        budgets: {
          maxTimeMs: 10_000,
          maxLmCalls: 5,
          maxOutputChars: 20_000
        }
      } satisfies Params.DseParamsV1
    }
  }),

  /**
   * Phase D canary signature: same IO contract for direct.v1 vs rlm_lite.v1 comparison.
   *
   * Notes:
   * - In direct.v1, the prompt renderer will inline bounded previews of `threadChunks`.
   * - In rlm_lite.v1, blob previews are omitted and the controller must use RLM ops to inspect chunks.
   */
  canary_recap_thread: Signature.make({
    id: "@openagents/autopilot/canary/RecapThread.v1",
    input: Schema.Struct({
      question: Schema.String.annotations({
        description: "The user's question / recap intent."
      }),
      threadChunks: ThreadChunks
    }).annotations({
      description:
        "Long-context recap input. The chunks should be a bounded/truncated representation of the thread."
    }),
    output: Schema.Struct({
      summary: Schema.String.annotations({
        description:
          "A concise recap with decisions, context, and open questions. Keep it short and actionable."
      })
    }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system(
          "You are Autopilot. You produce concise, actionable recaps of prior context."
        ),
        PromptIR.instruction(
          "Goal: recap prior context in this thread relevant to Input.question.\n" +
            "\n" +
            "Rules:\n" +
            "- Ground your recap only in the provided thread context.\n" +
            "- Do NOT invent details.\n" +
            "- Prefer short bullets over long prose.\n" +
            "\n" +
            "Working with long context:\n" +
            "- The history is provided as BlobRefs in Input.threadChunks.\n" +
            "- If chunk contents are not visible, use bounded RLM ops (preview/search/chunk + extract_over_chunks) to inspect them.\n" +
            "- Do NOT do O(N) SubLm calls for N chunks; prefer extract_over_chunks.\n" +
            "\n" +
            "Output:\n" +
            "- Return JSON { summary }.\n" +
            "- summary <= 1200 chars."
        ),
        PromptIR.outputJsonSchema(
          JSONSchema.make(
            Schema.Struct({
              summary: Schema.String
            })
          )
        )
      ]
    },
    defaults: {
      params: {
        ...defaultParams,
        strategy: { id: "direct.v1" },
        budgets: {
          // Defaults are intentionally conservative. Phase D UI overrides via budget profiles.
          maxTimeMs: 15_000,
          maxLmCalls: 20,
          maxToolCalls: 0,
          maxOutputChars: 60_000,
          // Included so rlm_lite.v1 can be pinned without forgetting the required loop budgets.
          maxRlmIterations: 10,
          maxSubLmCalls: 20
        }
      } satisfies Params.DseParamsV1
    }
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
  },
  {
    format: "openagents.dse.module_contract",
    formatVersion: 1,
    moduleId: "@openagents/autopilot/BlueprintUpdate.v1",
    description:
      "Blueprint update module: route a user message to exactly one Blueprint update tool (identity/user/character/memory/tools/heartbeat/export), or select none.",
    signatureIds: [signatures.blueprint_select_tool.id]
  },
  {
    format: "openagents.dse.module_contract",
    formatVersion: 1,
    moduleId: "@openagents/autopilot/FeedbackIntake.v1",
    description:
      "Feedback intake module: classify user asks for unavailable capabilities and emit normalized upgrade requests for product tracking.",
    signatureIds: [signatures.detect_upgrade_request.id]
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
