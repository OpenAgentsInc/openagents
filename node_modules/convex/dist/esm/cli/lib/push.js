"use strict";
import chalk from "chalk";
import {
  changeSpinner,
  logFinishedStep,
  logMessage
} from "../../bundler/log.js";
import { doCodegen } from "./codegen.js";
import {
  configFromProjectConfig,
  diffConfig,
  debugIsolateEndpointBundles,
  pullConfig,
  pushConfig
} from "./config.js";
import { pushSchema } from "./indexes.js";
import { typeCheckFunctionsInMode } from "./typecheck.js";
import { ensureHasConvexDependency, functionsDir } from "./utils/utils.js";
import { handleDebugBundlePath } from "./debugBundlePath.js";
export async function runNonComponentsPush(ctx, options, configPath, projectConfig) {
  if (options.writePushRequest) {
    logMessage(
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
  await ensureHasConvexDependency(ctx, "push");
  if (!options.codegen) {
    logMessage(
      chalk.gray("Skipping codegen. Remove --codegen=disable to enable.")
    );
    const funcDir = functionsDir(configPath, projectConfig);
    await typeCheckFunctionsInMode(ctx, options.typecheck, funcDir);
  } else {
    await doCodegen(
      ctx,
      functionsDir(configPath, projectConfig),
      options.typecheck,
      options
    );
    if (verbose) {
      logMessage(chalk.green("Codegen finished."));
    }
  }
  if (options.debugNodeApis) {
    await debugIsolateEndpointBundles(ctx, projectConfig, configPath);
    logFinishedStep(
      "All non-'use node' entry points successfully bundled. Skipping rest of push."
    );
    return;
  }
  const timeBundleStarts = performance.now();
  const { config: localConfig, bundledModuleInfos } = await configFromProjectConfig(ctx, projectConfig, configPath, verbose);
  if (options.debugBundlePath) {
    await handleDebugBundlePath(ctx, options.debugBundlePath, localConfig);
    logFinishedStep(
      `Wrote bundle and metadata to ${options.debugBundlePath}. Skipping rest of push.`
    );
    return;
  }
  const timeSchemaPushStarts = performance.now();
  const { schemaId, schemaState } = await pushSchema(
    ctx,
    origin,
    options.adminKey,
    functionsDir(configPath, localConfig.projectConfig),
    options.dryRun,
    options.deploymentName
  );
  const timeConfigPullStarts = performance.now();
  const remoteConfigWithModuleHashes = await pullConfig(
    ctx,
    void 0,
    void 0,
    origin,
    options.adminKey
  );
  changeSpinner("Diffing local code and deployment state...");
  const { diffString, stats } = diffConfig(
    remoteConfigWithModuleHashes,
    localConfig,
    true
  );
  if (diffString === "" && schemaState?.state === "active") {
    if (verbose) {
      const msg = localConfig.modules.length === 0 ? `No functions found in ${localConfig.projectConfig.functions}` : "Config already synced";
      logMessage(
        chalk.gray(
          `${options.dryRun ? "Command would skip function push" : "Function push skipped"}: ${msg}.`
        )
      );
    }
    return;
  }
  if (verbose) {
    logMessage(
      chalk.bold(
        `Remote config ${options.dryRun ? "would" : "will"} be overwritten with the following changes:`
      )
    );
    logMessage(diffString);
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
  await pushConfig(ctx, localConfig, {
    adminKey: options.adminKey,
    url: options.url,
    deploymentName: options.deploymentName,
    pushMetrics: timing,
    schemaId,
    bundledModuleInfos
  });
}
//# sourceMappingURL=push.js.map
