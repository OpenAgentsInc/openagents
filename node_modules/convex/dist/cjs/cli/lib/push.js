"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);
var push_exports = {};
__export(push_exports, {
  runNonComponentsPush: () => runNonComponentsPush
});
module.exports = __toCommonJS(push_exports);
var import_chalk = __toESM(require("chalk"), 1);
var import_log = require("../../bundler/log.js");
var import_codegen = require("./codegen.js");
var import_config = require("./config.js");
var import_indexes = require("./indexes.js");
var import_typecheck = require("./typecheck.js");
var import_utils = require("./utils/utils.js");
var import_debugBundlePath = require("./debugBundlePath.js");
async function runNonComponentsPush(ctx, options, configPath, projectConfig) {
  if (options.writePushRequest) {
    (0, import_log.logMessage)(
      "Skipping push because --write-push-request is set, but we are on the non-components path so there is nothing to write."
    );
    return;
  }
  const timeRunPushStarts = performance.now();
  const origin = options.url;
  const verbose = options.verbose || options.dryRun;
  if (verbose) {
    process.env["CONVEX_VERBOSE"] = "1";
  }
  await (0, import_utils.ensureHasConvexDependency)(ctx, "push");
  if (!options.codegen) {
    (0, import_log.logMessage)(
      import_chalk.default.gray("Skipping codegen. Remove --codegen=disable to enable.")
    );
    const funcDir = (0, import_utils.functionsDir)(configPath, projectConfig);
    await (0, import_typecheck.typeCheckFunctionsInMode)(ctx, options.typecheck, funcDir);
  } else {
    await (0, import_codegen.doCodegen)(
      ctx,
      (0, import_utils.functionsDir)(configPath, projectConfig),
      options.typecheck,
      options
    );
    if (verbose) {
      (0, import_log.logMessage)(import_chalk.default.green("Codegen finished."));
    }
  }
  if (options.debugNodeApis) {
    await (0, import_config.debugIsolateEndpointBundles)(ctx, projectConfig, configPath);
    (0, import_log.logFinishedStep)(
      "All non-'use node' entry points successfully bundled. Skipping rest of push."
    );
    return;
  }
  const timeBundleStarts = performance.now();
  const { config: localConfig, bundledModuleInfos } = await (0, import_config.configFromProjectConfig)(ctx, projectConfig, configPath, verbose);
  if (options.debugBundlePath) {
    await (0, import_debugBundlePath.handleDebugBundlePath)(ctx, options.debugBundlePath, localConfig);
    (0, import_log.logFinishedStep)(
      `Wrote bundle and metadata to ${options.debugBundlePath}. Skipping rest of push.`
    );
    return;
  }
  const timeSchemaPushStarts = performance.now();
  const { schemaId, schemaState } = await (0, import_indexes.pushSchema)(
    ctx,
    origin,
    options.adminKey,
    (0, import_utils.functionsDir)(configPath, localConfig.projectConfig),
    options.dryRun,
    options.deploymentName
  );
  const timeConfigPullStarts = performance.now();
  const remoteConfigWithModuleHashes = await (0, import_config.pullConfig)(
    ctx,
    void 0,
    void 0,
    origin,
    options.adminKey
  );
  (0, import_log.changeSpinner)("Diffing local code and deployment state...");
  const { diffString, stats } = (0, import_config.diffConfig)(
    remoteConfigWithModuleHashes,
    localConfig,
    true
  );
  if (diffString === "" && schemaState?.state === "active") {
    if (verbose) {
      const msg = localConfig.modules.length === 0 ? `No functions found in ${localConfig.projectConfig.functions}` : "Config already synced";
      (0, import_log.logMessage)(
        import_chalk.default.gray(
          `${options.dryRun ? "Command would skip function push" : "Function push skipped"}: ${msg}.`
        )
      );
    }
    return;
  }
  if (verbose) {
    (0, import_log.logMessage)(
      import_chalk.default.bold(
        `Remote config ${options.dryRun ? "would" : "will"} be overwritten with the following changes:`
      )
    );
    (0, import_log.logMessage)(diffString);
  }
  if (options.dryRun) {
    return;
  }
  const timePushStarts = performance.now();
  const timing = {
    typecheck: (timeBundleStarts - timeRunPushStarts) / 1e3,
    bundle: (timeSchemaPushStarts - timeBundleStarts) / 1e3,
    schemaPush: (timeConfigPullStarts - timeSchemaPushStarts) / 1e3,
    codePull: (timePushStarts - timeConfigPullStarts) / 1e3,
    totalBeforePush: (timePushStarts - timeRunPushStarts) / 1e3,
    moduleDiffStats: stats
  };
  await (0, import_config.pushConfig)(ctx, localConfig, {
    adminKey: options.adminKey,
    url: options.url,
    deploymentName: options.deploymentName,
    pushMetrics: timing,
    schemaId,
    bundledModuleInfos
  });
}
//# sourceMappingURL=push.js.map
