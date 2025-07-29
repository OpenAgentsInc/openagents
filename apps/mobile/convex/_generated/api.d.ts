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
import type * as confect_apm from "../confect/apm.js";
import type * as confect_confect from "../confect/confect.js";
import type * as confect_error_tracking from "../confect/error-tracking.js";
import type * as confect_github from "../confect/github.js";
import type * as confect_http_api from "../confect/http-api.js";
import type * as confect_http from "../confect/http.js";
import type * as confect_index from "../confect/index.js";
import type * as confect_integration from "../confect/integration.js";
import type * as confect_messages from "../confect/messages.js";
import type * as confect_mobile_sync from "../confect/mobile-sync.js";
import type * as confect_onboarding from "../confect/onboarding.js";
import type * as confect_users from "../confect/users.js";
import type * as confect_validation from "../confect/validation.js";

/**
 * A utility for referencing Convex functions in your app's API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = api.myModule.myFunction;
 * ```
 */
declare const fullApi: ApiFromModules<{
  "confect/apm": typeof confect_apm;
  "confect/confect": typeof confect_confect;
  "confect/error-tracking": typeof confect_error_tracking;
  "confect/github": typeof confect_github;
  "confect/http-api": typeof confect_http_api;
  "confect/http": typeof confect_http;
  "confect/index": typeof confect_index;
  "confect/integration": typeof confect_integration;
  "confect/messages": typeof confect_messages;
  "confect/mobile-sync": typeof confect_mobile_sync;
  "confect/onboarding": typeof confect_onboarding;
  "confect/users": typeof confect_users;
  "confect/validation": typeof confect_validation;
}>;
export declare const api: FilterApi<
  typeof fullApi,
  FunctionReference<any, "public">
>;
export declare const internal: FilterApi<
  typeof fullApi,
  FunctionReference<any, "internal">
>;
