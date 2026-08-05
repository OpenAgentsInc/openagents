/**
 * A deterministic in-memory engine behind the SWAP-0 boundary, used until
 * the wasm binding of the Immortal client crate is wired through
 * `engine-binding.ts`, and by every boundary test after that.
 *
 * It exists so hosts and tests can drive a session through order →
 * verification → funding-authorised without the host ever computing a
 * profile-level verdict, and so the funding gate
 * (`swp_funding_not_authorized`) is exercised end to end. It performs no
 * cryptography beyond hashing and authorises nothing outside its fixtures.
 */
import { Effect, Layer } from "effect";
import { FundVerifierUnavailable, VERIFY_CHECK_IDS } from "@openagentsinc/mkt-swp-compare";
import type {
  VerifyBeforeFundReport,
  VerifyCheckId,
  VerifyCheckRow,
} from "@openagentsinc/mkt-swp-compare";
import {
  ENGINE_CONTRACT_VERSION,
  EngineDescriptionSchema,
  FundingAuthorizationSchema,
  MKT_SWP_PROFILE,
  MKT_SWP_PROFILE_VERSION,
  ProfileRecordVerdictSchema,
  Service,
  SwapEngineError,
  SessionRecordsSchema,
  TransactionTemplateSchema,
  reportPassed,
} from "./swap-engine.js";
import type {
  AuthorizeFundingRequest,
  FundingAuthorization,
  SessionRecords,
} from "./swap-engine.js";

export interface FixtureEngineOptions {
  /** Force one verify-before-fund row to fail. */
  readonly failingCheck?: VerifyCheckId;
  /** Force one verify-before-fund row to stay unresolved. */
  readonly unresolvedCheck?: VerifyCheckId;
  /** Force the engine's own verdict to block while every row reads pass. */
  readonly blockVerdict?: boolean;
}

const sha256Hex = (input: string): Effect.Effect<string> =>
  Effect.promise(async () => {
    const digest = await globalThis.crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(input),
    );
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join(
      "",
    );
  });

const contractDigestFor = (records: SessionRecords): Effect.Effect<string> =>
  sha256Hex([records.quote, records.order, ...records.swapContracts].join("\n"));

/**
 * A public-safe fixture session: opaque signed-record stand-ins with
 * obviously synthetic signatures. Regtest-shaped, no real key material, and
 * nothing here is a secret under MKT-SWP §14's forbidden-material list.
 */
export const fixtureSwapSession: SessionRecords = SessionRecordsSchema.make({
  sessionId: "1f".repeat(32),
  quoteEventId: "2a".repeat(32),
  orderEventId: "4c".repeat(32),
  epoch: 1,
  quote: JSON.stringify({
    kind: 39605,
    id: "2a".repeat(32),
    pubkey: "3b".repeat(32),
    sig: "00".repeat(64),
    content: "<opaque mkt-swp quote payload>",
    tags: [["d", "1f".repeat(32)]],
  }),
  order: JSON.stringify({
    kind: 39606,
    id: "4c".repeat(32),
    pubkey: "5d".repeat(32),
    sig: "00".repeat(64),
    content: "<opaque mkt-swp order payload>",
    tags: [["d", "1f".repeat(32)]],
  }),
  swapContracts: [
    JSON.stringify({
      kind: 39610,
      id: "6e".repeat(32),
      pubkey: "5d".repeat(32),
      sig: "00".repeat(64),
      content: "<opaque requester swap contract>",
      tags: [["d", "1f".repeat(32)]],
    }),
    JSON.stringify({
      kind: 39610,
      id: "7f".repeat(32),
      pubkey: "3b".repeat(32),
      sig: "00".repeat(64),
      content: "<opaque provider swap contract>",
      tags: [["d", "1f".repeat(32)]],
    }),
  ],
});

