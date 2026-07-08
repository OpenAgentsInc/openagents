import { Effect, Schema as S } from "effect"
import {
  PSIONIC_QWEN_BACKEND_KIND,
  PSIONIC_QWEN_DEFAULT_BASE_URL,
  PSIONIC_QWEN_DEFAULT_MODEL_ID,
  PSIONIC_QWEN_LOCAL_PROFILE_ID,
} from "./contract.js"

export type PsionicQwenReadiness = Readonly<{
  ready: false
  status: "archived" | "unreachable" | "malformed"
  message: string
  profile: {
    id: typeof PSIONIC_QWEN_LOCAL_PROFILE_ID
    kind: typeof PSIONIC_QWEN_BACKEND_KIND
    baseUrl: typeof PSIONIC_QWEN_DEFAULT_BASE_URL
    baseUrlSource: "archived"
    model: typeof PSIONIC_QWEN_DEFAULT_MODEL_ID
  }
  modelIds: ReadonlyArray<string>
  modelRefs: ReadonlyArray<string>
  observedModelRefs: ReadonlyArray<string>
  supportedEndpointRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  receipt: Readonly<Record<string, unknown>>
  health?: Readonly<Record<string, string | undefined> & { models?: ReadonlyArray<string> }>
}>

export type PsionicQwenCompleteResult = Readonly<{
  text: string
  roundTrips: number
  receipt: { usage: Readonly<Record<string, unknown>> }
}>

export class PsionicQwenClientError extends S.TaggedErrorClass<PsionicQwenClientError>()(
  "PsionicQwenClientError",
  {
    reason: S.String,
    failureClass: S.String,
    receipt: S.optional(S.Record(S.String, S.Unknown)),
  },
) {}

const readiness = (): PsionicQwenReadiness => ({
  ready: false,
  status: "archived",
  message: "Psionic Qwen backend was archived to backroom with the retired Tassadar/Psionic program.",
  profile: {
    id: PSIONIC_QWEN_LOCAL_PROFILE_ID,
    kind: PSIONIC_QWEN_BACKEND_KIND,
    baseUrl: PSIONIC_QWEN_DEFAULT_BASE_URL,
    baseUrlSource: "archived",
    model: PSIONIC_QWEN_DEFAULT_MODEL_ID,
  },
  modelIds: [],
  modelRefs: [],
  observedModelRefs: [],
  supportedEndpointRefs: [],
  blockerRefs: ["blocker.psionic_qwen35.archived_to_backroom"],
  receipt: { archivedTo: "backroom.openagents_prune_20260708_tassadar_psionic" },
})

type ArchivedPsionicQwenClient = Readonly<{
  profile: PsionicQwenReadiness["profile"]
  doctor: () => Effect.Effect<PsionicQwenReadiness>
  complete: (_options?: unknown) => Effect.Effect<never, PsionicQwenClientError>
}>

export const makePsionicQwenClient = (
  _options?: unknown,
): Effect.Effect<ArchivedPsionicQwenClient, PsionicQwenClientError> =>
  Effect.succeed({
    profile: readiness().profile,
    doctor: () => Effect.succeed(readiness()),
    complete: (_options?: unknown) =>
      Effect.fail(
        new PsionicQwenClientError({
          reason: "Psionic Qwen backend is archived.",
          failureClass: "archived",
          receipt: { archivedTo: "backroom.openagents_prune_20260708_tassadar_psionic" },
        }),
      ),
  } satisfies ArchivedPsionicQwenClient)
