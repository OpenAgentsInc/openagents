/**
 * Type definitions and error types for the dedicated Effect-TS service layer
 * Following EffectPatterns/define-tagged-errors.mdx pattern
 */

import { Data, Schema } from "effect";

// Branded types for stronger type safety
export type SessionId = string & { readonly _brand: 'SessionId' };
export type UserId = string & { readonly _brand: 'UserId' };
export type MessageId = string & { readonly _brand: 'MessageId' };
export type ProjectPath = string & { readonly _brand: 'ProjectPath' };

// Tagged error types for comprehensive error handling
export class SessionCreationError extends Data.TaggedError("SessionCreationError")<{
  readonly reason: string;
  readonly sessionId: string;
  readonly metadata?: Record<string, any>;
}> {}

export class SessionNotFoundError extends Data.TaggedError("SessionNotFoundError")<{
  readonly sessionId: string;
}> {}

export class SessionPermissionError extends Data.TaggedError("SessionPermissionError")<{
  readonly sessionId: string;
  readonly userId: string;
  readonly action: string;
}> {}

export class SessionValidationError extends Data.TaggedError("SessionValidationError")<{
  readonly reason: string;
  readonly sessionId: string;
  readonly field?: string;
}> {}

export class DatabaseOperationError extends Data.TaggedError("DatabaseOperationError")<{
  readonly operation: string;
  readonly sessionId?: string;
  readonly cause: unknown;
}> {}

export class AuthenticationError extends Data.TaggedError("AuthenticationError")<{
  readonly reason: string;
  readonly userId?: string;
}> {}

export class ProcessingTimeoutError extends Data.TaggedError("ProcessingTimeoutError")<{
  readonly timeoutMs: number;
  readonly operation: string;
  readonly sessionId?: string;
}> {}

// Service interfaces and types
export interface CreateSessionParams {
  readonly sessionId: string;
  readonly projectPath: string;
  readonly createdBy: "desktop" | "mobile";
  readonly title?: string;
  readonly initialMessage?: string;
  readonly metadata?: SessionMetadata;
}

export interface SessionMetadata {
  readonly workingDirectory?: string;
  readonly model?: string;
  readonly systemPrompt?: string;
  readonly originalMobileSessionId?: string;
  readonly aiModel?: string;
  readonly contextWindow?: number;
}

export interface SessionData {
  readonly sessionId: string;
  readonly projectPath: string;
  readonly title: string;
  readonly status: SessionStatus;
  readonly createdBy: "desktop" | "mobile";
  readonly lastActivity: number;
  readonly createdAt: number;
  readonly userId?: string;
  readonly metadata: SessionMetadata;
  readonly version: number;
}

export type SessionStatus = "active" | "inactive" | "error" | "processed" | "offline";

export interface SessionUpdate {
  readonly sessionId: string;
  readonly type: "MESSAGE_ADDED" | "STATUS_CHANGED" | "METADATA_UPDATED";
  readonly data: any;
  readonly timestamp: number;
}

export interface SessionQueryCriteria {
  readonly userId?: string;
  readonly status?: SessionStatus;
  readonly createdBy?: "desktop" | "mobile";
  readonly limit?: number;
  readonly sortBy?: "lastActivity" | "createdAt";
  readonly sortOrder?: "asc" | "desc";
}

export interface SessionQueryResult {
  readonly sessions: SessionData[];
  readonly totalCount: number;
  readonly hasMore: boolean;
}

// Retry policy configuration
export interface RetryPolicy {
  readonly maxRetries: number;
  readonly baseDelay: string;
  readonly timeout: string;
}

export interface CircuitBreakerConfig {
  readonly name: string;
  readonly failureThreshold: number;
  readonly timeout: string;
}

// Schema definitions for validation
export const CreateSessionParamsSchema = Schema.Struct({
  sessionId: Schema.String,
  projectPath: Schema.String,
  createdBy: Schema.Literal("desktop", "mobile"),
  title: Schema.optional(Schema.String),
  initialMessage: Schema.optional(Schema.String),
  metadata: Schema.optional(
    Schema.Struct({
      workingDirectory: Schema.optional(Schema.String),
      model: Schema.optional(Schema.String),
      systemPrompt: Schema.optional(Schema.String),
      originalMobileSessionId: Schema.optional(Schema.String),
      aiModel: Schema.optional(Schema.String),
      contextWindow: Schema.optional(Schema.Number),
    })
  ),
});

export const SessionDataSchema = Schema.Struct({
  sessionId: Schema.String,
  projectPath: Schema.String,
  title: Schema.String,
  status: Schema.Literal("active", "inactive", "error", "processed", "offline"),
  createdBy: Schema.Literal("desktop", "mobile"),
  lastActivity: Schema.Number,
  createdAt: Schema.Number,
  userId: Schema.optional(Schema.String),
  metadata: Schema.Struct({
    workingDirectory: Schema.optional(Schema.String),
    model: Schema.optional(Schema.String),
    systemPrompt: Schema.optional(Schema.String),
    originalMobileSessionId: Schema.optional(Schema.String),
    aiModel: Schema.optional(Schema.String),
    contextWindow: Schema.optional(Schema.Number),
  }),
  version: Schema.Number,
});

// Branded type helpers (for future use)
export const createSessionId = (id: string): SessionId => id as SessionId;
export const createUserId = (id: string): UserId => id as UserId;
export const createMessageId = (id: string): MessageId => id as MessageId;
export const createProjectPath = (path: string): ProjectPath => path as ProjectPath;