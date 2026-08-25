/**
 * The `openagents provider` command family.
 *
 * This is the earning half of provider mode, revived at the one seam that can
 * be proven offline: the settlement decision. `provider settle` reads a lease
 * and, when one exists, the NIP-LBR closeout receipt that covers it, and says
 * what the job earned. Unverified work earns zero and says which gate stopped
 * it.
 *
 * What is deliberately not here: presence, the claim and lease transport, and
 * any payout. Presence needs a market transport the do-not-build register keeps
 * deferred (an open market lane later uses the relay/provider-daemon/skeptical-
 * client shape, not a NIP-90/DVM revival), and payout stays on the MDK/Nexus
 * bridge. So `settle` takes the receipt as a file at that seam rather than
 * pretending to fetch it, in the same spirit as `trace upload` naming the
 * server half it is waiting for.
 *
 * The family is defined through a factory taking the root command, so the
 * registration hunk in `cli.ts` stays a single import and a single list entry.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { Effect, Option } from "effect";
import { Command, Flag } from "effect/unstable/cli";

import { InputError } from "./errors.js";
import { Output, type OutputMode } from "./output.js";
import {
  settleLease,
  type LaborCloseoutReceipt,
  type ProviderLease,
  type SettlementDecision,
} from "./provider-settlement.js";

/** The shared flags a provider handler reads back off the root command. */
interface SharedFlags {
  readonly json: boolean;
}

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

const readJsonFile = (path: string, label: string) =>
  Effect.try({
    try: () => JSON.parse(readFileSync(resolve(path), "utf8")) as unknown,
    catch: () =>
      new InputError({ message: `The ${label} file at ${path} could not be read as JSON.` }),
  });

const asRecord = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const text = (value: unknown): string => (typeof value === "string" ? value : "");
const count = (value: unknown): number => (typeof value === "number" ? value : Number.NaN);

/**
 * Read a lease document.
 *
 * The four fields the gate needs are required; a lease missing one is a typo
 * the reader would rather hear about now than as a mysterious refusal.
 */
const decodeLease = (value: unknown, path: string) =>
  Effect.gen(function* () {
    const record = asRecord(value);
    if (record === undefined) {
      return yield* new InputError({ message: `The lease at ${path} is not a JSON object.` });
    }
    const missing = ["job_id", "lane", "provider", "expires_at"].filter((field) =>
      typeof record[field] === "string" ? record[field] === "" : true,
    );
    if (missing.length > 0) {
      return yield* new InputError({
        message: `The lease at ${path} is missing ${missing.join(", ")}.`,
      });
    }
    const lease: ProviderLease = {
      job_id: text(record["job_id"]),
      lane: text(record["lane"]),
      provider: text(record["provider"]),
      price_msats: count(record["price_msats"]),
      expires_at: text(record["expires_at"]),
    };
    return lease;
  });

/**
 * Read a closeout receipt.
 *
 * Absent or malformed fields become empty strings rather than an error: the
 * settlement gate already has a named refusal for each of them, and a receipt
 * that is missing its verification refs should be refused as unverified work,
 * not as a bad file.
 */
const decodeCloseout = (value: unknown, path: string) =>
  Effect.gen(function* () {
    const record = asRecord(value);
    if (record === undefined) {
      return yield* new InputError({ message: `The closeout at ${path} is not a JSON object.` });
    }
    const closeout: LaborCloseoutReceipt = {
      receiptRef: text(record["receiptRef"]),
      requestId: text(record["requestId"]),
      requesterPubkey: text(record["requesterPubkey"]),
      providerPubkey: text(record["providerPubkey"]),
      quotedAmountMsats: count(record["quotedAmountMsats"]),
      verificationCommandRef: text(record["verificationCommandRef"]),
      testRef: text(record["testRef"]),
      platformCloseoutRef: text(record["platformCloseoutRef"]),
      digest: text(record["digest"]),
      settled_at: text(record["settled_at"]),
    };
    return closeout;
  });

const decisionHuman = (decision: SettlementDecision): ReadonlyArray<string> => [
  `Job: ${decision.job_id}`,
  `Outcome: ${decision.state}`,
  `Earned: ${decision.earned_msats} msats`,
  ...(decision.refusal === undefined ? [] : [`Refused: ${decision.refusal}`]),
  ...(decision.receipt_ref === undefined ? [] : [`Receipt: ${decision.receipt_ref}`]),
  decision.reason,
  "Accrual only: this command holds no key, connects no payout rail, and moves nothing.",
];

export const makeProviderCommand = <R>(root: Effect.Effect<SharedFlags, never, R>) => {
  const leaseFlag = Flag.string("lease").pipe(
    Flag.withDescription("Path to the lease document the buyer granted for this job"),
  );
  const closeoutFlag = Flag.string("closeout").pipe(
    Flag.optional,
    Flag.withDescription(
      "Path to the NIP-LBR closeout receipt covering this job. Omit it to see what an unverified job earns.",
    ),
  );

  const settleCommand = Command.make(
    "settle",
    { lease: leaseFlag, closeout: closeoutFlag },
    ({ closeout: closeoutPath, lease: leasePath }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const output = yield* Output;

        const lease = yield* decodeLease(yield* readJsonFile(leasePath, "lease"), leasePath);
        const closeout = Option.isSome(closeoutPath)
          ? yield* decodeCloseout(
              yield* readJsonFile(closeoutPath.value, "closeout"),
              closeoutPath.value,
            )
          : undefined;

        const decision = settleLease(lease, closeout);
        yield* output.write(
          { value: decision, human: decisionHuman(decision) },
          outputMode(flags.json),
        );
      }),
  ).pipe(
    Command.withDescription(
      "Decide what one leased job earned. Payment follows a NIP-LBR closeout receipt that names a verification command, its evidence, and the platform's own closeout; a lease, a submission, or time spent online earns nothing. The decision accrues and never pays: no key is held and no payout rail is connected.",
    ),
  );

  return Command.make("provider").pipe(
    Command.withDescription(
      "Earn on verified work. The settlement gate is live; presence, the claim and lease transport, and payout are not wired, so this decides what work is owed rather than moving anything.",
    ),
    Command.withSubcommands([settleCommand]),
  );
};
