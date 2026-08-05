/**
 * The engine boundary (SWAP-0, openagents#9315).
 *
 * MKT-SWP profile logic — script and tree parsing, output-key re-derivation,
 * invoice checks, MuSig2 transcript checks, timeout ladders, exit packages,
 * the typestate fund-authorisation flow — exists once, in the Immortal
 * client crate (immortal#12), which builds for `wasm32` and takes key
 * material as bytes with no randomness source of its own. This module is the
 * Effect Schema contract that the wasm binding satisfies.
 *
 * SWAP-3 (#9318) already declared the half of this boundary its checklist
 * consumes: `FundVerifier`, `VerifyBeforeFundReport`, and the `VERIFY_CHECK_IDS`
 * row vocabulary live in `@openagentsinc/mkt-swp-compare` and are imported
 * here rather than restated. This module extends that port with the rest of
 * what can authorise funding: profile validation, exit-package construction
 * and digest binding, the funding typestate, and transaction construction.
 *
 * The host never computes a profile-level verdict. Signed records cross the
 * boundary as opaque canonical JSON and come back as typed verdicts.
 */
import { Context, Schema } from "effect";
import type { Effect } from "effect";
import type { FundVerifier, VerifyBeforeFundReport } from "@openagentsinc/mkt-swp-compare";
import { SWP_ERROR_IDENTIFIERS } from "@openagentsinc/swap-i18n";

export const ENGINE_CONTRACT_VERSION = "openagents.mkt_swp.engine.v1" as const;

/** MKT-SWP profile identity (§1). */
export const MKT_SWP_PROFILE = "mkt-swp" as const;
export const MKT_SWP_PROFILE_VERSION = 1 as const;

/** The §17 identifier vocabulary, owned by SWAP-8 and re-schema'd here. */
export const SwpErrorIdentifierSchema = Schema.Literals(SWP_ERROR_IDENTIFIERS);

/** 64-hex identifier (event IDs, session IDs, digests). */
export const Hex64Schema = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));
export type Hex64 = typeof Hex64Schema.Type;

/**
 * A signed Nostr record in canonical JSON, opaque to the host. The engine
 * parses scripts, trees, and profile members from bytes (MKT-SWP §7.1); the
 * host passes the string through and never inspects content.
 */
export const SignedRecordJsonSchema = Schema.NonEmptyString;
export type SignedRecordJson = typeof SignedRecordJsonSchema.Type;

export const EngineDescriptionSchema = Schema.Struct({
  contractVersion: Schema.Literal(ENGINE_CONTRACT_VERSION),
  profile: Schema.Literal(MKT_SWP_PROFILE),
  profileVersion: Schema.Literal(MKT_SWP_PROFILE_VERSION),
  /** Public-safe engine build provenance for the SWAP-7 provenance line. */
  source: Schema.Struct({
    repository: Schema.NonEmptyString,
    commit: Schema.optionalKey(Schema.String),
  }),
});
export interface EngineDescription extends Schema.Schema.Type<typeof EngineDescriptionSchema> {}

/** The engine's verdict on one opaque profile record. */
export const ProfileRecordVerdictSchema = Schema.TaggedUnion({
  Accepted: { kind: Schema.Number.check(Schema.isInt()) },
  Refused: { identifier: SwpErrorIdentifierSchema },
});
export type ProfileRecordVerdict = typeof ProfileRecordVerdictSchema.Type;

/**
 * The non-secret descriptor of a persisted exit package (MKT-SWP §12).
 * Seeds, private keys, preimages, macaroons, and MuSig2 secret nonces never
 * appear here; secrets stay in the local secret store behind the non-secret
 * `storageRef` handle (SWAP-4 owns that store, SWAP-5 persists this
 * descriptor and its tripwire refuses secret material).
 */
export const ExitPackageDescriptorSchema = Schema.Struct({
  sessionId: Hex64Schema,
  contractDigest: Hex64Schema,
  packageDigest: Hex64Schema,
  storageRef: Schema.NonEmptyString,
});
export interface ExitPackageDescriptor extends Schema.Schema.Type<
  typeof ExitPackageDescriptorSchema
> {}

