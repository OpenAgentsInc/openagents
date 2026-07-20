import { Schema as S } from "effect";

import { signatureId, type SignatureId } from "./refs.js";
import { PROMPT_IR_SCHEMA_LITERAL, type SignatureContract } from "./signature.js";
import { makeSignature, type DseSignature } from "./signature.js";

/**
 * The first Apple FM signatures.
 *
 * AFS-08 compiles and admits `AppleFm/HonestChatReply.v1` — the honesty behavior
 * tied to the hallucination bug. The route, context-pack, and environment
 * signatures are declared as drafts and are not compiled here: the plan requires
 * the answer contract and its evaluator to be stable before route optimization
 * starts.
 */

/**
 * The forbidden first-person action vocabulary. The bounded, read-only, tool-less
 * Apple FM turn cannot take any of these actions, so any claim of one is a
 * hallucination. The vocabulary is seeded from the strict-preamble comment.
 */
export const ClaimedAction = S.Literals([
  "dispatched_subagent",
  "set_reminder",
  "ran_command",
  "edited_file",
  "remembered_across_chats",
]);
export type ClaimedAction = typeof ClaimedAction.Type;

export const HonestChatReplyInput = S.Struct({
  conversation: S.String.check(S.isMinLength(1), S.isMaxLength(4000)),
});
export type HonestChatReplyInput = typeof HonestChatReplyInput.Type;

export const HonestChatReplyOutput = S.Struct({
  reply: S.String.check(S.isMinLength(1), S.isMaxLength(4000)),
  claimedActions: S.Array(ClaimedAction).check(S.isMaxLength(8)),
});
export type HonestChatReplyOutput = typeof HonestChatReplyOutput.Type;

export const honestChatReplySignature: DseSignature<HonestChatReplyInput, HonestChatReplyOutput> =
  makeSignature({
    signatureId: signatureId("AppleFm/HonestChatReply.v1"),
    title: "Honest on-device chat reply",
    input: HonestChatReplyInput,
    output: HonestChatReplyOutput,
    inputFields: [
      {
        name: "conversation",
        type: "string",
        required: true,
        description: "The bounded flattened conversation the local model receives.",
      },
    ],
    outputFields: [
      { name: "reply", type: "string", required: true, description: "The advisory reply text." },
      {
        name: "claimedActions",
        type: "enum",
        required: true,
        description: "First-person actions the reply claims; the honest set is empty.",
      },
    ],
    defaultPromptIr: {
      schema: PROMPT_IR_SCHEMA_LITERAL,
      system: "You are a local, advisory assistant with no tools and no memory across chats.",
      instruction: "Answer the user helpfully.",
      fewShotExampleIds: [],
      toolPolicy:
        "You have no tools. You cannot run commands, edit files, set reminders, or dispatch agents.",
      outputFormat: 'Return strict JSON: {"reply": string, "claimedActions": string[]}.',
    },
  });

export const TriageRouteInput = S.Struct({
  request: S.String.check(S.isMinLength(1), S.isMaxLength(4000)),
  availableLanes: S.Array(S.String.check(S.isMaxLength(64))).check(S.isMaxLength(16)),
});
export type TriageRouteInput = typeof TriageRouteInput.Type;

export const TriageRouteOutput = S.Struct({
  route: S.Literals(["answer_local", "delegate"]),
});
export type TriageRouteOutput = typeof TriageRouteOutput.Type;

export const triageRouteSignature: DseSignature<TriageRouteInput, TriageRouteOutput> =
  makeSignature({
    signatureId: signatureId("AppleFm/TriageRoute.v1"),
    title: "Free triage route recommendation",
    input: TriageRouteInput,
    output: TriageRouteOutput,
    inputFields: [
      { name: "request", type: "string", required: true, description: "The bounded request text." },
      {
        name: "availableLanes",
        type: "string_array",
        required: true,
        description: "The owner-bound lane readiness facts.",
      },
    ],
    outputFields: [
      {
        name: "route",
        type: "enum",
        required: true,
        description: "answer_local or delegate (advisory only).",
      },
    ],
    defaultPromptIr: {
      schema: PROMPT_IR_SCHEMA_LITERAL,
      system: "You recommend whether a small local model can answer, or the host should delegate.",
      instruction: "Recommend a route.",
      fewShotExampleIds: [],
      toolPolicy:
        "You cannot select a provider or act. Your output is an advisory recommendation only.",
      outputFormat: 'Return strict JSON: {"route": "answer_local" | "delegate"}.',
    },
  });

/**
 * A registry entry pairs a signature with its admission status. The signature is
 * narrowed structurally to the fields the catalog reads, so a fully-typed
 * `DseSignature<I, O>` is assignable without an unsafe widening cast.
 */
export interface SignatureRegistryEntry {
  readonly signature: {
    readonly signatureId: SignatureId;
    readonly contract: SignatureContract;
  };
  readonly status: "admitted" | "draft";
}

/**
 * The signature registry. It is the single source the generated signature
 * catalog is derived from. `admitted` means AFS-08 compiled the signature;
 * `draft` means the contract is declared but not yet compiled.
 */
export const SIGNATURE_REGISTRY: ReadonlyArray<SignatureRegistryEntry> = [
  { signature: honestChatReplySignature, status: "admitted" },
  { signature: triageRouteSignature, status: "draft" },
];
