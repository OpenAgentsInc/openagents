import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";
import { confectSchema } from "./schema";

// GetOrCreateUser schemas
export const GetOrCreateUserArgs = Schema.Struct({
  email: Schema.String.pipe(Schema.nonEmptyString()),
  name: Schema.optional(Schema.String),
  avatar: Schema.optional(Schema.String),
  githubId: Schema.String.pipe(Schema.nonEmptyString()),
  githubUsername: Schema.String.pipe(Schema.nonEmptyString()),
  openAuthSubject: Schema.optional(Schema.String.pipe(Schema.nonEmptyString())),
  githubAccessToken: Schema.optional(Schema.String.pipe(Schema.nonEmptyString())),
});

export const GetOrCreateUserResult = Id.Id("users");

// GetCurrentUser schemas
export const GetCurrentUserArgs = Schema.Struct({});

export const GetCurrentUserResult = Schema.Option(
  confectSchema.tableSchemas.users.withSystemFields
);

// GetUserById schemas
export const GetUserByIdArgs = Schema.Struct({
  userId: Id.Id("users"),
});

export const GetUserByIdResult = Schema.Option(
  confectSchema.tableSchemas.users.withSystemFields
);