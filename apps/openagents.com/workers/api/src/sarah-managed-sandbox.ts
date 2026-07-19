import type { SyncSql } from "@openagentsinc/khala-sync-server";
import {
  type ManagedSandboxCommand,
  type ManagedSandboxResource,
  ManagedSandboxRuntimeIdentitySchema,
  SandboxBudgetSchema,
  SandboxRef,
  Sha256Digest,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import type { SarahAgentTool, SarahAgentToolResult } from "./sarah-agent-runtime";
import { SarahAgentToolError } from "./sarah-agent-runtime";
import type { ManagedSandboxBroker } from "./managed-sandbox-broker";
import type { BoxV1Policy, BoxV1Principal } from "./managed-sandbox-box-v1-routes";
import {
  type SarahOperationAuthorityInput,
  type SarahOperationAuthorityOutcome,
  authorizeSarahOperation,
} from "./sarah-owner-routes";

const PROGRAM_REF = "program.managed_agent_sandboxes" as const;
const RESOURCE_REF = "authenticated_owner_openagents_managed_sandboxes" as const;
const REPOSITORY_REF = "OpenAgentsInc/openagents" as const;

const Ref = SandboxRef;
const Program = S.Literal(PROGRAM_REF);
const Repository = S.Literal(REPOSITORY_REF);
const Idempotency = S.String.check(S.isMinLength(8), S.isMaxLength(256));
const Prompt = S.String.check(S.isMinLength(1), S.isMaxLength(100_000));
const CapabilityKinds = S.Array(
  S.Literals(["agent_turn", "command", "file_read", "file_write", "artifact_read"]),
).check(S.isMinLength(1), S.isMaxLength(5));

const TargetBinding = {
  programRef: Program,
  repositoryRef: Repository,
  targetRef: Ref,
  imageDigest: Sha256Digest,
  profileRef: Ref,
};

const ExactResourceScopeSchema = S.Struct({
  ...TargetBinding,
  sandboxRef: Ref,
  workUnitRef: Ref,
  resourceGeneration: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  expectedVersion: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  budget: SandboxBudgetSchema,
  capabilityRefs: S.Array(Ref).check(S.isMinLength(1), S.isMaxLength(16)),
  idempotencyRef: Idempotency,
});

const CreateSchema = S.Struct({
  ...TargetBinding,
  workUnitRef: Ref,
  attachmentRef: Ref,
  attachmentGeneration: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  ttlSeconds: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(60),
    S.isLessThanOrEqualTo(86_400),
  ),
  budget: SandboxBudgetSchema,
  capabilityKinds: CapabilityKinds,
  idempotencyRef: Idempotency,
});

const ListSchema = S.Struct({
  ...TargetBinding,
  idempotencyRef: Idempotency,
});

const DispatchSchema = S.Struct({
  ...ExactResourceScopeSchema.fields,
  turnRef: Ref,
  capabilityRef: Ref,
  prompt: Prompt,
  runtime: ManagedSandboxRuntimeIdentitySchema,
});

const InterruptSchema = S.Struct({
  ...ExactResourceScopeSchema.fields,
  turnRef: Ref,
  reasonRef: Ref,
});

const ActionSchema = S.Struct({
  ...ExactResourceScopeSchema.fields,
  reasonRef: S.optionalKey(Ref),
});

type Authorize = (
  sql: SyncSql,
  input: SarahOperationAuthorityInput,
) => Effect.Effect<SarahOperationAuthorityOutcome, unknown>;

export type SarahManagedSandboxDependencies = Readonly<{
  sql: SyncSql;
  ownerUserId: string;
  threadRef: string;
  turnId: string;
  principal: BoxV1Principal;
  policy: BoxV1Policy;
  broker: ManagedSandboxBroker;
  runtimeAdmitted: boolean;
  authorizeOperation?: Authorize | undefined;
  now?: (() => Date) | undefined;
}>;

const failure = (reason: string) => new SarahAgentToolError({ reason });

const safeSegment = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:-]/gu, "_").slice(0, 120);

const digest = (value: string): Effect.Effect<string, SarahAgentToolError> =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
      return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    },
    catch: () => failure("managed_sandbox_identity_unavailable"),
  });

