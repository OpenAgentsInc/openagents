import { v } from 'convex/values';
import { Effect, Option } from 'effect';
import { effectAction, effectMutation, effectQuery } from './effect/functions';
import { tryPromise } from './effect/tryPromise';

// Write your Convex functions in any file inside this directory (`convex`).
// See https://docs.convex.dev/functions for more.

// You can read data from the database via a query:
export const listNumbers = effectQuery({
  // Validators for arguments.
  args: {
    count: v.number(),
  },
  returns: v.object({
    viewer: v.string(),
    numbers: v.array(v.number()),
  }),

  // Query implementation.
  handler: (ctx, args) =>
    Effect.gen(function* () {
      const user = yield* ctx.auth.getUserIdentity();

      const numbers = yield* tryPromise(() =>
        ctx.db
          .query('numbers')
          // Ordered by _creationTime, return most recent
          .order('desc')
          .take(args.count),
      );

      return {
        viewer: Option.match(user, {
          onNone: () => 'Anonymous',
          onSome: (u) => String((u as any).subject ?? 'Anonymous'),
        }),
        numbers: [...numbers].reverse().map((number: any) => number.value),
      };
    }),
});

// You can write data to the database via a mutation:
export const addNumber = effectMutation({
  // Validators for arguments.
  args: {
    value: v.number(),
  },
  returns: v.null(),

  // Mutation implementation.
  handler: (ctx, args) =>
    Effect.gen(function* () {
      const id = yield* tryPromise(() => ctx.db.insert('numbers', { value: args.value }));
      console.log('Added new document with id:', id);
      return null;
    }),
});

// You can fetch data from and send data to third-party APIs via an action:
export const myAction = effectAction({
  // Validators for arguments.
  args: {
    first: v.number(),
    second: v.string(),
  },
  returns: v.null(),

  // Action implementation.
  handler: (ctx, args) =>
    Effect.gen(function* () {
      // // Use the browser-like `fetch` API to send HTTP requests.
      // // See https://docs.convex.dev/functions/actions#calling-third-party-apis-and-using-npm-packages.
      // const response = await ctx.ctx.fetch("https://api.thirdpartyservice.com");
      // const data = await response.json();

      const user = yield* ctx.auth.getUserIdentity();
      console.log('myAction', {
        first: args.first,
        second: args.second,
        viewer: Option.match(user, {
          onNone: () => 'Anonymous',
          onSome: (u) => String((u as any).subject ?? 'Anonymous'),
        }),
      });
      return null;
    }),
});
