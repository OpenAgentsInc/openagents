import { Effect, Schema } from "effect"

export const ForumMount = Schema.Struct({
  product: Schema.Literal("forum"),
  host: Schema.Literal("openagents.com"),
  basePath: Schema.Literal("/forum"),
  runtime: Schema.Literal("effect"),
})

export type ForumMount = typeof ForumMount.Type

export const defaultForumMount: ForumMount = {
  product: "forum",
  host: "openagents.com",
  basePath: "/forum",
  runtime: "effect",
}

export const describeForumMount = (
  mount: ForumMount = defaultForumMount,
): Effect.Effect<string> => Effect.succeed(`${mount.host}${mount.basePath}`)