const decode = <A>(schema: S.Decoder<A>, value: unknown) =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value, { onExcessProperty: "error" }),
    catch: () => failure("invalid_tool_arguments"),
  });

const json = (value: unknown): string => JSON.stringify(value);

const exactBudget = (
  left: ManagedSandboxResource["budget"],
  right: ManagedSandboxResource["budget"],
): boolean =>
  left.currency === right.currency &&
  left.maxCostMicros === right.maxCostMicros &&
  left.maxCpuMillis === right.maxCpuMillis &&
  left.maxNetworkBytes === right.maxNetworkBytes &&
  left.maxArtifactBytes === right.maxArtifactBytes &&
  left.maxLifetimeSeconds === right.maxLifetimeSeconds;

const admittedBinding = (
  policy: BoxV1Policy,
  value: Readonly<{
    programRef: string;
    repositoryRef: string;
    targetRef: string;
    imageDigest: string;
    profileRef: string;
    budget?: ManagedSandboxResource["budget"] | undefined;
    ttlSeconds?: number | undefined;
  }>,
) => ({
  scope:
    value.programRef === PROGRAM_REF &&
    value.repositoryRef === REPOSITORY_REF &&
    value.targetRef === policy.target.targetRef &&
    value.imageDigest === policy.imageDigest &&
    value.profileRef === policy.profileRef,
  budget:
    value.budget === undefined ||
    (value.budget.currency === "USD" &&
      value.budget.maxCostMicros <= policy.maxCostMicros &&
      value.budget.maxCpuMillis <= policy.maxCpuMillis &&
      value.budget.maxNetworkBytes <= policy.maxNetworkBytes &&
      value.budget.maxArtifactBytes <= policy.maxArtifactBytes &&
      value.budget.maxLifetimeSeconds <= policy.maxTtlSeconds &&
      (value.ttlSeconds === undefined || value.budget.maxLifetimeSeconds <= value.ttlSeconds)),
});

const exactScope = (
  resource: ManagedSandboxResource,
  scope: typeof ExactResourceScopeSchema.Type,
  ownerRef: string,
): boolean =>
  resource.ownerRef === ownerRef &&
  resource.programRef === scope.programRef &&
  resource.sandboxRef === scope.sandboxRef &&
  resource.workUnitRef === scope.workUnitRef &&
  resource.target.targetRef === scope.targetRef &&
  resource.imageDigest === scope.imageDigest &&
  resource.profileRef === scope.profileRef &&
  resource.resourceGeneration === scope.resourceGeneration &&
  resource.version >= scope.expectedVersion &&
  exactBudget(resource.budget, scope.budget) &&
  scope.capabilityRefs.every((ref) =>
    resource.capabilities.some((capability) => capability.capabilityRef === ref),
  );

const parameters = (
  policy: BoxV1Policy,
  properties: Readonly<Record<string, unknown>>,
  required: ReadonlyArray<string>,
) => ({
  additionalProperties: false,
  properties: {
    programRef: { const: PROGRAM_REF, type: "string" },
    repositoryRef: { const: REPOSITORY_REF, type: "string" },
    targetRef: { const: policy.target.targetRef, type: "string" },
    imageDigest: { const: policy.imageDigest, type: "string" },
    profileRef: { const: policy.profileRef, type: "string" },
    ...properties,
  },
  required: ["programRef", "repositoryRef", "targetRef", "imageDigest", "profileRef", ...required],
  type: "object",
});

const scopeProperties = {
  sandboxRef: { type: "string" },
  workUnitRef: { type: "string" },
  resourceGeneration: { minimum: 1, type: "integer" },
  expectedVersion: { minimum: 0, type: "integer" },
  budget: {
    additionalProperties: false,
    properties: {
      currency: { const: "USD", type: "string" },
      maxCostMicros: { minimum: 0, type: "integer" },
      maxCpuMillis: { minimum: 0, type: "integer" },
      maxNetworkBytes: { minimum: 0, type: "integer" },
      maxArtifactBytes: { minimum: 0, type: "integer" },
      maxLifetimeSeconds: { minimum: 1, type: "integer" },
    },
    required: [
      "currency",
      "maxCostMicros",
      "maxCpuMillis",
      "maxNetworkBytes",
      "maxArtifactBytes",
      "maxLifetimeSeconds",
    ],
    type: "object",
  },
  capabilityRefs: { items: { type: "string" }, minItems: 1, type: "array" },
  idempotencyRef: { minLength: 8, type: "string" },
} as const;

