import {
  FULL_AUTO_CONTROL_SCHEMA,
  FULL_AUTO_CONTROL_TURNS_LIMIT,
} from "./full-auto-control-contract.ts"

/**
 * FA-H13 (#8886): the hand-authored OpenAPI 3.1 document for the Phase 1
 * local Full Auto control surface. This document IS the deliverable the MCP
 * server and CLI are thin pass-through clients of -- it must describe every
 * served route, the bearer auth requirement, and every schema exactly; the
 * parity test in full-auto-control-server.test.ts asserts it against the
 * shared FULL_AUTO_CONTROL_ROUTES table in both directions, and the response
 * shapes are decoded against the Effect Schemas in
 * full-auto-control-contract.ts by the same test file.
 *
 * Shape/tone follows the repo's OpenAPI-triad convention
 * (apps/openagents.com/workers/api/src/openagents-openapi-routes.ts): one
 * typed document, served by the same process that implements it.
 */

const threadRefParameter = {
  name: "threadRef",
  in: "path",
  required: true,
  description: "The Desktop thread ref the Full Auto record belongs to.",
  schema: { type: "string", minLength: 1, maxLength: 120 },
} as const

const errorResponseSchema = { $ref: "#/components/schemas/FullAutoControlError" } as const

const unauthorizedResponse = {
  description: "Missing, malformed, or wrong bearer credential.",
  content: { "application/json": { schema: errorResponseSchema } },
} as const

const notFoundResponse = {
  description: "No Full Auto record exists for that threadRef.",
  content: { "application/json": { schema: errorResponseSchema } },
} as const

const invalidRequestResponse = {
  description: "The path parameter or request body failed schema validation.",
  content: { "application/json": { schema: errorResponseSchema } },
} as const

