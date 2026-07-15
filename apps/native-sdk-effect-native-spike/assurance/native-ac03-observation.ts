import { Exit, Schema } from "@effect-native/core/effect";

const CodingRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

const Admission = Schema.Struct({
  grantRef: CodingRef,
  projectRef: CodingRef,
  repositoryRef: CodingRef,
  worktreeRef: CodingRef,
  workContextRef: CodingRef,
  sessionRef: CodingRef,
});

const Generation = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));
const ProcessId = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0));

export const NativeAc03ObservationSchema = Schema.Struct({
  schema: Schema.Literal("openagents.native-sdk.cw-ac-03.v1"),
  criterionRef: Schema.Literal("CW-AC-03"),
  grantSource: Schema.Literal("native_canvas_file_drop"),
  initial: Schema.Struct({
    generation: Generation,
    sidecarPid: ProcessId,
    catalogSessionCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    admission: Admission,
  }),
  restarted: Schema.Struct({
    generation: Generation,
    sidecarPid: ProcessId,
    catalogSessionCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
    admission: Admission,
  }),
  aliasCanonicalized: Schema.Boolean,
  ambientInputsExcluded: Schema.Boolean,
  privateBindingMode: Schema.String,
});

export type NativeAc03Observation = typeof NativeAc03ObservationSchema.Type;

export type NativeAc03Evaluation = Readonly<{
  criterionRef: "CW-AC-03";
  verdict: "confirmed" | "refuted";
  reason: string;
}>;

export const evaluateNativeAc03Observation = (
  candidate: unknown,
  forbiddenValues: ReadonlyArray<string> = [],
): NativeAc03Evaluation => {
  const decoded = Schema.decodeUnknownExit(NativeAc03ObservationSchema)(candidate, {
    onExcessProperty: "error",
  });
  if (!Exit.isSuccess(decoded)) {
    return { criterionRef: "CW-AC-03", verdict: "refuted", reason: "observation_schema_invalid" };
  }
  const observation = decoded.value;
  if (
    observation.initial.generation === observation.restarted.generation ||
    observation.initial.sidecarPid === observation.restarted.sidecarPid
  ) {
    return { criterionRef: "CW-AC-03", verdict: "refuted", reason: "generation_fence_stale" };
  }
  if (
    observation.initial.catalogSessionCount !== 1 ||
    observation.restarted.catalogSessionCount !== 1 ||
    !observation.aliasCanonicalized
  ) {
    return { criterionRef: "CW-AC-03", verdict: "refuted", reason: "repository_alias_duplicated" };
  }
  if (!observation.ambientInputsExcluded || observation.privateBindingMode !== "0600") {
    return {
      criterionRef: "CW-AC-03",
      verdict: "refuted",
      reason: "host_privacy_boundary_invalid",
    };
  }
  if (
    JSON.stringify(observation.initial.admission) !==
    JSON.stringify(observation.restarted.admission)
  ) {
    return { criterionRef: "CW-AC-03", verdict: "refuted", reason: "durable_identity_drift" };
  }
  const publicObservation = JSON.stringify(observation);
  if (forbiddenValues.some((value) => value.length > 0 && publicObservation.includes(value))) {
    return { criterionRef: "CW-AC-03", verdict: "refuted", reason: "ambient_identity_leak" };
  }
  return {
    criterionRef: "CW-AC-03",
    verdict: "confirmed",
    reason: "durable_native_repository_identity",
  };
};
