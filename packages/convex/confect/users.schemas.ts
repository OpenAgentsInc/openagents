import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

// GetOrCreateUser schemas
export const GetOrCreateUserArgs = Schema.Struct({
  email: Schema.String.pipe(Schema.nonEmptyString()),
  name: Schema.optional(Schema.String),
  avatar: Schema.optional(Schema.String),
  githubId: Schema.String.pipe(Schema.nonEmptyString()),
  githubUsername: Schema.String.pipe(Schema.nonEmptyString()),
});

export const GetOrCreateUserResult = Id.Id("users");

// GetCurrentUser schemas
export const GetCurrentUserArgs = Schema.Struct({});

export const GetCurrentUserResult = Schema.Option(
  Schema.Struct({
    _id: Id.Id("users"),
    email: Schema.String,
    name: Schema.optional(Schema.String),
    avatar: Schema.optional(Schema.String),
    githubId: Schema.String,
    githubUsername: Schema.String,
    createdAt: Schema.Number,
    lastLogin: Schema.Number,
    _creationTime: Schema.Number,
  })
);

// GetUserById schemas
export const GetUserByIdArgs = Schema.Struct({
  userId: Id.Id("users"),
});

export const GetUserByIdResult = Schema.Option(
  Schema.Struct({
    _id: Id.Id("users"),
    email: Schema.String,
    name: Schema.optional(Schema.String),
    avatar: Schema.optional(Schema.String),
    githubId: Schema.String,
    githubUsername: Schema.String,
    createdAt: Schema.Number,
    lastLogin: Schema.Number,
    _creationTime: Schema.Number,
  })
);