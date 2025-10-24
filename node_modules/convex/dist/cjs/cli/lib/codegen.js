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
var codegen_exports = {};
__export(codegen_exports, {
  doCodegen: () => doCodegen,
  doCodegenForNewProject: () => doCodegenForNewProject,
  doFinalComponentCodegen: () => doFinalComponentCodegen,
  doInitCodegen: () => doInitCodegen,
  doInitialComponentCodegen: () => doInitialComponentCodegen
});
module.exports = __toCommonJS(codegen_exports);
var import_path = __toESM(require("path"), 1);
var import_prettier = __toESM(require("prettier"), 1);
var import_fs = require("../../bundler/fs.js");
var import_bundler = require("../../bundler/index.js");
var import_api = require("../codegen_templates/api.js");
var import_api_cjs = require("../codegen_templates/api_cjs.js");
var import_dataModel = require("../codegen_templates/dataModel.js");
var import_readme = require("../codegen_templates/readme.js");
var import_server = require("../codegen_templates/server.js");
var import_tsconfig = require("../codegen_templates/tsconfig.js");
var import_log = require("../../bundler/log.js");
var import_typecheck = require("./typecheck.js");
var import_config = require("./config.js");
var import_fsUtils = require("./fsUtils.js");
var import_component_server = require("../codegen_templates/component_server.js");
var import_component_api = require("../codegen_templates/component_api.js");
var import_utils = require("./utils/utils.js");
async function doCodegenForNewProject(ctx) {
  const { projectConfig: existingProjectConfig } = await (0, import_config.readProjectConfig)(ctx);
  const configPath = await (0, import_config.configFilepath)(ctx);
  const functionsPath = (0, import_utils.functionsDir)(configPath, existingProjectConfig);
  await doInitCodegen(ctx, functionsPath, true);
  await doCodegen(ctx, functionsPath, "disable");
}
async function doInitCodegen(ctx, functionsDir2, skipIfExists, opts) {
  await prepareForCodegen(ctx, functionsDir2, opts);
  await (0, import_fs.withTmpDir)(async (tmpDir) => {
    await doReadmeCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts);
    await doTsconfigCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts);
  });
}
async function prepareForCodegen(ctx, functionsDir2, opts) {
  const legacyCodegenPath = import_path.default.join(functionsDir2, "_generated.ts");
  if (ctx.fs.exists(legacyCodegenPath)) {
    if (opts?.dryRun) {
      (0, import_log.logError)(
        `Command would delete legacy codegen file: ${legacyCodegenPath}}`
      );
    } else {
      (0, import_log.logError)(`Deleting legacy codegen file: ${legacyCodegenPath}}`);
      ctx.fs.unlink(legacyCodegenPath);
    }
  }
  const codegenDir = import_path.default.join(functionsDir2, "_generated");
  ctx.fs.mkdir(codegenDir, { allowExisting: true, recursive: true });
  return codegenDir;
}
async function doCodegen(ctx, functionsDir2, typeCheckMode, opts) {
  const { projectConfig } = await (0, import_config.readProjectConfig)(ctx);
  const codegenDir = await prepareForCodegen(ctx, functionsDir2, opts);
  await (0, import_fs.withTmpDir)(async (tmpDir) => {
    const writtenFiles = [];
    const schemaFiles = await doDataModelCodegen(
      ctx,
      tmpDir,
      functionsDir2,
      codegenDir,
      opts
    );
    writtenFiles.push(...schemaFiles);
    const serverFiles = await doServerCodegen(ctx, tmpDir, codegenDir, opts);
    writtenFiles.push(...serverFiles);
    const apiFiles = await doApiCodegen(
      ctx,
      tmpDir,
      functionsDir2,
      codegenDir,
      opts?.generateCommonJSApi || projectConfig.generateCommonJSApi,
      opts
    );
    writtenFiles.push(...apiFiles);
    for (const file of ctx.fs.listDir(codegenDir)) {
      if (!writtenFiles.includes(file.name)) {
        (0, import_fsUtils.recursivelyDelete)(ctx, import_path.default.join(codegenDir, file.name), opts);
      }
    }
    await (0, import_typecheck.typeCheckFunctionsInMode)(ctx, typeCheckMode, functionsDir2);
  });
}
async function doInitialComponentCodegen(ctx, tmpDir, componentDirectory, opts) {
  const { projectConfig } = await (0, import_config.readProjectConfig)(ctx);
  const isPublishedPackage = componentDirectory.definitionPath.endsWith(".js") && !componentDirectory.isRoot;
  if (isPublishedPackage) {
    if (opts?.verbose) {
      (0, import_log.logMessage)(
        `skipping initial codegen for installed package ${componentDirectory.path}`
      );
    }
    return;
  }
  const codegenDir = await prepareForCodegen(
    ctx,
    componentDirectory.path,
    opts
  );
  const writtenFiles = [];
  const dataModelFiles = await doInitialComponentDataModelCodegen(
    ctx,
    tmpDir,
    componentDirectory,
    codegenDir,
    opts
  );
  writtenFiles.push(...dataModelFiles);
  const serverFiles = await doInitialComponentServerCodegen(
    ctx,
    componentDirectory.isRoot,
    tmpDir,
    codegenDir,
    opts
  );
  writtenFiles.push(...serverFiles);
  const apiFiles = await doInitialComponentApiCodegen(
    ctx,
    componentDirectory.isRoot,
    tmpDir,
    codegenDir,
    opts?.generateCommonJSApi || projectConfig.generateCommonJSApi,
    opts
  );
  writtenFiles.push(...apiFiles);
  for (const file of ctx.fs.listDir(codegenDir)) {
    if (!writtenFiles.includes(file.name)) {
      (0, import_fsUtils.recursivelyDelete)(ctx, import_path.default.join(codegenDir, file.name), opts);
    }
  }
}
async function doFinalComponentCodegen(ctx, tmpDir, rootComponent, componentDirectory, startPushResponse, opts) {
  const { projectConfig } = await (0, import_config.readProjectConfig)(ctx);
  const isPublishedPackage = componentDirectory.definitionPath.endsWith(".js") && !componentDirectory.isRoot;
  if (isPublishedPackage) {
    return;
  }
  const codegenDir = import_path.default.join(componentDirectory.path, "_generated");
  ctx.fs.mkdir(codegenDir, { allowExisting: true, recursive: true });
  const hasSchemaFile = schemaFileExists(ctx, componentDirectory.path);
  let dataModelContents;
  if (hasSchemaFile) {
    if (projectConfig.codegen.staticDataModel) {
      dataModelContents = await (0, import_dataModel.staticDataModelDTS)(
        ctx,
        startPushResponse,
        rootComponent,
        componentDirectory
      );
    } else {
      dataModelContents = (0, import_dataModel.dynamicDataModelDTS)();
    }
  } else {
    dataModelContents = (0, import_dataModel.noSchemaDataModelDTS)();
  }
  const dataModelDTSPath = import_path.default.join(codegenDir, "dataModel.d.ts");
  await writeFormattedFile(
    ctx,
    tmpDir,
    dataModelContents,
    "typescript",
    dataModelDTSPath,
    opts
  );
  const serverDTSPath = import_path.default.join(codegenDir, "server.d.ts");
  const serverContents = await (0, import_component_server.componentServerDTS)(componentDirectory);
  await writeFormattedFile(
    ctx,
    tmpDir,
    serverContents,
    "typescript",
    serverDTSPath,
    opts
  );
  const apiDTSPath = import_path.default.join(codegenDir, "api.d.ts");
  const apiContents = await (0, import_component_api.componentApiDTS)(
    ctx,
    startPushResponse,
    rootComponent,
    componentDirectory,
    { staticApi: projectConfig.codegen.staticApi }
  );
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiContents,
    "typescript",
    apiDTSPath,
    opts
  );
  if (opts?.generateCommonJSApi || projectConfig.generateCommonJSApi) {
    const apiCjsDTSPath = import_path.default.join(codegenDir, "api_cjs.d.cts");
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiContents,
      "typescript",
      apiCjsDTSPath,
      opts
    );
  }
}
async function doReadmeCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts) {
  const readmePath = import_path.default.join(functionsDir2, "README.md");
  if (skipIfExists && ctx.fs.exists(readmePath)) {
    (0, import_log.logVerbose)(`Not overwriting README.md.`);
    return;
  }
  await writeFormattedFile(
    ctx,
    tmpDir,
    (0, import_readme.readmeCodegen)(),
    "markdown",
    readmePath,
    opts
  );
}
async function doTsconfigCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts) {
  const tsconfigPath = import_path.default.join(functionsDir2, "tsconfig.json");
  if (skipIfExists && ctx.fs.exists(tsconfigPath)) {
    (0, import_log.logVerbose)(`Not overwriting tsconfig.json.`);
    return;
  }
  await writeFormattedFile(
    ctx,
    tmpDir,
    (0, import_tsconfig.tsconfigCodegen)(),
    "json",
    tsconfigPath,
    opts
  );
}
function schemaFileExists(ctx, functionsDir2) {
  let schemaPath = import_path.default.join(functionsDir2, "schema.ts");
  let hasSchemaFile = ctx.fs.exists(schemaPath);
  if (!hasSchemaFile) {
    schemaPath = import_path.default.join(functionsDir2, "schema.js");
    hasSchemaFile = ctx.fs.exists(schemaPath);
  }
  return hasSchemaFile;
}
async function doDataModelCodegen(ctx, tmpDir, functionsDir2, codegenDir, opts) {
  const hasSchemaFile = schemaFileExists(ctx, functionsDir2);
  const schemaContent = hasSchemaFile ? (0, import_dataModel.dynamicDataModelDTS)() : (0, import_dataModel.noSchemaDataModelDTS)();
  await writeFormattedFile(
    ctx,
    tmpDir,
    schemaContent,
    "typescript",
    import_path.default.join(codegenDir, "dataModel.d.ts"),
    opts
  );
  return ["dataModel.d.ts"];
}
async function doServerCodegen(ctx, tmpDir, codegenDir, opts) {
  const serverContent = (0, import_server.serverCodegen)();
  await writeFormattedFile(
    ctx,
    tmpDir,
    serverContent.JS,
    "typescript",
    import_path.default.join(codegenDir, "server.js"),
    opts
  );
  await writeFormattedFile(
    ctx,
    tmpDir,
    serverContent.DTS,
    "typescript",
    import_path.default.join(codegenDir, "server.d.ts"),
    opts
  );
  return ["server.js", "server.d.ts"];
}
async function doInitialComponentServerCodegen(ctx, isRoot, tmpDir, codegenDir, opts) {
  await writeFormattedFile(
    ctx,
    tmpDir,
    (0, import_component_server.componentServerJS)(),
    "typescript",
    import_path.default.join(codegenDir, "server.js"),
    opts
  );
  const serverDTSPath = import_path.default.join(codegenDir, "server.d.ts");
  if (!ctx.fs.exists(serverDTSPath)) {
    await writeFormattedFile(
      ctx,
      tmpDir,
      (0, import_component_server.componentServerStubDTS)(isRoot),
      "typescript",
      import_path.default.join(codegenDir, "server.d.ts"),
      opts
    );
  }
  return ["server.js", "server.d.ts"];
}
async function doInitialComponentDataModelCodegen(ctx, tmpDir, componentDirectory, codegenDir, opts) {
  const hasSchemaFile = schemaFileExists(ctx, componentDirectory.path);
  const dataModelContext = hasSchemaFile ? (0, import_dataModel.dynamicDataModelDTS)() : (0, import_dataModel.noSchemaDataModelDTS)();
  const dataModelPath = import_path.default.join(codegenDir, "dataModel.d.ts");
  if (!ctx.fs.exists(dataModelPath)) {
    await writeFormattedFile(
      ctx,
      tmpDir,
      dataModelContext,
      "typescript",
      dataModelPath,
      opts
    );
  }
  return ["dataModel.d.ts"];
}
async function doInitialComponentApiCodegen(ctx, isRoot, tmpDir, codegenDir, generateCommonJSApi, opts) {
  const apiJS = (0, import_component_api.componentApiJs)();
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiJS,
    "typescript",
    import_path.default.join(codegenDir, "api.js"),
    opts
  );
  const apiDTSPath = import_path.default.join(codegenDir, "api.d.ts");
  const apiStubDTS = (0, import_component_api.componentApiStubDTS)();
  if (!ctx.fs.exists(apiDTSPath)) {
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiStubDTS,
      "typescript",
      apiDTSPath,
      opts
    );
  }
  const writtenFiles = ["api.js", "api.d.ts"];
  if (generateCommonJSApi && isRoot) {
    const apiCjsJS = (0, import_component_api.rootComponentApiCJS)();
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiCjsJS,
      "typescript",
      import_path.default.join(codegenDir, "api_cjs.cjs"),
      opts
    );
    const cjsStubPath = import_path.default.join(codegenDir, "api_cjs.d.cts");
    if (!ctx.fs.exists(cjsStubPath)) {
      await writeFormattedFile(
        ctx,
        tmpDir,
        apiStubDTS,
        "typescript",
        cjsStubPath,
        opts
      );
    }
    writtenFiles.push("api_cjs.cjs", "api_cjs.d.cts");
  }
  return writtenFiles;
}
async function doApiCodegen(ctx, tmpDir, functionsDir2, codegenDir, generateCommonJSApi, opts) {
  const absModulePaths = await (0, import_bundler.entryPoints)(ctx, functionsDir2);
  const modulePaths = absModulePaths.map((p) => import_path.default.relative(functionsDir2, p));
  const apiContent = (0, import_api.apiCodegen)(modulePaths);
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiContent.JS,
    "typescript",
    import_path.default.join(codegenDir, "api.js"),
    opts
  );
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiContent.DTS,
    "typescript",
    import_path.default.join(codegenDir, "api.d.ts"),
    opts
  );
  const writtenFiles = ["api.js", "api.d.ts"];
  if (generateCommonJSApi) {
    const apiCjsContent = (0, import_api_cjs.apiCjsCodegen)(modulePaths);
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiCjsContent.JS,
      "typescript",
      import_path.default.join(codegenDir, "api_cjs.cjs"),
      opts
    );
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiCjsContent.DTS,
      "typescript",
      import_path.default.join(codegenDir, "api_cjs.d.cts"),
      opts
    );
    writtenFiles.push("api_cjs.cjs", "api_cjs.d.cts");
  }
  return writtenFiles;
}
async function writeFormattedFile(ctx, tmpDir, contents, filetype, destination, options) {
  const formattedContents = await import_prettier.default.format(contents, {
    parser: filetype,
    pluginSearchDirs: false
  });
  if (options?.debug) {
    (0, import_log.logOutput)(`# ${import_path.default.resolve(destination)}`);
    (0, import_log.logOutput)(formattedContents);
    return;
  }
  try {
    const existing = ctx.fs.readUtf8File(destination);
    if (existing === formattedContents) {
      return;
    }
  } catch (err) {
    if (err.code !== "ENOENT") {
      throw err;
    }
  }
  if (options?.dryRun) {
    (0, import_log.logOutput)(`Command would write file: ${destination}`);
    return;
  }
  const tmpPath = tmpDir.writeUtf8File(formattedContents);
  ctx.fs.swapTmpFile(tmpPath, destination);
}
//# sourceMappingURL=codegen.js.map
