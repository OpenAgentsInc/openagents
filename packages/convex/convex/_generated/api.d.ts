/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";
import type * as claude from "../claude.js";
import type * as confect_apm from "../confect/apm.js";
import type * as confect_github from "../confect/github.js";
import type * as confect_mobile_sync from "../confect/mobile_sync.js";
import type * as confect_onboarding from "../confect/onboarding.js";
import type * as confect_users from "../confect/users.js";
import type * as github from "../github.js";
import type * as messages from "../messages.js";
import type * as migration from "../migration.js";

/**
 * A utility for referencing Convex functions in your app's API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = api.myModule.myFunction;
 * ```
 */
declare const fullApi: ApiFromModules<{
  claude: typeof claude;
  "confect/apm": typeof confect_apm;
  "confect/github": typeof confect_github;
  "confect/mobile_sync": typeof confect_mobile_sync;
  "confect/onboarding": typeof confect_onboarding;
  "confect/users": typeof confect_users;
  github: typeof github;
  messages: typeof messages;
  migration: typeof migration;
}>;
export declare const api: FilterApi<
  typeof fullApi,
  FunctionReference<any, "public">
>;
export declare const internal: FilterApi<
  typeof fullApi,
  FunctionReference<any, "internal">
>;
