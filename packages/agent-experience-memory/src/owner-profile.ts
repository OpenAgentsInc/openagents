import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import { ownerScopeId, type OwnerScopeId } from "./contract/refs.js";
import { assertRecallClean, guardMemoryText } from "./redaction.js";

/**
 * OWNER-PROFILE (#9107): a durable, owner-scoped profile the on-device
 * assistant can recall across sessions — the owner's name/handle, role,
 * current projects, and stated preferences — so the assistant genuinely
 * "knows the user" over time, not just this turn.
 *
 * This is the SECOND memory layer, distinct from the AFS-10 experience memory
 * in this same package. The ambient context block ships deterministic facts
 * (date, OS, working directory, sovereign identity npub) for the current turn;
 * AFS-10 recalls learned patterns; this layer holds explicit, owner-STATED
 * durable facts. It reuses the AFS-10 ATIF redaction boundary and the same
 * owner-scope + default-off + inspect/forget discipline.
 *
 * Guarantees, identical in spirit to AFS-10:
 * - Default-OFF and byte-identical when off: with the flag off, no fact is read
 *   or written and no profile block enters any prompt.
 * - Redacted: every stored fact and every projected slice passes
 *   `guardMemoryText`. A hard-unsafe value (secret / wallet or payment
 *   material / local path) is REJECTED and never stored, even scrubbed. Soft
 *   PII (for example an email) is scrubbed but the redacted fact is kept.
 * - Owner-scoped: keyed by the owner scope (the sovereign identity npub as the
 *   owner key). One owner scope never reads another owner's profile.
 * - Honest: the projected block states only stored facts. An empty profile
 *   says so; it never invents facts.
 */

export const OWNER_PROFILE_FORMAT_VERSION = "0.1" as const;

/** The declared owner-fact kinds. Bounded so the profile stays a small,
 * legible set of owner-stated facts, not an open memory dump. */
export const OwnerProfileFactCategorySchema = S.Literals([
  "name",
  "handle",
  "role",
  "project",
  "preference",
]);
export type OwnerProfileFactCategory = typeof OwnerProfileFactCategorySchema.Type;

export const OwnerProfileFactSchema = S.Struct({
  category: OwnerProfileFactCategorySchema,
  /** Already redacted at store time. Never carries hard-unsafe material. */
  value: S.String.check(S.isMinLength(1), S.isMaxLength(400)),
  statedAt: S.String,
});
export type OwnerProfileFact = typeof OwnerProfileFactSchema.Type;

/** The profile is owner-level: the facts are cross-project owner facts. */
export type OwnerProfileScope = Readonly<{ owner: OwnerScopeId }>;

/** Construct an owner-profile scope from the owner's sovereign identity npub. */
export const ownerProfileScopeForNpub = (npub: string): OwnerProfileScope => ({
  owner: ownerScopeId(`owner:${npub}`),
});

export type OwnerFactGuardResult =
  | Readonly<{ ok: true; fact: OwnerProfileFact }>
  | Readonly<{ ok: false; reason: "hard_unsafe"; categories: ReadonlyArray<string> }>;

/**
 * Guard a candidate owner fact through the shared ATIF boundary. A value that
 * carried hard-unsafe material (secret, wallet/payment, local path) is rejected
 * and never becomes a fact. Soft PII is scrubbed and the redacted fact is kept.
 */
export const guardOwnerFact = (
  category: OwnerProfileFactCategory,
  value: string,
  statedAt: string,
): OwnerFactGuardResult => {
  const verdict = guardMemoryText(value);
  if (!verdict.storable) return { ok: false, reason: "hard_unsafe", categories: verdict.categories };
  return { ok: true, fact: { category, value: verdict.redacted, statedAt } };
};

export const OWNER_PROFILE_DEFAULT_BUDGET_CHARS = 600;

const CATEGORY_LABELS: Record<OwnerProfileFactCategory, string> = {
  name: "Name",
  handle: "Handle",
  role: "Role",
  project: "Project",
  preference: "Preference",
};

/**
 * Project a compact, cited owner-profile block for the prompt. Default-OFF:
 * with `enabled` false (the default) it returns the empty string, so the
 * caller's prompt is unchanged. Honest: an enabled-but-empty profile renders an
 * explicit "no stored facts" line and never invents. Bounded by `budgetChars`.
 */
export const projectOwnerProfileBlock = (
  facts: ReadonlyArray<OwnerProfileFact>,
  options?: Readonly<{ enabled?: boolean; budgetChars?: number }>,
): string => {
  if (!(options?.enabled ?? false)) return "";
  const header = "## Owner profile (owner-stated, local, redacted)";
  if (facts.length === 0) {
    return `${header}\n\nNo stored owner profile facts yet. State only what is stored; do not invent owner facts.`;
  }
  const budget = options?.budgetChars ?? OWNER_PROFILE_DEFAULT_BUDGET_CHARS;
  const lines = facts.map(
    (fact) => `- ${CATEGORY_LABELS[fact.category]}: ${assertRecallClean(fact.value)}`,
  );
  const full = `${header}\n\n${lines.join("\n")}`;
  if (full.length <= budget) return full;
  const kept: string[] = [];
  let used = header.length + 2;
  for (const line of lines) {
    if (used + line.length + 1 > budget) break;
    kept.push(line);
    used += line.length + 1;
  }
  return `${header}\n\n${kept.join("\n")}\n- (profile truncated to budget)`;
};

