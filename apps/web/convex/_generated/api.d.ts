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
import type * as autopilot_featureRequests from "../autopilot/featureRequests.js";
import type * as autopilot_messages from "../autopilot/messages.js";
import type * as autopilot_reset from "../autopilot/reset.js";
import type * as autopilot_threads from "../autopilot/threads.js";
import type * as autopilot_traces from "../autopilot/traces.js";
import type * as crons from "../crons.js";
import type * as dse_active from "../dse/active.js";
import type * as dse_artifacts from "../dse/artifacts.js";
import type * as dse_blobs from "../dse/blobs.js";
import type * as dse_canary from "../dse/canary.js";
import type * as dse_compileReports from "../dse/compileReports.js";
import type * as dse_evalReports from "../dse/evalReports.js";
import type * as dse_examples from "../dse/examples.js";
import type * as dse_opsAdmin from "../dse/opsAdmin.js";
import type * as dse_opsRuns from "../dse/opsRuns.js";
import type * as dse_receipts from "../dse/receipts.js";
import type * as dse_varSpace from "../dse/varSpace.js";
import type * as effect_auth from "../effect/auth.js";
import type * as effect_ctx from "../effect/ctx.js";
import type * as effect_functions from "../effect/functions.js";
import type * as effect_scheduler from "../effect/scheduler.js";
import type * as effect_storage from "../effect/storage.js";
import type * as effect_tryPromise from "../effect/tryPromise.js";
import type * as effect_validators from "../effect/validators.js";
import type * as lightning_tasks from "../lightning/tasks.js";
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
  "autopilot/featureRequests": typeof autopilot_featureRequests;
  "autopilot/messages": typeof autopilot_messages;
  "autopilot/reset": typeof autopilot_reset;
  "autopilot/threads": typeof autopilot_threads;
  "autopilot/traces": typeof autopilot_traces;
  crons: typeof crons;
  "dse/active": typeof dse_active;
  "dse/artifacts": typeof dse_artifacts;
  "dse/blobs": typeof dse_blobs;
  "dse/canary": typeof dse_canary;
  "dse/compileReports": typeof dse_compileReports;
  "dse/evalReports": typeof dse_evalReports;
  "dse/examples": typeof dse_examples;
  "dse/opsAdmin": typeof dse_opsAdmin;
  "dse/opsRuns": typeof dse_opsRuns;
  "dse/receipts": typeof dse_receipts;
  "dse/varSpace": typeof dse_varSpace;
  "effect/auth": typeof effect_auth;
  "effect/ctx": typeof effect_ctx;
  "effect/functions": typeof effect_functions;
  "effect/scheduler": typeof effect_scheduler;
  "effect/storage": typeof effect_storage;
  "effect/tryPromise": typeof effect_tryPromise;
  "effect/validators": typeof effect_validators;
  "lightning/tasks": typeof lightning_tasks;
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
