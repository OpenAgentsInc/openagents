/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type * as access from "../access.js";
import type * as admin from "../admin.js";
import type * as apiTokens from "../apiTokens.js";
import type * as billing from "../billing.js";
import type * as control_auth from "../control_auth.js";
import type * as http from "../http.js";
import type * as lib_admin from "../lib/admin.js";
import type * as lib_errors from "../lib/errors.js";
import type * as lib_users from "../lib/users.js";
import type * as myFunctions from "../myFunctions.js";
import type * as nostr from "../nostr.js";
import type * as nostr_http from "../nostr_http.js";
import type * as openclaw from "../openclaw.js";
import type * as openclawApi from "../openclawApi.js";
import type * as openclaw_control_http from "../openclaw_control_http.js";
import type * as users from "../users.js";
import type * as waitlist from "../waitlist.js";

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";

declare const fullApi: ApiFromModules<{
  access: typeof access;
  admin: typeof admin;
  apiTokens: typeof apiTokens;
  billing: typeof billing;
  control_auth: typeof control_auth;
  http: typeof http;
  "lib/admin": typeof lib_admin;
  "lib/errors": typeof lib_errors;
  "lib/users": typeof lib_users;
  myFunctions: typeof myFunctions;
  nostr: typeof nostr;
  nostr_http: typeof nostr_http;
  openclaw: typeof openclaw;
  openclawApi: typeof openclawApi;
  openclaw_control_http: typeof openclaw_control_http;
  users: typeof users;
  waitlist: typeof waitlist;
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