const scopeRequired = [
  "sandboxRef",
  "workUnitRef",
  "resourceGeneration",
  "expectedVersion",
  "budget",
  "capabilityRefs",
  "idempotencyRef",
];

const summaryFor = (action: string, resource: ManagedSandboxResource): string => {
  if (resource.facts.lifecycle === "recovery_required")
    return `${action} requires recovery; cleanup is not complete.`;
  if (resource.facts.lifecycle === "failed") return `${action} failed on the managed target.`;
  if (resource.facts.lifecycle === "deleted" && resource.facts.cleanupComplete)
    return `${action} completed with cleanup confirmed.`;
  if (["provisioning", "stopping", "resuming", "deleting"].includes(resource.facts.lifecycle))
    return `${action} was accepted and remains ${resource.facts.lifecycle}.`;
  return `${action} observed ${resource.facts.lifecycle}.`;
};

export const makeSarahManagedSandboxTools = (
  deps: SarahManagedSandboxDependencies,
): ReadonlyArray<SarahAgentTool> => {
  const authorize = deps.authorizeOperation ?? authorizeSarahOperation;
  const now = deps.now ?? (() => new Date());

  const authority = (
    action: string,
    toolCallId: string,
    evidenceRefs: ReadonlyArray<string>,
    gates: Readonly<{ scope: boolean; budget: boolean }>,
  ) =>
    authorize(deps.sql, {
      action,
      ownerUserId: deps.ownerUserId,
      threadRef: deps.threadRef,
      resource: RESOURCE_REF,
      programRef: PROGRAM_REF,
      triggerRef: `turn.${safeSegment(deps.turnId)}.tool.${safeSegment(toolCallId)}`,
      targetEvidenceRefs: evidenceRefs,
      conditionResults: [
        {
          conditionRef: "condition.owner_scope",
          passed: true,
          evidenceRefs: [`thread:${deps.threadRef}`],
        },
        {
          conditionRef: "condition.managed_sandbox_scope",
          passed: gates.scope,
          evidenceRefs: gates.scope ? evidenceRefs : [],
        },
        {
          conditionRef: "condition.managed_sandbox_budget",
          passed: gates.budget,
          evidenceRefs: gates.budget ? ["gate:bounded_lease_budget"] : [],
        },
        {
          conditionRef: "condition.managed_sandbox_runtime_admission",
          passed: deps.runtimeAdmitted,
          evidenceRefs: deps.runtimeAdmitted
            ? [
                `target:${deps.policy.target.targetRef}`,
                `image:${deps.policy.imageDigest}`,
                `profile:${deps.policy.profileRef}`,
              ]
            : [],
        },
        {
          conditionRef: "condition.redaction",
          passed: true,
          evidenceRefs: ["schema:openagents.sarah.managed_sandbox_tool.v1"],
        },
        {
          conditionRef: "condition.rollback",
          passed: true,
          evidenceRefs: ["gate:managed_sandbox_delete_and_reconcile"],
        },
      ],
    }).pipe(
      Effect.mapError((error) =>
        failure(error instanceof Error ? error.message : "managed_sandbox_authority_unavailable"),
      ),
    );

  const refused = (decision: SarahOperationAuthorityOutcome): SarahAgentToolResult => ({
    authorityReceiptRef: decision.receiptRef,
    authorityAllowed: false,
    content: json({
      ok: false,
      outcome: "refused",
      reason: decision.refusalReason ?? "managed_sandbox_authority_refused",
      activity: [
        {
          order: 1,
          state: "refused",
          summary: "Managed-sandbox authority refused before any target effect.",
          receiptRef: decision.receiptRef,
        },
      ],
    }),
    isError: true,
    resultRefs: [decision.receiptRef],
    summary: "Managed-sandbox authority refused before any target effect.",
  });

  const run = <A>(
    input: Readonly<{
      action: string;
      label: string;
      schema: S.Decoder<A>;
      raw: unknown;
      toolCallId: string;
      evidence: (decoded: A) => ReadonlyArray<string>;
      command: (decoded: A) => Effect.Effect<ManagedSandboxCommand, SarahAgentToolError>;
      prompt?: ((decoded: A) => string | undefined) | undefined;
      attachmentGeneration?: ((decoded: A) => number | undefined) | undefined;
    }>,
  ): Effect.Effect<SarahAgentToolResult, SarahAgentToolError> =>
    Effect.gen(function* () {
      const decoded = yield* decode(input.schema, input.raw);
      const decision = yield* authority(
        input.action,
        input.toolCallId,
        input.evidence(decoded),
        admittedBinding(deps.policy, decoded as never),
      );
      if (!decision.allowed) return refused(decision);
      const command = yield* input.command(decoded);
      const result = yield* deps.broker
        .execute(command, {
          prompt: input.prompt?.(decoded),
          attachmentGeneration: input.attachmentGeneration?.(decoded),
        })
        .pipe(Effect.mapError((error) => failure(error.code)));
      const targetSummary = summaryFor(input.label, result.resource);
      const failed = ["failed", "recovery_required"].includes(result.resource.facts.lifecycle);
      return {
        authorityReceiptRef: decision.receiptRef,
        content: json({
          ok: !failed,
          outcome: result.receipt.outcome,
          resource: result.resource,
          command: result.command,
          targetReceipt: result.receipt,
          turn: result.turn,
          turnReceipt: result.turnReceipt,
          events: result.events,
          activity: [
            {
              order: 1,
              state: "succeeded",
              summary: "Owner-thread authority admitted the exact action.",
              receiptRef: decision.receiptRef,
            },
            {
              order: 2,
              state: result.receipt.outcome,
              summary: targetSummary,
              receiptRef: result.receipt.receiptRef,
            },
          ],
        }),
        isError: failed,
        resultRefs: [
          decision.receiptRef,
          result.receipt.receiptRef,
          result.resource.sandboxRef,
          ...(result.turn === null ? [] : [result.turn.turnRef]),
          ...(result.turnReceipt === null ? [] : [result.turnReceipt.receiptRef]),
        ],
        summary: targetSummary,
      };
    });

  const identity = (action: string, idempotencyRef: string) =>
    digest(
      `${deps.principal.ownerRef}\n${deps.principal.tenantRef}\n${action}\n${idempotencyRef}`,
    ).pipe(
      Effect.map((value) => ({
        commandRef: `command.sarah.sbx.${action}.${value.slice(0, 32)}`,
        idempotencyRef: `idempotency.sarah.sbx.${value.slice(0, 32)}`,
      })),
    );

  const base = (
    action: string,
    idempotencyRef: string,
  ): Effect.Effect<Readonly<Record<string, unknown>>, SarahAgentToolError> =>
    identity(action, idempotencyRef).pipe(
      Effect.map((identityFields) => ({
        schema: "openagents.managed_sandbox_command.v1",
        ...identityFields,
        requestedByRef: "principal.sarah",
        ownerRef: deps.principal.ownerRef,
        tenantRef: deps.principal.tenantRef,
        requestedAt: now().toISOString(),
      })),
    );

  const command =
    <A>(
      action: string,
      make: (decoded: A, baseFields: Readonly<Record<string, unknown>>) => ManagedSandboxCommand,
    ) =>
    (decoded: A): Effect.Effect<ManagedSandboxCommand, SarahAgentToolError> =>
      Effect.gen(function* () {
        const baseFields = yield* base(
          action,
          (decoded as { idempotencyRef: string }).idempotencyRef,
        );
        return yield* Effect.try({
          try: () => make(decoded, baseFields),
          catch: () => failure("managed_sandbox_command_invalid"),
        });
      });

  const commonEvidence = (value: {
    targetRef: string;
    imageDigest: string;
    profileRef: string;
    workUnitRef?: string | undefined;
    sandboxRef?: string | undefined;
  }) => [
    `program:${PROGRAM_REF}`,
    `repository:${REPOSITORY_REF}`,
    `target:${value.targetRef}`,
    `image:${value.imageDigest}`,
    `profile:${value.profileRef}`,
    ...(value.workUnitRef === undefined ? [] : [`work_unit:${value.workUnitRef}`]),
    ...(value.sandboxRef === undefined ? [] : [`sandbox:${value.sandboxRef}`]),
  ];

  const create: SarahAgentTool = {
    definition: {
      name: "managed_sandbox_create",
      description:
        "Create one exact owner-scoped OpenAgents-managed GCP sandbox with a bounded lease, budget, capabilities, and idempotency identity.",
      parameters: parameters(
        deps.policy,
        {
          workUnitRef: { type: "string" },
          attachmentRef: { type: "string" },
          attachmentGeneration: { minimum: 1, type: "integer" },
          ttlSeconds: {
            maximum: deps.policy.maxTtlSeconds,
            minimum: 60,
            type: "integer",
          },
          budget: scopeProperties.budget,
          capabilityKinds: {
            items: {
              enum: ["agent_turn", "command", "file_read", "file_write", "artifact_read"],
              type: "string",
            },
            minItems: 1,
            type: "array",
          },
          idempotencyRef: scopeProperties.idempotencyRef,
        },
        [
          "workUnitRef",
          "attachmentRef",
          "attachmentGeneration",
          "ttlSeconds",
          "budget",
          "capabilityKinds",
          "idempotencyRef",
        ],
      ),
    },
    execute: (raw, toolCall) =>
      run({
        action: "create_managed_sandbox",
        label: "Create",
        schema: CreateSchema,
        raw,
        toolCallId: toolCall.id,
        evidence: commonEvidence,
        command: command("create", (value, baseFields) => {
          const issuedAt = new Date(baseFields.requestedAt as string);
          const expiresAt = new Date(issuedAt.getTime() + value.ttlSeconds * 1_000).toISOString();
          return {
            ...baseFields,
            _tag: "Create",
            workUnitRef: value.workUnitRef,
            attachmentRef: value.attachmentRef,
            target: deps.policy.target,
            imageDigest: value.imageDigest,
            profileRef: value.profileRef,
            lease: {
              leaseRef: `lease.sarah.sbx.${safeSegment(value.idempotencyRef)}`,
              state: "active",
              issuedAt: issuedAt.toISOString(),
              expiresAt,
              ttlSeconds: value.ttlSeconds,
              renewable: true,
            },
            budget: value.budget,
            requestedCapabilities: value.capabilityKinds.map((kind) => ({
              capabilityRef: `capability.sarah.sbx.${safeSegment(value.idempotencyRef)}.${kind}`,
              kind,
              state: "active",
              expiresAt,
            })),
          } as unknown as ManagedSandboxCommand;
        }),
        attachmentGeneration: (value) => value.attachmentGeneration,
      }),
  };

  const list: SarahAgentTool = {
    definition: {
      name: "managed_sandboxes_list",
      description:
        "List only the authenticated owner’s OpenAgents-managed sandboxes on the exact admitted target.",
      parameters: parameters(deps.policy, { idempotencyRef: scopeProperties.idempotencyRef }, [
        "idempotencyRef",
      ]),
    },
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const value = yield* decode(ListSchema, raw);
        const decision = yield* authority(
          "list_managed_sandboxes",
          toolCall.id,
          commonEvidence(value),
          admittedBinding(deps.policy, value),
        );
        if (!decision.allowed) return refused(decision);
        const resources = yield* deps.broker
          .list()
          .pipe(Effect.mapError((error) => failure(error.code)));
        if (
          resources.some(
            (resource) =>
              resource.ownerRef !== deps.principal.ownerRef ||
              resource.tenantRef !== deps.principal.tenantRef ||
              resource.programRef !== PROGRAM_REF ||
              resource.target.targetRef !== deps.policy.target.targetRef ||
              resource.imageDigest !== deps.policy.imageDigest ||
              resource.profileRef !== deps.policy.profileRef,
          )
        ) {
          return yield* failure("managed_sandbox_scope_mismatch");
        }
        const receiptDigest = yield* digest(
          `${deps.principal.ownerRef}\nlist\n${value.idempotencyRef}`,
        );
        const targetReceiptRef = `receipt.sbx.list.${receiptDigest.slice(0, 40)}`;
        return {
          authorityReceiptRef: decision.receiptRef,
          content: json({
            ok: true,
            resources,
            targetReceipt: {
              receiptRef: targetReceiptRef,
              outcome: "succeeded",
              ownerRef: deps.principal.ownerRef,
              targetRef: deps.policy.target.targetRef,
              observedAt: now().toISOString(),
            },
            activity: [
              {
                order: 1,
                state: "succeeded",
                summary: "Owner-thread authority admitted the exact list action.",
                receiptRef: decision.receiptRef,
              },
              {
                order: 2,
                state: "succeeded",
                summary: `Observed ${resources.length} owner-scoped managed sandbox${resources.length === 1 ? "" : "es"}.`,
                receiptRef: targetReceiptRef,
              },
            ],
          }),
          resultRefs: [
            decision.receiptRef,
            targetReceiptRef,
            ...resources.map((resource) => resource.sandboxRef),
          ],
          summary: `Observed ${resources.length} owner-scoped managed sandbox${resources.length === 1 ? "" : "es"}.`,
        };
      }),
  };

  const scopedTool = <A>(
    input: Readonly<{
      name: string;
      description: string;
      action: string;
      label: string;
      schema: S.Decoder<A>;
      extraProperties?: Readonly<Record<string, unknown>> | undefined;
      extraRequired?: ReadonlyArray<string> | undefined;
      make: (value: A, baseFields: Readonly<Record<string, unknown>>) => ManagedSandboxCommand;
      prompt?: ((value: A) => string | undefined) | undefined;
    }>,
  ): SarahAgentTool => ({
    definition: {
      name: input.name,
      description: input.description,
      parameters: parameters(deps.policy, { ...scopeProperties, ...input.extraProperties }, [
        ...scopeRequired,
        ...(input.extraRequired ?? []),
      ]),
    },
    execute: (raw, toolCall) =>
      run({
        action: input.action,
        label: input.label,
        schema: input.schema,
        raw,
        toolCallId: toolCall.id,
        evidence: (value) => commonEvidence(value as never),
        command: command(input.name, input.make),
        prompt: input.prompt,
      }).pipe(
        Effect.flatMap((result) => {
          if (result.authorityAllowed === false) return Effect.succeed(result);
          const payload = JSON.parse(result.content) as {
            resource?: ManagedSandboxResource;
          };
          return payload.resource !== undefined &&
            exactScope(
              payload.resource,
              raw as typeof ExactResourceScopeSchema.Type,
              deps.principal.ownerRef,
            )
            ? Effect.succeed(result)
            : Effect.fail(failure("managed_sandbox_scope_mismatch"));
        }),
      ),
  });

  const inspect = scopedTool({
    name: "managed_sandbox_inspect",
    description:
      "Inspect one exact owner sandbox and follow its latest structural runtime activity without treating silence as completion.",
    action: "inspect_managed_sandbox",
    label: "Inspect",
    schema: ExactResourceScopeSchema,
    make: (value, baseFields) =>
      ({
        ...baseFields,
        _tag: "Inspect",
        sandboxRef: value.sandboxRef,
      }) as ManagedSandboxCommand,
  });

  const dispatch = scopedTool({
    name: "managed_sandbox_dispatch",
    description:
      "Dispatch one bounded Codex or Claude turn into an exact ready owner sandbox and return ordered native activity.",
    action: "dispatch_managed_sandbox_work",
    label: "Dispatch",
    schema: DispatchSchema,
    extraProperties: {
      turnRef: { type: "string" },
      capabilityRef: { type: "string" },
      prompt: { maxLength: 100000, minLength: 1, type: "string" },
      runtime: {
        additionalProperties: false,
        properties: {
          provider: { enum: ["codex", "claude"], type: "string" },
          modelRef: { type: "string" },
          harnessRef: { type: "string" },
          reasoningEffort: { type: "string" },
        },
        required: ["provider", "modelRef", "harnessRef"],
        type: "object",
      },
    },
    extraRequired: ["turnRef", "capabilityRef", "prompt", "runtime"],
    make: (value, baseFields) =>
      ({
        ...baseFields,
        _tag: "Dispatch",
        sandboxRef: value.sandboxRef,
        expectedVersion: value.expectedVersion,
        turnRef: value.turnRef,
        capabilityRef: value.capabilityRef,
        promptDigest: "sha256:".padEnd(71, "0"),
        runtime: value.runtime,
      }) as ManagedSandboxCommand,
    prompt: (value) => value.prompt,
  });

  // The prompt digest must be computed from the exact prompt bytes. Wrap only
  // the dispatch command builder so the model cannot supply or substitute it.
  const exactDispatch: SarahAgentTool = {
    definition: dispatch.definition,
    execute: (raw, toolCall) =>
      Effect.gen(function* () {
        const value = yield* decode(DispatchSchema, raw);
        const promptDigest = yield* digest(value.prompt);
        const result = yield* run({
          action: "dispatch_managed_sandbox_work",
          label: "Dispatch",
          schema: DispatchSchema,
          raw,
          toolCallId: toolCall.id,
          evidence: commonEvidence,
          command: command(
            "dispatch",
            (decoded, baseFields) =>
              ({
                ...baseFields,
                _tag: "Dispatch",
                sandboxRef: decoded.sandboxRef,
                expectedVersion: decoded.expectedVersion,
                turnRef: decoded.turnRef,
                capabilityRef: decoded.capabilityRef,
                promptDigest: `sha256:${promptDigest}`,
                runtime: decoded.runtime,
              }) as ManagedSandboxCommand,
          ),
          prompt: (decoded) => decoded.prompt,
        });
        if (result.authorityAllowed === false) return result;
        const payload = JSON.parse(result.content) as {
          resource?: ManagedSandboxResource;
        };
        if (
          payload.resource === undefined ||
          !exactScope(payload.resource, value, deps.principal.ownerRef)
        ) {
          return yield* failure("managed_sandbox_scope_mismatch");
        }
        return result;
      }),
  };

  const interrupt = scopedTool({
    name: "managed_sandbox_interrupt",
    description:
      "Interrupt one exact running managed-sandbox turn idempotently; pending is not terminal.",
    action: "interrupt_managed_sandbox_turn",
    label: "Interrupt",
    schema: InterruptSchema,
    extraProperties: {
      turnRef: { type: "string" },
      reasonRef: { type: "string" },
    },
    extraRequired: ["turnRef", "reasonRef"],
    make: (value, baseFields) =>
      ({
        ...baseFields,
        _tag: "Interrupt",
        sandboxRef: value.sandboxRef,
        expectedVersion: value.expectedVersion,
        turnRef: value.turnRef,
        reasonRef: value.reasonRef,
      }) as ManagedSandboxCommand,
  });

  const lifecycle = (tag: "Stop" | "Resume" | "Delete", action: string) =>
    scopedTool({
      name: `managed_sandbox_${tag.toLowerCase()}`,
      description:
        tag === "Delete"
          ? "Delete one exact owner sandbox; report deleting, recovery-required, and cleanup-complete as distinct outcomes."
          : `${tag} one exact owner sandbox through the receipt-bearing lifecycle broker.`,
      action,
      label: tag,
      schema: ActionSchema,
      extraProperties: { reasonRef: { type: "string" } },
      extraRequired: tag === "Resume" ? [] : ["reasonRef"],
      make: (value, baseFields) =>
        ({
          ...baseFields,
          _tag: tag,
          sandboxRef: value.sandboxRef,
          expectedVersion: value.expectedVersion,
          ...(tag === "Resume"
            ? {}
            : {
                reasonRef: value.reasonRef ?? `reason.sarah.sbx.${tag.toLowerCase()}`,
              }),
        }) as ManagedSandboxCommand,
    });

  return [
    create,
    list,
    inspect,
    exactDispatch,
    interrupt,
    lifecycle("Stop", "stop_managed_sandbox"),
    lifecycle("Resume", "resume_managed_sandbox"),
    lifecycle("Delete", "delete_managed_sandbox"),
  ];
};
