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
var config_exports = {};
__export(config_exports, {
  configFilepath: () => configFilepath,
  configFromProjectConfig: () => configFromProjectConfig,
  configJSON: () => configJSON,
  configName: () => configName,
  debugIsolateEndpointBundles: () => debugIsolateEndpointBundles,
  diffConfig: () => diffConfig,
  enforceDeprecatedConfigField: () => enforceDeprecatedConfigField,
  getFunctionsDirectoryPath: () => getFunctionsDirectoryPath,
  handlePushConfigError: () => handlePushConfigError,
  parseProjectConfig: () => parseProjectConfig,
  productionProvisionHost: () => import_utils2.productionProvisionHost,
  provisionHost: () => import_utils2.provisionHost,
  pullConfig: () => pullConfig,
  pushConfig: () => pushConfig,
  readConfig: () => readConfig,
  readProjectConfig: () => readProjectConfig,
  removedExistingConfig: () => removedExistingConfig,
  upgradeOldAuthInfoToAuthConfig: () => upgradeOldAuthInfoToAuthConfig,
  writeProjectConfig: () => writeProjectConfig
});
module.exports = __toCommonJS(config_exports);
var import_chalk = __toESM(require("chalk"), 1);
var import_deep_equal = __toESM(require("deep-equal"), 1);
var import_os = require("os");
var import_path = __toESM(require("path"), 1);
var import_log = require("../../bundler/log.js");
var import_bundler = require("../../bundler/index.js");
var import_version = require("../version.js");
var import_dashboard = require("./dashboard.js");
var import_utils = require("./utils/utils.js");
var import_crypto = require("crypto");
var import_util = require("util");
var import_zlib = __toESM(require("zlib"), 1);
var import_fsUtils = require("./fsUtils.js");
var import_errors = require("./localDeployment/errors.js");
var import_debugBundle = require("../../bundler/debugBundle.js");
var import_workos = require("./workos/workos.js");
var import_utils2 = require("./utils/utils.js");
const brotli = (0, import_util.promisify)(import_zlib.default.brotliCompress);
const DEFAULT_FUNCTIONS_PATH = "convex/";
function isAuthInfo(object) {
  return "applicationID" in object && typeof object.applicationID === "string" && "domain" in object && typeof object.domain === "string";
}
function isAuthInfos(object) {
  return Array.isArray(object) && object.every((item) => isAuthInfo(item));
}
class ParseError extends Error {
}
async function parseProjectConfig(ctx, obj) {
  if (typeof obj !== "object") {
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: "Expected `convex.json` to contain an object"
    });
  }
  if (typeof obj.node === "undefined") {
    obj.node = {
      externalPackages: []
    };
  } else {
    if (typeof obj.node.externalPackages === "undefined") {
      obj.node.externalPackages = [];
    } else if (!Array.isArray(obj.node.externalPackages) || !obj.node.externalPackages.every((item) => typeof item === "string")) {
      return await ctx.crash({
        exitCode: 1,
        errorType: "invalid filesystem data",
        printedMessage: "Expected `node.externalPackages` in `convex.json` to be an array of strings"
      });
    }
    if (typeof obj.node.nodeVersion !== "undefined" && typeof obj.node.nodeVersion !== "string") {
      return await ctx.crash({
        exitCode: 1,
        errorType: "invalid filesystem data",
        printedMessage: "Expected `node.nodeVersion` in `convex.json` to be a string"
      });
    }
  }
  if (typeof obj.generateCommonJSApi === "undefined") {
    obj.generateCommonJSApi = false;
  } else if (typeof obj.generateCommonJSApi !== "boolean") {
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: "Expected `generateCommonJSApi` in `convex.json` to be true or false"
    });
  }
  if (typeof obj.functions === "undefined") {
    obj.functions = DEFAULT_FUNCTIONS_PATH;
  } else if (typeof obj.functions !== "string") {
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: "Expected `functions` in `convex.json` to be a string"
    });
  }
  if (obj.authInfo !== void 0) {
    if (!isAuthInfos(obj.authInfo)) {
      return await ctx.crash({
        exitCode: 1,
        errorType: "invalid filesystem data",
        printedMessage: "Expected `authInfo` in `convex.json` to be type AuthInfo[]"
      });
    }
  }
  if (typeof obj.codegen === "undefined") {
    obj.codegen = {};
  }
  if (typeof obj.codegen !== "object") {
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: "Expected `codegen` in `convex.json` to be an object"
    });
  }
  if (typeof obj.codegen.staticApi === "undefined") {
    obj.codegen.staticApi = false;
  }
  if (typeof obj.codegen.staticDataModel === "undefined") {
    obj.codegen.staticDataModel = false;
  }
  if (typeof obj.codegen.staticApi !== "boolean" || typeof obj.codegen.staticDataModel !== "boolean") {
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: "Expected `codegen.staticApi` and `codegen.staticDataModel` in `convex.json` to be booleans"
    });
  }
  return obj;
}
function parseBackendConfig(obj) {
  function throwParseError(message) {
    throw new ParseError(message);
  }
  if (typeof obj !== "object") {
    throwParseError("Expected an object");
  }
  const { functions, authInfo, nodeVersion } = obj;
  if (typeof functions !== "string") {
    throwParseError("Expected functions to be a string");
  }
  if ((authInfo ?? null) !== null && !isAuthInfos(authInfo)) {
    throwParseError("Expected authInfo to be type AuthInfo[]");
  }
  if (typeof nodeVersion !== "undefined" && typeof nodeVersion !== "string") {
    throwParseError("Expected nodeVersion to be a string");
  }
  return {
    functions,
    ...(authInfo ?? null) !== null ? { authInfo } : {},
    ...(nodeVersion ?? null) !== null ? { nodeVersion } : {}
  };
}
function configName() {
  return "convex.json";
}
async function configFilepath(ctx) {
  const configFn = configName();
  const preferredLocation = configFn;
  const wrongLocation = import_path.default.join("src", configFn);
  const preferredLocationExists = ctx.fs.exists(preferredLocation);
  const wrongLocationExists = ctx.fs.exists(wrongLocation);
  if (preferredLocationExists && wrongLocationExists) {
    const message = `${import_chalk.default.red(`Error: both ${preferredLocation} and ${wrongLocation} files exist!`)}
Consolidate these and remove ${wrongLocation}.`;
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: message
    });
  }
  if (!preferredLocationExists && wrongLocationExists) {
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      printedMessage: `Error: Please move ${wrongLocation} to the root of your project`
    });
  }
  return preferredLocation;
}
async function getFunctionsDirectoryPath(ctx) {
  const { projectConfig, configPath } = await readProjectConfig(ctx);
  return (0, import_utils.functionsDir)(configPath, projectConfig);
}
async function readProjectConfig(ctx) {
  if (!ctx.fs.exists("convex.json")) {
    const packages = await (0, import_utils.loadPackageJson)(ctx);
    const isCreateReactApp = "react-scripts" in packages;
    return {
      projectConfig: {
        functions: isCreateReactApp ? `src/${DEFAULT_FUNCTIONS_PATH}` : DEFAULT_FUNCTIONS_PATH,
        node: {
          externalPackages: []
        },
        generateCommonJSApi: false,
        codegen: {
          staticApi: false,
          staticDataModel: false
        }
      },
      configPath: configName()
    };
  }
  let projectConfig;
  const configPath = await configFilepath(ctx);
  try {
    projectConfig = await parseProjectConfig(
      ctx,
      JSON.parse(ctx.fs.readUtf8File(configPath))
    );
  } catch (err) {
    if (err instanceof ParseError || err instanceof SyntaxError) {
      (0, import_log.logError)(import_chalk.default.red(`Error: Parsing "${configPath}" failed`));
      (0, import_log.logMessage)(import_chalk.default.gray(err.toString()));
    } else {
      (0, import_log.logFailure)(
        `Error: Unable to read project config file "${configPath}"
  Are you running this command from the root directory of a Convex project? If so, run \`npx convex dev\` first.`
      );
      if (err instanceof Error) {
        (0, import_log.logError)(import_chalk.default.red(err.message));
      }
    }
    return await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem data",
      errForSentry: err,
      // TODO -- move the logging above in here
      printedMessage: null
    });
  }
  return {
    projectConfig,
    configPath
  };
}
async function enforceDeprecatedConfigField(ctx, config, field) {
  const value = config[field];
  if (typeof value === "string") {
    return value;
  }
  const err = new ParseError(`Expected ${field} to be a string`);
  return await ctx.crash({
    exitCode: 1,
    errorType: "invalid filesystem data",
    errForSentry: err,
    printedMessage: `Error: Parsing convex.json failed:
${import_chalk.default.gray(err.toString())}`
  });
}
async function configFromProjectConfig(ctx, projectConfig, configPath, verbose) {
  const baseDir = (0, import_utils.functionsDir)(configPath, projectConfig);
  const entryPoints = await (0, import_bundler.entryPointsByEnvironment)(ctx, baseDir);
  if (verbose) {
    (0, import_log.showSpinner)("Bundling modules for Convex's runtime...");
  }
  const convexResult = await (0, import_bundler.bundle)(
    ctx,
    baseDir,
    entryPoints.isolate,
    true,
    "browser"
  );
  if (verbose) {
    (0, import_log.logMessage)(
      "Convex's runtime modules: ",
      convexResult.modules.map((m) => m.path)
    );
  }
  if (verbose && entryPoints.node.length !== 0) {
    (0, import_log.showSpinner)("Bundling modules for Node.js runtime...");
  }
  const nodeResult = await (0, import_bundler.bundle)(
    ctx,
    baseDir,
    entryPoints.node,
    true,
    "node",
    import_path.default.join("_deps", "node"),
    projectConfig.node.externalPackages
  );
  if (verbose && entryPoints.node.length !== 0) {
    (0, import_log.logMessage)(
      "Node.js runtime modules: ",
      nodeResult.modules.map((m) => m.path)
    );
    if (projectConfig.node.externalPackages.length > 0) {
      (0, import_log.logMessage)(
        "Node.js runtime external dependencies (to be installed on the server): ",
        [...nodeResult.externalDependencies.entries()].map(
          (a) => `${a[0]}: ${a[1]}`
        )
      );
    }
  }
  const modules = convexResult.modules;
  modules.push(...nodeResult.modules);
  modules.push(...await (0, import_bundler.bundleAuthConfig)(ctx, baseDir));
  const nodeDependencies = [];
  for (const [moduleName, moduleVersion] of nodeResult.externalDependencies) {
    nodeDependencies.push({ name: moduleName, version: moduleVersion });
  }
  const bundledModuleInfos = Array.from(
    convexResult.bundledModuleNames.keys()
  ).map((moduleName) => {
    return {
      name: moduleName,
      platform: "convex"
    };
  });
  bundledModuleInfos.push(
    ...Array.from(nodeResult.bundledModuleNames.keys()).map(
      (moduleName) => {
        return {
          name: moduleName,
          platform: "node"
        };
      }
    )
  );
  return {
    config: {
      projectConfig,
      modules,
      nodeDependencies,
      // We're just using the version this CLI is running with for now.
      // This could be different than the version of `convex` the app runs with
      // if the CLI is installed globally.
      udfServerVersion: import_version.version,
      nodeVersion: projectConfig.node.nodeVersion
    },
    bundledModuleInfos
  };
}
async function debugIsolateEndpointBundles(ctx, projectConfig, configPath) {
  const baseDir = (0, import_utils.functionsDir)(configPath, projectConfig);
  const entryPoints = await (0, import_bundler.entryPointsByEnvironment)(ctx, baseDir);
  if (entryPoints.isolate.length === 0) {
    (0, import_log.logFinishedStep)("No non-'use node' modules found.");
  }
  await (0, import_debugBundle.debugIsolateBundlesSerially)(ctx, {
    entryPoints: entryPoints.isolate,
    extraConditions: [],
    dir: baseDir
  });
}
async function readConfig(ctx, verbose) {
  const { projectConfig, configPath } = await readProjectConfig(ctx);
  const { config, bundledModuleInfos } = await configFromProjectConfig(
    ctx,
    projectConfig,
    configPath,
    verbose
  );
  return { config, configPath, bundledModuleInfos };
}
async function upgradeOldAuthInfoToAuthConfig(ctx, config, functionsPath) {
  if (config.authInfo !== void 0) {
    const authConfigPathJS = import_path.default.resolve(functionsPath, "auth.config.js");
    const authConfigPathTS = import_path.default.resolve(functionsPath, "auth.config.js");
    const authConfigPath = ctx.fs.exists(authConfigPathJS) ? authConfigPathJS : authConfigPathTS;
    const authConfigRelativePath = import_path.default.join(
      config.functions,
      ctx.fs.exists(authConfigPathJS) ? "auth.config.js" : "auth.config.ts"
    );
    if (ctx.fs.exists(authConfigPath)) {
      await ctx.crash({
        exitCode: 1,
        errorType: "invalid filesystem data",
        printedMessage: `Cannot set auth config in both \`${authConfigRelativePath}\` and convex.json, remove it from convex.json`
      });
    }
    if (config.authInfo.length > 0) {
      const providersStringLines = JSON.stringify(
        config.authInfo,
        null,
        2
      ).split(import_os.EOL);
      const indentedProvidersString = [providersStringLines[0]].concat(providersStringLines.slice(1).map((line) => `  ${line}`)).join(import_os.EOL);
      ctx.fs.writeUtf8File(
        authConfigPath,
        `  export default {
    providers: ${indentedProvidersString},
  };`
      );
      (0, import_log.logMessage)(
        import_chalk.default.yellowBright(
          `Moved auth config from config.json to \`${authConfigRelativePath}\``
        )
      );
    }
    delete config.authInfo;
  }
  return config;
}
async function writeProjectConfig(ctx, projectConfig, { deleteIfAllDefault } = {
  deleteIfAllDefault: false
}) {
  const configPath = await configFilepath(ctx);
  const strippedConfig = filterWriteableConfig(stripDefaults(projectConfig));
  if (Object.keys(strippedConfig).length > 0) {
    try {
      const contents = JSON.stringify(strippedConfig, void 0, 2) + "\n";
      ctx.fs.writeUtf8File(configPath, contents, 420);
    } catch (err) {
      return await ctx.crash({
        exitCode: 1,
        errorType: "invalid filesystem data",
        errForSentry: err,
        printedMessage: `Error: Unable to write project config file "${configPath}" in current directory
  Are you running this command from the root directory of a Convex project?`
      });
    }
  } else if (deleteIfAllDefault && ctx.fs.exists(configPath)) {
    ctx.fs.unlink(configPath);
    (0, import_log.logMessage)(
      import_chalk.default.yellowBright(
        `Deleted ${configPath} since it completely matched defaults`
      )
    );
  }
  ctx.fs.mkdir((0, import_utils.functionsDir)(configPath, projectConfig), {
    allowExisting: true
  });
}
function stripDefaults(projectConfig) {
  const stripped = { ...projectConfig };
  if (stripped.functions === DEFAULT_FUNCTIONS_PATH) {
    delete stripped.functions;
  }
  if (Array.isArray(stripped.authInfo) && stripped.authInfo.length === 0) {
    delete stripped.authInfo;
  }
  if (stripped.node.externalPackages.length === 0) {
    delete stripped.node.externalPackages;
  }
  if (stripped.generateCommonJSApi === false) {
    delete stripped.generateCommonJSApi;
  }
  if (Object.keys(stripped.node).length === 0) {
    delete stripped.node;
  }
  if (stripped.codegen.staticApi === false) {
    delete stripped.codegen.staticApi;
  }
  if (stripped.codegen.staticDataModel === false) {
    delete stripped.codegen.staticDataModel;
  }
  if (Object.keys(stripped.codegen).length === 0) {
    delete stripped.codegen;
  }
  return stripped;
}
function filterWriteableConfig(projectConfig) {
  const writeable = { ...projectConfig };
  delete writeable.project;
  delete writeable.team;
  delete writeable.prodUrl;
  return writeable;
}
function removedExistingConfig(ctx, configPath, options) {
  if (!options.allowExistingConfig) {
    return false;
  }
  (0, import_fsUtils.recursivelyDelete)(ctx, configPath);
  (0, import_log.logFinishedStep)(`Removed existing ${configPath}`);
  return true;
}
async function pullConfig(ctx, project, team, origin, adminKey) {
  const fetch = (0, import_utils.deploymentFetch)(ctx, {
    deploymentUrl: origin,
    adminKey
  });
  (0, import_log.changeSpinner)("Downloading current deployment state...");
  try {
    const res = await fetch("/api/get_config_hashes", {
      method: "POST",
      body: JSON.stringify({ version: import_version.version, adminKey })
    });
    (0, import_utils.deprecationCheckWarning)(ctx, res);
    const data = await res.json();
    const backendConfig = parseBackendConfig(data.config);
    const projectConfig = {
      ...backendConfig,
      node: {
        // This field is not stored in the backend, which is ok since it is also
        // not used to diff configs.
        externalPackages: [],
        nodeVersion: data.nodeVersion
      },
      // This field is not stored in the backend, it only affects the client.
      generateCommonJSApi: false,
      // This field is also not stored in the backend, it only affects the client.
      codegen: {
        staticApi: false,
        staticDataModel: false
      },
      project,
      team,
      prodUrl: origin
    };
    return {
      projectConfig,
      moduleHashes: data.moduleHashes,
      // TODO(presley): Add this to diffConfig().
      nodeDependencies: data.nodeDependencies,
      udfServerVersion: data.udfServerVersion
    };
  } catch (err) {
    (0, import_log.logFailure)(`Error: Unable to pull deployment config from ${origin}`);
    return await (0, import_utils.logAndHandleFetchError)(ctx, err);
  }
}
function configJSON(config, adminKey, schemaId, pushMetrics, bundledModuleInfos) {
  const projectConfig = {
    projectSlug: config.projectConfig.project,
    teamSlug: config.projectConfig.team,
    functions: config.projectConfig.functions,
    authInfo: config.projectConfig.authInfo
  };
  return {
    config: projectConfig,
    modules: config.modules,
    nodeDependencies: config.nodeDependencies,
    udfServerVersion: config.udfServerVersion,
    schemaId,
    adminKey,
    pushMetrics,
    bundledModuleInfos,
    nodeVersion: config.nodeVersion
  };
}
async function pushConfig(ctx, config, options) {
  const serializedConfig = configJSON(
    config,
    options.adminKey,
    options.schemaId,
    options.pushMetrics,
    options.bundledModuleInfos
  );
  const fetch = (0, import_utils.deploymentFetch)(ctx, {
    deploymentUrl: options.url,
    adminKey: options.adminKey
  });
  try {
    if (config.nodeDependencies.length > 0) {
      (0, import_log.changeSpinner)(
        "Installing external packages and deploying source code..."
      );
    } else {
      (0, import_log.changeSpinner)("Analyzing and deploying source code...");
    }
    await fetch("/api/push_config", {
      body: await brotli(JSON.stringify(serializedConfig), {
        params: {
          [import_zlib.default.constants.BROTLI_PARAM_MODE]: import_zlib.default.constants.BROTLI_MODE_TEXT,
          [import_zlib.default.constants.BROTLI_PARAM_QUALITY]: 4
        }
      }),
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Content-Encoding": "br"
      }
    });
  } catch (error) {
    await handlePushConfigError(
      ctx,
      error,
      "Error: Unable to push deployment config to " + options.url,
      options.deploymentName,
      {
        adminKey: options.adminKey,
        deploymentUrl: options.url,
        deploymentNotice: ""
      }
    );
  }
}
function renderModule(module2) {
  return module2.path + ` (${(0, import_utils.formatSize)(module2.sourceSize)}, source map ${module2.sourceMapSize})`;
}
function hash(bundle2) {
  return (0, import_crypto.createHash)("sha256").update(bundle2.source).update(bundle2.sourceMap || "").digest("hex");
}
function compareModules(oldModules, newModules) {
  let diff = "";
  const oldModuleMap = new Map(
    oldModules.map((value) => [value.path, value.hash])
  );
  const newModuleMap = new Map(
    newModules.map((value) => [
      value.path,
      {
        hash: hash(value),
        sourceMapSize: value.sourceMap?.length ?? 0,
        sourceSize: value.source.length
      }
    ])
  );
  const updatedModules = [];
  const identicalModules = [];
  const droppedModules = [];
  const addedModules = [];
  for (const [path2, oldHash] of oldModuleMap.entries()) {
    const newModule = newModuleMap.get(path2);
    if (newModule === void 0) {
      droppedModules.push(path2);
    } else if (newModule.hash !== oldHash) {
      updatedModules.push({
        path: path2,
        sourceMapSize: newModule.sourceMapSize,
        sourceSize: newModule.sourceSize
      });
    } else {
      identicalModules.push({
        path: path2,
        size: newModule.sourceSize + newModule.sourceMapSize
      });
    }
  }
  for (const [path2, newModule] of newModuleMap.entries()) {
    if (oldModuleMap.get(path2) === void 0) {
      addedModules.push({
        path: path2,
        sourceMapSize: newModule.sourceMapSize,
        sourceSize: newModule.sourceSize
      });
    }
  }
  if (droppedModules.length > 0 || updatedModules.length > 0) {
    diff += "Delete the following modules:\n";
    for (const module2 of droppedModules) {
      diff += `[-] ${module2}
`;
    }
    for (const module2 of updatedModules) {
      diff += `[-] ${module2.path}
`;
    }
  }
  if (addedModules.length > 0 || updatedModules.length > 0) {
    diff += "Add the following modules:\n";
    for (const module2 of addedModules) {
      diff += "[+] " + renderModule(module2) + "\n";
    }
    for (const module2 of updatedModules) {
      diff += "[+] " + renderModule(module2) + "\n";
    }
  }
  return {
    diffString: diff,
    stats: {
      updated: {
        count: updatedModules.length,
        size: updatedModules.reduce((acc, curr) => {
          return acc + curr.sourceMapSize + curr.sourceSize;
        }, 0)
      },
      identical: {
        count: identicalModules.length,
        size: identicalModules.reduce((acc, curr) => {
          return acc + curr.size;
        }, 0)
      },
      added: {
        count: addedModules.length,
        size: addedModules.reduce((acc, curr) => {
          return acc + curr.sourceMapSize + curr.sourceSize;
        }, 0)
      },
      numDropped: droppedModules.length
    }
  };
}
function diffConfig(oldConfig, newConfig, shouldDiffModules) {
  let diff = "";
  let stats;
  if (shouldDiffModules) {
    const { diffString, stats: moduleStats } = compareModules(
      oldConfig.moduleHashes,
      newConfig.modules
    );
    diff = diffString;
    stats = moduleStats;
  }
  const droppedAuth = [];
  if (oldConfig.projectConfig.authInfo !== void 0 && newConfig.projectConfig.authInfo !== void 0) {
    for (const oldAuth of oldConfig.projectConfig.authInfo) {
      let matches2 = false;
      for (const newAuth of newConfig.projectConfig.authInfo) {
        if ((0, import_deep_equal.default)(oldAuth, newAuth)) {
          matches2 = true;
          break;
        }
      }
      if (!matches2) {
        droppedAuth.push(oldAuth);
      }
    }
    if (droppedAuth.length > 0) {
      diff += "Remove the following auth providers:\n";
      for (const authInfo of droppedAuth) {
        diff += "[-] " + JSON.stringify(authInfo) + "\n";
      }
    }
    const addedAuth = [];
    for (const newAuth of newConfig.projectConfig.authInfo) {
      let matches2 = false;
      for (const oldAuth of oldConfig.projectConfig.authInfo) {
        if ((0, import_deep_equal.default)(newAuth, oldAuth)) {
          matches2 = true;
          break;
        }
      }
      if (!matches2) {
        addedAuth.push(newAuth);
      }
    }
    if (addedAuth.length > 0) {
      diff += "Add the following auth providers:\n";
      for (const auth of addedAuth) {
        diff += "[+] " + JSON.stringify(auth) + "\n";
      }
    }
  } else if (oldConfig.projectConfig.authInfo !== void 0 !== (newConfig.projectConfig.authInfo !== void 0)) {
    diff += "Moved auth config into auth.config.ts\n";
  }
  let versionMessage = "";
  const matches = oldConfig.udfServerVersion === newConfig.udfServerVersion;
  if (oldConfig.udfServerVersion && (!newConfig.udfServerVersion || !matches)) {
    versionMessage += `[-] ${oldConfig.udfServerVersion}
`;
  }
  if (newConfig.udfServerVersion && (!oldConfig.udfServerVersion || !matches)) {
    versionMessage += `[+] ${newConfig.udfServerVersion}
`;
  }
  if (versionMessage) {
    diff += "Change the server's function version:\n";
    diff += versionMessage;
  }
  if (oldConfig.projectConfig.node.nodeVersion !== newConfig.nodeVersion) {
    diff += "Change the server's version for Node.js actions:\n";
    if (oldConfig.projectConfig.node.nodeVersion) {
      diff += `[-] ${oldConfig.projectConfig.node.nodeVersion}
`;
    }
    if (newConfig.nodeVersion) {
      diff += `[+] ${newConfig.nodeVersion}
`;
    }
  }
  return { diffString: diff, stats };
}
async function handlePushConfigError(ctx, error, defaultMessage, deploymentName, deployment) {
  const data = error instanceof import_utils.ThrowingFetchError ? error.serverErrorData : void 0;
  if (data?.code === "AuthConfigMissingEnvironmentVariable") {
    const errorMessage = data.message || "(no error message given)";
    const [, variableName] = errorMessage.match(/Environment variable (\S+)/i) ?? [];
    if (variableName === "WORKOS_CLIENT_ID" && deploymentName && deployment) {
      const homepage = await (0, import_utils.currentPackageHomepage)(ctx);
      const autoProvisionIfWorkOSTeamAssociated = !!(homepage && [
        "https://github.com/workos/template-convex-nextjs-authkit/#readme",
        "https://github.com/workos/template-convex-react-vite-authkit/#readme",
        "https://github.com:workos/template-convex-react-vite-authkit/#readme"
      ].includes(homepage));
      const offerToAssociateWorkOSTeam = autoProvisionIfWorkOSTeamAssociated;
      const autoConfigureAuthkitConfig = autoProvisionIfWorkOSTeamAssociated;
      const result = await (0, import_workos.ensureWorkosEnvironmentProvisioned)(
        ctx,
        deploymentName,
        deployment,
        {
          offerToAssociateWorkOSTeam,
          autoProvisionIfWorkOSTeamAssociated,
          autoConfigureAuthkitConfig
        }
      );
      if (result === "ready") {
        return await ctx.crash({
          exitCode: 1,
          errorType: "already handled",
          printedMessage: null
        });
      }
    }
    const envVarMessage = `Environment variable ${import_chalk.default.bold(
      variableName
    )} is used in auth config file but its value was not set.`;
    let setEnvVarInstructions = "Go set it in the dashboard or using `npx convex env set`";
    if (deploymentName !== null) {
      const variableQuery = variableName !== void 0 ? `?var=${variableName}` : "";
      const dashboardUrl = (0, import_dashboard.deploymentDashboardUrlPage)(
        deploymentName,
        `/settings/environment-variables${variableQuery}`
      );
      setEnvVarInstructions = `Go to:

    ${import_chalk.default.bold(
        dashboardUrl
      )}

  to set it up. `;
    }
    await ctx.crash({
      exitCode: 1,
      errorType: "invalid filesystem or env vars",
      errForSentry: error,
      printedMessage: envVarMessage + "\n" + setEnvVarInstructions
    });
  }
  if (data?.code === "InternalServerError") {
    if (deploymentName?.startsWith("local-")) {
      (0, import_errors.printLocalDeploymentOnError)();
      return ctx.crash({
        exitCode: 1,
        errorType: "fatal",
        errForSentry: new import_errors.LocalDeploymentError(
          "InternalServerError while pushing to local deployment"
        ),
        printedMessage: defaultMessage
      });
    }
  }
  (0, import_log.logFailure)(defaultMessage);
  return await (0, import_utils.logAndHandleFetchError)(ctx, error);
}
//# sourceMappingURL=config.js.map
