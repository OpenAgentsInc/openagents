import { Schema as S } from "effect";

/**
 * Capability names an adapter may or may not support. There is intentionally no
 * static capability matrix on the adapter: optional behavior is signalled by the
 * presence or absence of the optional method, and a request the adapter cannot
 * satisfy fails with {@link HarnessCapabilityUnsupported}.
 *
 * This mirrors the Box v1 facade's `501 capability_not_implemented` posture, one
 * layer lower — at the runtime adapter instead of the HTTP surface. Keeping the
 * set enumerated lets conformance tests and telemetry name a refusal precisely
 * without turning it into a negotiated capability object.
 */
export const HARNESS_CAPABILITIES = [
  "compact",
  "suspend_turn",
  "continue_turn",
  "detach",
  "builtin_tool_approvals",
  "builtin_tool_filtering",
  "bootstrap",
  "sandbox",
] as const;

export type HarnessCapability = (typeof HARNESS_CAPABILITIES)[number];

export const HarnessCapabilitySchema = S.Literals(HARNESS_CAPABILITIES);

/**
 * Fail-closed refusal raised by any adapter method whose capability is absent.
 * Consumers pattern-match `capability` to decide fallback; they never assume a
 * capability from the adapter's shape.
 */
export class HarnessCapabilityUnsupported extends S.TaggedErrorClass<HarnessCapabilityUnsupported>()(
  "AgentHarness.CapabilityUnsupported",
  {
    harnessId: S.String,
    capability: HarnessCapabilitySchema,
    detail: S.optionalKey(S.String),
  },
) {}
