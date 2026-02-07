/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type * as effect_auth from "../effect/auth.js";
import type * as effect_ctx from "../effect/ctx.js";
import type * as effect_functions from "../effect/functions.js";
import type * as effect_scheduler from "../effect/scheduler.js";
import type * as effect_storage from "../effect/storage.js";
import type * as effect_tryPromise from "../effect/tryPromise.js";
import type * as effect_validators from "../effect/validators.js";
import type * as myFunctions from "../myFunctions.js";

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";

declare const fullApi: ApiFromModules<{
  "effect/auth": typeof effect_auth;
  "effect/ctx": typeof effect_ctx;
  "effect/functions": typeof effect_functions;
  "effect/scheduler": typeof effect_scheduler;
  "effect/storage": typeof effect_storage;
  "effect/tryPromise": typeof effect_tryPromise;
  "effect/validators": typeof effect_validators;
  myFunctions: typeof myFunctions;
}>;

/**
 * A utility for referencing Convex functions in your app's public API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = api.myModule.myFunction;
 * ```
 */
export declare const api: FilterApi<
  typeof fullApi,
  FunctionReference<any, "public">
>;

/**
 * A utility for referencing Convex functions in your app's internal API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = internal.myModule.myFunction;
 * ```
 */
export declare const internal: FilterApi<
  typeof fullApi,
  FunctionReference<any, "internal">
>;

export declare const components: {};