/**
 * The typestate token for the fund action. Only the engine produces one, and
 * only from a report whose verdict is `verification_passed` with every row
 * passing, plus a digest-bound exit package. The widget state machine
 * requires this value to enter `AwaitingFunding`, so the fund action is
 * unreachable without it — the state-machine half of the same law SWAP-3's
 * `fundingGate` enforces on the checklist surface.
 */
export const FundingAuthorizationSchema = Schema.TaggedStruct("FundingAuthorization", {
  sessionId: Hex64Schema,
  contractDigest: Hex64Schema,
  exitPackageDigest: Hex64Schema,
  reportEpoch: Schema.Number.check(Schema.isInt()),
  authorizedAtEpochSeconds: Schema.Number.check(Schema.isInt()),
});
export interface FundingAuthorization extends Schema.Schema.Type<
  typeof FundingAuthorizationSchema
> {}

/** An unsigned engine-constructed transaction, opaque to the host. */
export const TransactionTemplateSchema = Schema.Struct({
  sessionId: Hex64Schema,
  purpose: Schema.Literal("funding"),
  template: Schema.NonEmptyString,
});
export interface TransactionTemplate extends Schema.Schema.Type<typeof TransactionTemplateSchema> {}

/** The signed-record set one session's verification and exit package bind. */
export const SessionRecordsSchema = Schema.Struct({
  sessionId: Hex64Schema,
  quoteEventId: Hex64Schema,
  orderEventId: Schema.NullOr(Hex64Schema),
  epoch: Schema.Number.check(Schema.isInt()),
  quote: SignedRecordJsonSchema,
  order: SignedRecordJsonSchema,
  /** The complementary requester/provider Swap Contract pair (§4.5). */
  swapContracts: Schema.NonEmptyArray(SignedRecordJsonSchema),
});
export interface SessionRecords extends Schema.Schema.Type<typeof SessionRecordsSchema> {}

export const AuthorizeFundingRequestSchema = Schema.Struct({
  sessionId: Hex64Schema,
  exitPackage: ExitPackageDescriptorSchema,
});
export interface AuthorizeFundingRequest extends Schema.Schema.Type<
  typeof AuthorizeFundingRequestSchema
> {}

/**
 * A typed engine refusal. `identifier` is always an MKT-SWP §17 identifier
 * rendered through the SWAP-8 table; `detail` is public-safe prose for logs,
 * never a substitute for the identifier and never a counterparty's string.
 */
export class SwapEngineError extends Schema.TaggedErrorClass<SwapEngineError>()(
  "MktSwp.SwapEngineError",
  {
    identifier: SwpErrorIdentifierSchema,
    detail: Schema.String,
  },
) {}

/**
 * The engine surface. `verifySession` is SWAP-3's `FundVerifier` method, so
 * a `SwapEngine` is a `FundVerifier` and the compare surface's checklist and
 * `fundingGate` consume this engine directly with no adapter.
 */
export interface Interface extends FundVerifier {
  readonly describe: () => Effect.Effect<EngineDescription>;
  readonly validateProfileRecord: (
    record: SignedRecordJson,
  ) => Effect.Effect<ProfileRecordVerdict, SwapEngineError>;
  /** Load one session's opaque records; every later call addresses it by id. */
  readonly openSession: (records: SessionRecords) => Effect.Effect<void, SwapEngineError>;
  readonly buildExitPackage: (
    sessionId: string,
  ) => Effect.Effect<ExitPackageDescriptor, SwapEngineError>;
  readonly authorizeFunding: (
    request: AuthorizeFundingRequest,
  ) => Effect.Effect<FundingAuthorization, SwapEngineError>;
  readonly constructFundingTransaction: (
    authorization: FundingAuthorization,
  ) => Effect.Effect<TransactionTemplate, SwapEngineError>;
}

export class Service extends Context.Service<Service, Interface>()(
  "@openagentsinc/mkt-swp/SwapEngine",
) {}

/** True only when the engine's verdict and every row agree on a pass. */
export const reportPassed = (report: VerifyBeforeFundReport): boolean =>
  report.verdict === "verification_passed" && report.rows.every((row) => row.status === "pass");

export * as SwapEngine from "./swap-engine.js";