/** Build the fixture engine as a layer over the SwapEngine service tag. */
export const fixtureEngineLayer = (options: FixtureEngineOptions = {}) =>
  Layer.sync(Service, () => {
    const sessions = new Map<string, SessionRecords>();
    const reports = new Map<string, VerifyBeforeFundReport>();

    const describe = () =>
      Effect.succeed(
        EngineDescriptionSchema.make({
          contractVersion: ENGINE_CONTRACT_VERSION,
          profile: MKT_SWP_PROFILE,
          profileVersion: MKT_SWP_PROFILE_VERSION,
          source: { repository: "OpenAgentsInc/immortal" },
        }),
      );

    const validateProfileRecord = Effect.fn("MktSwpFixtureEngine.validateProfileRecord")(function* (
      record: string,
    ) {
      const parsed = yield* Effect.try({
        try: () => JSON.parse(record) as { readonly kind?: unknown },
        catch: () =>
          new SwapEngineError({
            identifier: "swp_unsupported_profile",
            detail: "record is not canonical JSON",
          }),
      }).pipe(Effect.option);
      if (parsed._tag === "None" || typeof parsed.value.kind !== "number") {
        return ProfileRecordVerdictSchema.cases.Refused.make({
          identifier: "swp_unsupported_profile",
        });
      }
      return ProfileRecordVerdictSchema.cases.Accepted.make({ kind: parsed.value.kind });
    });

    const openSession = Effect.fn("MktSwpFixtureEngine.openSession")(function* (
      records: SessionRecords,
    ) {
      if (records.swapContracts.length !== 2) {
        return yield* new SwapEngineError({
          identifier: "swp_contract_missing",
          detail: "the complementary swap contract pair is required before funding",
        });
      }
      sessions.set(records.sessionId, records);
    });

    const requireSession = Effect.fn("MktSwpFixtureEngine.requireSession")(function* (
      sessionId: string,
    ) {
      const records = sessions.get(sessionId);
      if (records === undefined) {
        return yield* new SwapEngineError({
          identifier: "swp_contract_missing",
          detail: "no session records were opened for this session",
        });
      }
      return records;
    });

    const verifySession = Effect.fn("MktSwpFixtureEngine.verifySession")(function* (request: {
      readonly quoteEventId: string;
      readonly orderEventId: string | null;
      readonly epoch: number;
    }) {
      const entry = [...sessions.values()].find(
        (records) => records.quoteEventId === request.quoteEventId,
      );
      if (entry === undefined) {
        // Unavailability is the only failure channel of SWAP-3's port, so a
        // missing engine session can never masquerade as a pass.
        return yield* new FundVerifierUnavailable({ reason: "engine_failed" });
      }
      const rows = VERIFY_CHECK_IDS.map((id): VerifyCheckRow => {
        if (id === options.failingCheck) {
          return { id, status: "fail", error: "swp_terms_mismatch" };
        }
        if (id === options.unresolvedCheck) return { id, status: "unresolved" };
        return { id, status: "pass" };
      });
      const blocked =
        options.blockVerdict === true ||
        options.failingCheck !== undefined ||
        options.unresolvedCheck !== undefined;
      const report: VerifyBeforeFundReport = {
        quoteEventId: request.quoteEventId,
        orderEventId: request.orderEventId,
        epoch: request.epoch,
        rows,
        verdict: blocked ? "verification_blocked" : "verification_passed",
      };
      reports.set(entry.sessionId, report);
      return report;
    });

    const buildExitPackage = Effect.fn("MktSwpFixtureEngine.buildExitPackage")(function* (
      sessionId: string,
    ) {
      const records = yield* requireSession(sessionId);
      const contractDigest = yield* contractDigestFor(records);
      const packageDigest = yield* sha256Hex(`exit-package\n${contractDigest}`);
      return {
        sessionId,
        contractDigest,
        packageDigest,
        storageRef: `local:exit-package:${sessionId}`,
      };
    });

    const authorizeFunding = Effect.fn("MktSwpFixtureEngine.authorizeFunding")(function* (
      request: AuthorizeFundingRequest,
    ) {
      const records = yield* requireSession(request.sessionId);
      const report = reports.get(request.sessionId);
      if (report === undefined || !reportPassed(report) || report.epoch !== records.epoch) {
        return yield* new SwapEngineError({
          identifier: "swp_funding_not_authorized",
          detail: "verify-before-fund is absent, stale, unresolved, or failed for this session",
        });
      }
      const contractDigest = yield* contractDigestFor(records);
      if (
        request.exitPackage.sessionId !== request.sessionId ||
        request.exitPackage.contractDigest !== contractDigest
      ) {
        return yield* new SwapEngineError({
          identifier: "swp_exit_package_mismatch",
          detail: "exit package digest is not bound to the verified contract pair",
        });
      }
      return FundingAuthorizationSchema.make({
        sessionId: request.sessionId,
        contractDigest,
        exitPackageDigest: request.exitPackage.packageDigest,
        reportEpoch: report.epoch,
        authorizedAtEpochSeconds: 0,
      });
    });

    const constructFundingTransaction = Effect.fn(
      "MktSwpFixtureEngine.constructFundingTransaction",
    )(function* (authorization: FundingAuthorization) {
      const template = yield* sha256Hex(`funding-template\n${authorization.contractDigest}`);
      return TransactionTemplateSchema.make({
        sessionId: authorization.sessionId,
        purpose: "funding",
        template: `fixture-unsigned-tx:${template}`,
      });
    });

    return Service.of({
      describe,
      validateProfileRecord,
      openSession,
      verifySession,
      buildExitPackage,
      authorizeFunding,
      constructFundingTransaction,
    });
  });
