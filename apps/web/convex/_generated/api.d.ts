/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type * as autopilot_access from "../autopilot/access.js";
import type * as autopilot_blueprint from "../autopilot/blueprint.js";
import type * as autopilot_defaults from "../autopilot/defaults.js";
import type * as autopilot_messages from "../autopilot/messages.js";
import type * as autopilot_reset from "../autopilot/reset.js";
import type * as autopilot_threads from "../autopilot/threads.js";
import type * as dse_active from "../dse/active.js";
import type * as dse_artifacts from "../dse/artifacts.js";
import type * as dse_examples from "../dse/examples.js";
import type * as dse_receipts from "../dse/receipts.js";
import type * as effect_auth from "../effect/auth.js";
import type * as effect_ctx from "../effect/ctx.js";
import type * as effect_functions from "../effect/functions.js";
import type * as effect_scheduler from "../effect/scheduler.js";
import type * as effect_storage from "../effect/storage.js";
import type * as effect_tryPromise from "../effect/tryPromise.js";
import type * as effect_validators from "../effect/validators.js";
import type * as testing_resetAll from "../testing/resetAll.js";

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";

declare const fullApi: ApiFromModules<{
  "autopilot/access": typeof autopilot_access;
  "autopilot/blueprint": typeof autopilot_blueprint;
  "autopilot/defaults": typeof autopilot_defaults;
  "autopilot/messages": typeof autopilot_messages;
  "autopilot/reset": typeof autopilot_reset;
  "autopilot/threads": typeof autopilot_threads;
  "dse/active": typeof dse_active;
  "dse/artifacts": typeof dse_artifacts;
  "dse/examples": typeof dse_examples;
  "dse/receipts": typeof dse_receipts;
  "effect/auth": typeof effect_auth;
  "effect/ctx": typeof effect_ctx;
  "effect/functions": typeof effect_functions;
  "effect/scheduler": typeof effect_scheduler;
  "effect/storage": typeof effect_storage;
  "effect/tryPromise": typeof effect_tryPromise;
  "effect/validators": typeof effect_validators;
  "testing/resetAll": typeof testing_resetAll;
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