/**
 * Inject the owner-profile block into a base prompt. Default-OFF is a
 * byte-identical no-op: an empty projected block returns the base prompt
 * unchanged (proven by test).
 */
export const applyOwnerProfile = (
  basePrompt: string,
  facts: ReadonlyArray<OwnerProfileFact>,
  options?: Readonly<{ enabled?: boolean; budgetChars?: number }>,
): string => {
  const block = projectOwnerProfileBlock(facts, options);
  return block === "" ? basePrompt : `${basePrompt}\n\n${block}`;
};

export class OwnerProfileStoreError extends S.TaggedErrorClass<OwnerProfileStoreError>()(
  "agent-experience-memory/OwnerProfileStoreError",
  { reason: S.Literals(["storage_unavailable", "scope_violation", "hard_unsafe"]) },
) {}

export type OwnerProfileStoreInterface = Readonly<{
  /** Whether this adapter reads or writes anything. The disabled adapter is false. */
  enabled: boolean;
  /** Store an owner fact. A hard-unsafe value is rejected and never stored. */
  put: (
    scope: OwnerProfileScope,
    category: OwnerProfileFactCategory,
    value: string,
    statedAt: string,
  ) => Effect.Effect<OwnerProfileFact, OwnerProfileStoreError>;
  /** The owner's view of their stored facts. */
  inspect: (scope: OwnerProfileScope) => Effect.Effect<ReadonlyArray<OwnerProfileFact>, OwnerProfileStoreError>;
  /** Forget one category's facts, or all facts when `category` is omitted. */
  forget: (
    scope: OwnerProfileScope,
    category?: OwnerProfileFactCategory,
  ) => Effect.Effect<number, OwnerProfileStoreError>;
  reads: Effect.Effect<number>;
  writes: Effect.Effect<number>;
}>;

export class OwnerProfileStore extends Context.Service<OwnerProfileStore, OwnerProfileStoreInterface>()(
  "agent-experience-memory/OwnerProfileStore",
) {}

type StoredFact = OwnerProfileFact & { owner: string };

/** The in-memory adapter the tests run against. Owner-scoped and counted. */
export const inMemoryOwnerProfileStoreLayer = Layer.effect(
  OwnerProfileStore,
  Effect.gen(function* () {
    const facts = yield* Ref.make<ReadonlyArray<StoredFact>>([]);
    const readCount = yield* Ref.make(0);
    const writeCount = yield* Ref.make(0);
    const bumpRead = Ref.update(readCount, (n) => n + 1);
    const bumpWrite = Ref.update(writeCount, (n) => n + 1);
    const inScope = (fact: StoredFact, scope: OwnerProfileScope): boolean => fact.owner === scope.owner;

    return OwnerProfileStore.of({
      enabled: true,
      put: (scope, category, value, statedAt) => {
        const guard = guardOwnerFact(category, value, statedAt);
        if (!guard.ok) {
          return Effect.fail(new OwnerProfileStoreError({ reason: "hard_unsafe" }));
        }
        return bumpWrite.pipe(
          Effect.flatMap(() =>
            Ref.update(facts, (all) => [...all, { ...guard.fact, owner: scope.owner }]),
          ),
          Effect.as(guard.fact),
        );
      },
      inspect: (scope) =>
        bumpRead.pipe(
          Effect.flatMap(() => Ref.get(facts)),
          Effect.map((all) =>
            all.filter((fact) => inScope(fact, scope)).map(({ owner: _owner, ...fact }) => fact),
          ),
        ),
      forget: (scope, category) =>
        bumpWrite.pipe(
          Effect.flatMap(() => Ref.get(facts)),
          Effect.flatMap((all) => {
            const kept = all.filter(
              (fact) => !(inScope(fact, scope) && (category === undefined || fact.category === category)),
            );
            return Ref.set(facts, kept).pipe(Effect.as(all.length - kept.length));
          }),
        ),
      reads: Ref.get(readCount),
      writes: Ref.get(writeCount),
    });
  }),
);

/**
 * The disabled adapter. Every operation is a true no-op: nothing is stored,
 * inspect is empty, and the counters stay at zero. This is the default posture
 * and it proves profile-off touches nothing.
 */
export const disabledOwnerProfileStoreLayer = Layer.sync(OwnerProfileStore, () =>
  OwnerProfileStore.of({
    enabled: false,
    put: () => Effect.fail(new OwnerProfileStoreError({ reason: "storage_unavailable" })),
    inspect: () => Effect.succeed([]),
    forget: () => Effect.succeed(0),
    reads: Effect.succeed(0),
    writes: Effect.succeed(0),
  }),
);