export const fullAutoControlOpenApiDocument = {
  openapi: "3.1.0",
  info: {
    title: "OpenAgents Desktop Full Auto Control API",
    version: "1.0.0",
    description:
      "Loopback-only (127.0.0.1), opt-in local control surface for OpenAgents Desktop Full Auto. " +
      "Off by default; Desktop main serves it only when OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1. " +
      "Every request requires the per-process scoped bearer credential written to " +
      "full-auto/control.json under the Desktop userData directory. Enable never grants a new " +
      "workspace: the caller names the workspace it expects and the server refuses (409) on any " +
      "mismatch with the currently resolved workspace. Start and enable accept an optional " +
      "ProviderLane ref (default codex-local) and refuse lanes that cannot safely settle " +
      "background questions. Every mutation appends a durable, " +
      "distinctly-attributed system note to the thread. Phase 2 (cross-machine relay) is " +
      "explicitly out of scope for this surface.",
  },
  servers: [{ url: "http://127.0.0.1:{port}", variables: { port: { default: "0" } } }],
  security: [{ controlBearer: [] }],
  paths: {
    "/v1/openapi.json": {
      get: {
        operationId: "getOpenApiDocument",
        summary: "This OpenAPI 3.1 document.",
        responses: {
          "200": {
            description: "The OpenAPI document describing this exact surface.",
            content: { "application/json": { schema: { type: "object" } } },
          },
          "401": unauthorizedResponse,
        },
      },
    },
    "/v1/lanes": {
      get: {
        operationId: "listProviderLanes",
        summary: "List configured provider lanes and their honest current status.",
        responses: {
          "200": {
            description: "Public-safe provider lane registry, including unavailable and unadmitted lanes.",
            content: { "application/json": { schema: { type: "object" } } },
          },
          "401": unauthorizedResponse,
        },
      },
    },
    "/v1/full-auto": {
      get: {
        operationId: "listFullAuto",
        summary: "List every Full Auto registry record (public-safe projection).",
        responses: {
          "200": {
            description: "All records with their coarse live state.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlListResponse" },
              },
            },
          },
          "401": unauthorizedResponse,
        },
      },
    },
    "/v1/full-auto/start": {
      post: {
        operationId: "startFullAuto",
        summary: "Bootstrap Full Auto: mint a new thread, enable it, and schedule the first turn.",
        description:
          "Programmatic bootstrap for agents that have no existing thread. The caller MUST name " +
          "the workspace it expects (workspaceRef) exactly like enable; on 409 workspace_mismatch " +
          "NO thread is created and NO record is written. On success main mints a brand-new local " +
          "thread (the server names the ref, never the caller), binds the resolved workspace, " +
          "enables Full Auto, appends a distinctly-attributed system note, and schedules the " +
          "shared serialized reconcile pass so the first continuation dispatches without a " +
          "separate continue-now call. The new threadRef is returned inside the record.",
        requestBody: {
          required: true,
          content: {
            "application/json": {
              schema: { $ref: "#/components/schemas/FullAutoControlStartRequest" },
            },
          },
        },
        responses: {
          "200": {
            description: "Thread minted, enabled, workspace-bound, first continuation scheduled.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlMutationResponse" },
              },
            },
          },
          "400": invalidRequestResponse,
          "401": unauthorizedResponse,
          "409": {
            description:
              "The named workspace does not match the currently resolved workspace; no thread " +
              "was created and the registry is left untouched.",
            content: { "application/json": { schema: errorResponseSchema } },
          },
        },
      },
    },
    "/v1/full-auto/{threadRef}": {
      get: {
        operationId: "getFullAutoStatus",
        summary: "One thread's Full Auto record plus coarse live state.",
        parameters: [threadRefParameter],
        responses: {
          "200": {
            description: "The record projection.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlStatusResponse" },
              },
            },
          },
          "400": invalidRequestResponse,
          "401": unauthorizedResponse,
          "404": notFoundResponse,
        },
      },
    },
    "/v1/full-auto/{threadRef}/enable": {
      post: {
        operationId: "enableFullAuto",
        summary: "Enable Full Auto for a thread, naming the expected workspace.",
        description:
          "The caller MUST name the workspace it expects (workspaceRef). The server resolves the " +
          "current workspace itself (the same resolution codex-local turns execute against) and " +
          "refuses with 409 workspace_mismatch when the caller's expectation does not match -- a " +
          "mismatch is a refusal, never a redirect, and this route can never grant a new, " +
          "previously-ungranted workspace. On success the record is enabled and bound to the " +
          "resolved workspace, and a distinctly-attributed system note is appended to the thread.",
        parameters: [threadRefParameter],
        requestBody: {
          required: true,
          content: {
            "application/json": {
              schema: { $ref: "#/components/schemas/FullAutoControlEnableRequest" },
            },
          },
        },
        responses: {
          "200": {
            description: "Enabled and workspace-bound.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlMutationResponse" },
              },
            },
          },
          "400": invalidRequestResponse,
          "401": unauthorizedResponse,
          "409": {
            description:
              "The named workspace does not match the currently resolved workspace; the registry " +
              "is left untouched.",
            content: { "application/json": { schema: errorResponseSchema } },
          },
        },
      },
    },
    "/v1/full-auto/{threadRef}/disable": {
      post: {
        operationId: "disableFullAuto",
        summary: "Durably disable Full Auto for a thread.",
        parameters: [threadRefParameter],
        responses: {
          "200": {
            description: "Disabled durably; a distinctly-attributed note was appended.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlMutationResponse" },
              },
            },
          },
          "400": invalidRequestResponse,
          "401": unauthorizedResponse,
        },
      },
    },
    "/v1/full-auto/{threadRef}/continue-now": {
      post: {
        operationId: "continueFullAutoNow",
        summary: "Trigger an immediate reconciliation attempt.",
        description:
          "Schedules the exact same serialized reconciliation pass every other Full Auto trigger " +
          "uses (a new trigger point, not a new dispatch mechanism) and returns immediately; the " +
          "reconcile runs asynchronously and dispatch remains subject to the durable lease, " +
          "workspace binding, backoff, and cap policies.",
        parameters: [threadRefParameter],
        responses: {
          "200": {
            description: "The shared reconcile pass was scheduled.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlContinueNowResponse" },
              },
            },
          },
          "400": invalidRequestResponse,
          "401": unauthorizedResponse,
          "404": notFoundResponse,
        },
      },
    },
    "/v1/full-auto/{threadRef}/turns": {
      get: {
        operationId: "listFullAutoTurns",
        summary: "Bounded recent Full Auto turn history for a thread.",
        description:
          `Last ${FULL_AUTO_CONTROL_TURNS_LIMIT} Full Auto continuation turns from the local-turn ` +
          "journal: turn identity, phase, disposition, and timestamps only -- never transcript text.",
        parameters: [threadRefParameter],
        responses: {
          "200": {
            description: "The bounded turn projection, most recent first.",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/FullAutoControlTurnsResponse" },
              },
            },
          },
          "400": invalidRequestResponse,
          "401": unauthorizedResponse,
        },
      },
    },
  },
  components: {
    securitySchemes: {
      controlBearer: {
        type: "http",
        scheme: "bearer",
        description:
          "Per-process scoped credential minted at server start and written (mode 0600) to " +
          "full-auto/control.json under the Desktop userData directory.",
      },
    },
    schemas: {
      FullAutoControlLive: {
        type: "object",
        required: ["state", "turnRef"],
        additionalProperties: false,
        properties: {
          state: {
            type: "string",
            enum: ["idle", "turn_running", "turn_completed", "turn_failed", "cap_reached", "blocked"],
          },
          turnRef: { type: ["string", "null"], minLength: 1, maxLength: 180 },
          detail: { type: "string", minLength: 1, maxLength: 300 },
        },
      },
      FullAutoControlRecord: {
        type: "object",
        required: [
          "threadRef",
          "enabled",
          "continuationCount",
          "updatedAt",
          "workspaceRef",
          "lane",
          "accountRef",
          "blockedReason",
          "disabledBy",
          "disabledAt",
          "live",
        ],
        additionalProperties: false,
        properties: {
          threadRef: { type: "string", minLength: 1, maxLength: 120 },
          enabled: { type: "boolean" },
          continuationCount: { type: "integer", minimum: 0 },
          updatedAt: { type: "string" },
          workspaceRef: { type: ["string", "null"], minLength: 1, maxLength: 1024 },
          lane: { type: "string", minLength: 1, maxLength: 80 },
          accountRef: { type: ["string", "null"], minLength: 1, maxLength: 80 },
          blockedReason: { type: ["string", "null"], minLength: 1, maxLength: 300 },
          disabledBy: {
            type: ["string", "null"],
            enum: [
              "ui_toggle",
              "control_api",
              "workspace_guard",
              "continuation_cap",
              "dispatch_failure_limit",
              null,
            ],
          },
          disabledAt: { type: ["string", "null"] },
          live: { $ref: "#/components/schemas/FullAutoControlLive" },
        },
      },
      FullAutoControlListResponse: {
        type: "object",
        required: ["schema", "serverInstanceId", "records"],
        additionalProperties: false,
        properties: {
          schema: { type: "string", const: FULL_AUTO_CONTROL_SCHEMA },
          serverInstanceId: { type: "string", minLength: 16, maxLength: 120 },
          records: { type: "array", items: { $ref: "#/components/schemas/FullAutoControlRecord" } },
        },
      },
      FullAutoControlStatusResponse: {
        type: "object",
        required: ["schema", "serverInstanceId", "record"],
        additionalProperties: false,
        properties: {
          schema: { type: "string", const: FULL_AUTO_CONTROL_SCHEMA },
          serverInstanceId: { type: "string", minLength: 16, maxLength: 120 },
          record: { $ref: "#/components/schemas/FullAutoControlRecord" },
        },
      },
      FullAutoControlEnableRequest: {
        type: "object",
        required: ["workspaceRef"],
        additionalProperties: false,
        properties: {
          workspaceRef: {
            type: "string",
            minLength: 1,
            maxLength: 1024,
            description: "The absolute workspace path the caller expects Full Auto to run against.",
          },
          lane: {
            type: "string",
            minLength: 1,
            maxLength: 80,
            default: "codex-local",
            description: "Optional admitted ProviderLane ref; defaults to codex-local.",
          },
        },
      },
      FullAutoControlStartRequest: {
        type: "object",
        required: ["workspaceRef"],
        additionalProperties: false,
        properties: {
          workspaceRef: {
            type: "string",
            minLength: 1,
            maxLength: 1024,
            description: "The absolute workspace path the caller expects Full Auto to run against.",
          },
          lane: {
            type: "string",
            minLength: 1,
            maxLength: 80,
            default: "codex-local",
            description: "Optional admitted ProviderLane ref; defaults to codex-local.",
          },
          title: {
            type: "string",
            minLength: 1,
            maxLength: 80,
            description: "Optional owner-visible title for the minted thread.",
          },
        },
      },
      FullAutoControlMutationResponse: {
        type: "object",
        required: ["schema", "ok", "record"],
        additionalProperties: false,
        properties: {
          schema: { type: "string", const: FULL_AUTO_CONTROL_SCHEMA },
          ok: { type: "boolean", const: true },
          record: { $ref: "#/components/schemas/FullAutoControlRecord" },
        },
      },
      FullAutoControlContinueNowResponse: {
        type: "object",
        required: ["schema", "scheduled"],
        additionalProperties: false,
        properties: {
          schema: { type: "string", const: FULL_AUTO_CONTROL_SCHEMA },
          scheduled: { type: "boolean", const: true },
        },
      },
      FullAutoControlTurn: {
        type: "object",
        required: ["turnRef", "phase", "disposition", "createdAt", "updatedAt"],
        additionalProperties: false,
        properties: {
          turnRef: { type: "string", minLength: 1, maxLength: 180 },
          phase: {
            type: "string",
            enum: [
              "accepted",
              "dispatching",
              "attached",
              "streaming",
              "recovering",
              "completed",
              "failed",
              "interrupted",
              "interrupted_by_restart",
            ],
          },
          disposition: {
            type: ["string", "null"],
            enum: [
              "completed",
              "failed",
              "owner_interrupted",
              "resumed_after_restart",
              "interrupted_by_restart",
              null,
            ],
          },
          createdAt: { type: "string" },
          updatedAt: { type: "string" },
        },
      },
      FullAutoControlTurnsResponse: {
        type: "object",
        required: ["schema", "threadRef", "turns"],
        additionalProperties: false,
        properties: {
          schema: { type: "string", const: FULL_AUTO_CONTROL_SCHEMA },
          threadRef: { type: "string", minLength: 1, maxLength: 120 },
          turns: {
            type: "array",
            maxItems: FULL_AUTO_CONTROL_TURNS_LIMIT,
            items: { $ref: "#/components/schemas/FullAutoControlTurn" },
          },
        },
      },
      FullAutoControlError: {
        type: "object",
        required: ["error", "message"],
        additionalProperties: false,
        properties: {
          error: {
            type: "string",
            enum: [
              "unauthorized",
              "not_found",
              "method_not_allowed",
              "invalid_request",
              "workspace_mismatch",
              "lane_not_eligible",
            ],
          },
          message: { type: "string", minLength: 1, maxLength: 600 },
          expectedWorkspaceRef: { type: "string", minLength: 1, maxLength: 1024 },
          resolvedWorkspaceRef: { type: "string", minLength: 1, maxLength: 1024 },
        },
      },
    },
  },
} as const
