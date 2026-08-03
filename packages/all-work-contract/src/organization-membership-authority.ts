import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { Context, Effect, Layer, Schema as S } from "effect";

import {
  decodeOrganizationMembershipLedger,
  decodeOrganizationMembershipReadRequest,
  OrganizationMembershipLedgerSchema,
  OrganizationMembershipReadResultSchema,
  type OrganizationMembership,
  type OrganizationMembershipLedger,
  type OrganizationMembershipReadResult,
} from "./generated.ts";

export const ORGANIZATION_MEMBERSHIP_AUTHORITY_STATE_SCHEMA =
  "openagents.organization_membership_authority_state.v1" as const;

export const OrganizationMembershipAuthorityStateSchema = S.Struct({
  schema: S.Literal(ORGANIZATION_MEMBERSHIP_AUTHORITY_STATE_SCHEMA),
  ledger: OrganizationMembershipLedgerSchema,
});
export interface OrganizationMembershipAuthorityState extends S.Schema.Type<
  typeof OrganizationMembershipAuthorityStateSchema
> {}

export class OrganizationMembershipAuthorityError extends S.TaggedErrorClass<OrganizationMembershipAuthorityError>()(
  "OrganizationMembershipAuthority.Error",
  {
    reason: S.Literals([
      "invalid_state",
      "invalid_request",
      "storage_unavailable",
      "revision_conflict",
    ]),
    detail: S.String,
  },
) {}

export interface OrganizationMembershipStateStoreShape {
  readonly load: Effect.Effect<
    OrganizationMembershipAuthorityState | null,
    OrganizationMembershipAuthorityError
  >;
}

export class OrganizationMembershipStateStore extends Context.Service<
  OrganizationMembershipStateStore,
  OrganizationMembershipStateStoreShape
>()("OrganizationMembershipAuthority.StateStore") {}

const decodeState = (input: unknown) =>
  S.decodeUnknownEffect(OrganizationMembershipAuthorityStateSchema)(input, {
    onExcessProperty: "error",
  }).pipe(
    Effect.mapError(
      () =>
        new OrganizationMembershipAuthorityError({
          reason: "invalid_state",
          detail: "decode",
        }),
    ),
  );

const uniqueMemberships = (
  memberships: ReadonlyArray<OrganizationMembership>,
): ReadonlyArray<OrganizationMembership> => {
  const identities = new Set<string>();
  for (const membership of memberships) {
    const identity = `${membership.membershipRef}\u0000${membership.accountRef}\u0000${membership.organizationRef}`;
    if (identities.has(identity)) {
      throw new OrganizationMembershipAuthorityError({
        reason: "invalid_state",
        detail: "duplicate_membership",
      });
    }
    identities.add(identity);
  }
  return [...memberships].sort((left, right) =>
    left.membershipRef.localeCompare(right.membershipRef),
  );
};

export const makeOrganizationMembershipAuthorityState = (input: {
  readonly revision: number;
  readonly observedAt: string;
  readonly memberships: ReadonlyArray<OrganizationMembership>;
}): OrganizationMembershipAuthorityState => ({
  schema: ORGANIZATION_MEMBERSHIP_AUTHORITY_STATE_SCHEMA,
  ledger: decodeOrganizationMembershipLedger({
    contractVersion: "openagents.all_work_boundary.v1",
    revision: input.revision,
    memberships: uniqueMemberships(input.memberships),
    completeness: {
      state: "complete",
      cursor: `cursor:organization-membership:${input.revision}`,
      gapRefs: [],
    },
    freshness: { state: "fresh", observedAt: input.observedAt },
  }),
});

export const emptyOrganizationMembershipAuthorityState = (
  observedAt: string,
): OrganizationMembershipAuthorityState =>
  makeOrganizationMembershipAuthorityState({
    revision: 0,
    observedAt,
    memberships: [],
  });

export interface OrganizationMembershipAuthorityShape {
  readonly read: (
    input: unknown,
  ) => Effect.Effect<OrganizationMembershipReadResult, OrganizationMembershipAuthorityError>;
}

export class OrganizationMembershipAuthority extends Context.Service<
  OrganizationMembershipAuthority,
  OrganizationMembershipAuthorityShape
