import {
  HttpApi,
  HttpApiBuilder,
  HttpApiEndpoint,
  HttpApiGroup,
  OpenApi,
} from "@effect/platform";
import type { HttpApiDecodeError } from "@effect/platform/HttpApiError";
import { Effect, Layer, Option, Schema } from "effect";
import { ConfectActionCtx } from "./confect";
import { confectSchema } from "./schema";
import {
  GetCurrentUserResult,
  GetUserByIdArgs,
  GetUserByIdResult,
} from "./users.schemas";
import {
  GetMessagesResult,
  GetSessionMessagesArgs,
  GetSessionMessagesResult,
} from "./messages.schemas";

// Define API groups
class UsersApiGroup extends HttpApiGroup.make("users")
  .add(
    HttpApiEndpoint.get("getCurrentUser", "/current")
      .annotate(OpenApi.Description, "Get the currently authenticated user.")
      .addSuccess(GetCurrentUserResult)
  )
  .add(
    HttpApiEndpoint.get("getUserById", "/user/:userId")
      .annotate(OpenApi.Description, "Get a user by their ID.")
      .addSuccess(GetUserByIdResult)
  )
  .annotate(OpenApi.Title, "Users")
  .annotate(OpenApi.Description, "Operations on users.") {}

class MessagesApiGroup extends HttpApiGroup.make("messages")
  .add(
    HttpApiEndpoint.get("getMessages", "/")
      .annotate(OpenApi.Description, "Get all basic messages (demo compatibility).")
      .addSuccess(GetMessagesResult)
  )
  .add(
    HttpApiEndpoint.get("getSessionMessages", "/session/:sessionId")
      .annotate(OpenApi.Description, "Get all messages for a specific Claude session.")
      .addSuccess(GetSessionMessagesResult)
  )
  .annotate(OpenApi.Title, "Messages")
  .annotate(OpenApi.Description, "Operations on messages and Claude session messages.") {}

class SessionsApiGroup extends HttpApiGroup.make("sessions")
  .add(
    HttpApiEndpoint.get("getPendingSessions", "/pending")
      .annotate(OpenApi.Description, "Get pending mobile sessions that need desktop attention.")
      .addSuccess(
        Schema.Array(
          Schema.Struct({
            _id: Schema.String,
            sessionId: Schema.String,
            projectPath: Schema.String,
            title: Schema.optional(Schema.String),
            status: Schema.Literal("active", "inactive", "error", "processed"),
            createdBy: Schema.Literal("desktop", "mobile"),
            lastActivity: Schema.Number,
            _creationTime: Schema.Number,
          })
        )
      )
  )
  .annotate(OpenApi.Title, "Sessions")
  .annotate(OpenApi.Description, "Operations on Claude Code sessions.") {}

// Main API definition
export class OpenAgentsApi extends HttpApi.make("OpenAgentsApi")
  .annotate(OpenApi.Title, "OpenAgents API")
  .annotate(
    OpenApi.Description,
    `
# OpenAgents API

A powerful API for the OpenAgents multi-platform Claude Code integration, built with Confect and powered by Effect-TS.

## Features

- **Type-safe operations** with Effect Schema validation
- **Multi-platform sync** between desktop and mobile
- **Real-time messaging** with Claude Code sessions
- **User authentication** with GitHub integration

## Getting Started

This API uses Effect-TS patterns with Option types for null safety and comprehensive error handling.

Learn more about OpenAgents at [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents).
`
  )
  .add(UsersApiGroup)
  .add(MessagesApiGroup)
  .add(SessionsApiGroup)
  .prefix("/api/v1") {}

// Implementation of API groups
const UsersApiGroupLive = HttpApiBuilder.group(OpenAgentsApi, "users", (handlers) =>
  handlers
    .handle("getCurrentUser", () =>
      Effect.gen(function* () {
        // TODO: Fix HTTP API integration with Confect functions
        return Option.none();
      })
    )
    .handle("getUserById", ({ request }) =>
      Effect.gen(function* () {
        // TODO: Fix HTTP API integration with Confect functions
        return Option.none();
      })
    )
);

const MessagesApiGroupLive = HttpApiBuilder.group(OpenAgentsApi, "messages", (handlers) =>
  handlers
    .handle("getMessages", () =>
      Effect.gen(function* () {
        // TODO: Fix HTTP API integration with Confect functions
        return [];
      })
    )
    .handle("getSessionMessages", ({ request }) =>
      Effect.gen(function* () {
        // TODO: Fix HTTP API integration with Confect functions
        return [];
      })
    )
);

const SessionsApiGroupLive = HttpApiBuilder.group(OpenAgentsApi, "sessions", (handlers) =>
  handlers.handle("getPendingSessions", () =>
    Effect.gen(function* () {
      // TODO: Fix HTTP API integration with Confect functions
      return [];
    })
  )
);

// Combine all API group implementations
export const OpenAgentsApiLive = HttpApiBuilder.api(OpenAgentsApi).pipe(
  Layer.provide(UsersApiGroupLive),
  Layer.provide(MessagesApiGroupLive),
  Layer.provide(SessionsApiGroupLive)
);