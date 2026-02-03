/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type * as billing from "../billing.js";
import type * as http from "../http.js";
import type * as lib_errors from "../lib/errors.js";
import type * as myFunctions from "../myFunctions.js";
import type * as nostr from "../nostr.js";
import type * as nostr_http from "../nostr_http.js";
import type * as openclaw from "../openclaw.js";

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";

declare const fullApi: ApiFromModules<{
  billing: typeof billing;
  http: typeof http;
  "lib/errors": typeof lib_errors;
  myFunctions: typeof myFunctions;
  nostr: typeof nostr;
  nostr_http: typeof nostr_http;
  openclaw: typeof openclaw;
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