>()("OrganizationMembershipAuthority.Service") {}

export const OrganizationMembershipAuthorityLive = Layer.effect(
  OrganizationMembershipAuthority,
  Effect.gen(function* () {
    const store = yield* OrganizationMembershipStateStore;
    return OrganizationMembershipAuthority.of({
      read: Effect.fn("OrganizationMembershipAuthority.read")(function* (input: unknown) {
        const request = yield* Effect.try({
          try: () => decodeOrganizationMembershipReadRequest(input),
          catch: () =>
            new OrganizationMembershipAuthorityError({
              reason: "invalid_request",
              detail: "decode",
            }),
        });
        const state = yield* store.load.pipe(
          Effect.flatMap((value) =>
            value === null
              ? Effect.fail(
                  new OrganizationMembershipAuthorityError({
                    reason: "storage_unavailable",
                    detail: "not_initialized",
                  }),
                )
              : Effect.succeed(value),
          ),
        );
        return yield* S.decodeUnknownEffect(OrganizationMembershipReadResultSchema)({
          ledger: {
            ...state.ledger,
            memberships: state.ledger.memberships.filter(
              (membership) =>
                membership.accountRef === request.accountRef &&
                membership.accountGeneration === request.accountGeneration &&
                membership.effectivePrincipalRef === request.effectivePrincipalRef,
            ),
          },
        }).pipe(
          Effect.mapError(
            () =>
              new OrganizationMembershipAuthorityError({
                reason: "invalid_state",
                detail: "read",
              }),
          ),
        );
      }),
    });
  }),
);

export const organizationMembershipStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "organization-memberships.v1.json");

const storageError = (detail: string) =>
  new OrganizationMembershipAuthorityError({
    reason: "storage_unavailable",
    detail,
  });
const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";

const loadFile = (
  filePath: string,
): Effect.Effect<
  OrganizationMembershipAuthorityState | null,
  OrganizationMembershipAuthorityError
> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (error) => (isNotFound(error) ? storageError("not_found") : storageError("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({
        try: () => JSON.parse(contents),
        catch: () => storageError("json"),
      }),
    ),
    Effect.flatMap(decodeState),
    Effect.catch((error) =>
      error.detail === "not_found" ? Effect.succeed(null) : Effect.fail(error),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: OrganizationMembershipAuthorityState,
): Effect.Effect<void, OrganizationMembershipAuthorityError> =>
  Effect.tryPromise({
    try: async () => {
      const directory = path.dirname(filePath);
      await mkdir(directory, { recursive: true, mode: 0o700 });
      const temporary = path.join(
        directory,
        `.${path.basename(filePath)}.${randomBytes(8).toString("hex")}.tmp`,
      );
      await writeFile(temporary, `${JSON.stringify(state, null, 2)}\n`, {
        encoding: "utf8",
        mode: 0o600,
      });
      await rename(temporary, filePath);
    },
    catch: () => storageError("write"),
  });

export const fileOrganizationMembershipStateStoreLayer = (rootDir: string) =>
  Layer.succeed(OrganizationMembershipStateStore, {
    load: loadFile(organizationMembershipStatePath(rootDir)),
  });

export const initializeFileOrganizationMembershipState = (
  rootDir: string,
  state: OrganizationMembershipAuthorityState,
): Effect.Effect<void, OrganizationMembershipAuthorityError> => {
  const filePath = organizationMembershipStatePath(rootDir);
  return loadFile(filePath).pipe(
    Effect.flatMap((existing) => (existing === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};

export const provisionFileOrganizationMembershipState = (
  rootDir: string,
  expectedRevision: number,
  next: OrganizationMembershipAuthorityState,
): Effect.Effect<void, OrganizationMembershipAuthorityError> => {
  const filePath = organizationMembershipStatePath(rootDir);
  return loadFile(filePath).pipe(
    Effect.flatMap((current) =>
      current === null ||
      current.ledger.revision !== expectedRevision ||
      next.ledger.revision !== expectedRevision + 1
        ? Effect.fail(
            new OrganizationMembershipAuthorityError({
              reason: "revision_conflict",
              detail: "expected_revision",
            }),
          )
        : atomicWrite(filePath, next),
    ),
  );
};
