"use strict";
import path from "path";
import prettier from "prettier";
import { withTmpDir } from "../../bundler/fs.js";
import { entryPoints } from "../../bundler/index.js";
import { apiCodegen } from "../codegen_templates/api.js";
import { apiCjsCodegen } from "../codegen_templates/api_cjs.js";
import {
  dynamicDataModelDTS,
  noSchemaDataModelDTS,
  staticDataModelDTS
} from "../codegen_templates/dataModel.js";
import { readmeCodegen } from "../codegen_templates/readme.js";
import { serverCodegen } from "../codegen_templates/server.js";
import { tsconfigCodegen } from "../codegen_templates/tsconfig.js";
import {
  logError,
  logMessage,
  logOutput,
  logVerbose
} from "../../bundler/log.js";
import { typeCheckFunctionsInMode } from "./typecheck.js";
import { configFilepath, readProjectConfig } from "./config.js";
import { recursivelyDelete } from "./fsUtils.js";
import {
  componentServerDTS,
  componentServerJS,
  componentServerStubDTS
} from "../codegen_templates/component_server.js";
import {
  componentApiDTS,
  componentApiJs,
  componentApiStubDTS,
  rootComponentApiCJS
} from "../codegen_templates/component_api.js";
import { functionsDir } from "./utils/utils.js";
export async function doCodegenForNewProject(ctx) {
  const { projectConfig: existingProjectConfig } = await readProjectConfig(ctx);
  const configPath = await configFilepath(ctx);
  const functionsPath = functionsDir(configPath, existingProjectConfig);
  await doInitCodegen(ctx, functionsPath, true);
  await doCodegen(ctx, functionsPath, "disable");
}
export async function doInitCodegen(ctx, functionsDir2, skipIfExists, opts) {
  await prepareForCodegen(ctx, functionsDir2, opts);
  await withTmpDir(async (tmpDir) => {
    await doReadmeCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts);
    await doTsconfigCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts);
  });
}
async function prepareForCodegen(ctx, functionsDir2, opts) {
  const legacyCodegenPath = path.join(functionsDir2, "_generated.ts");
  if (ctx.fs.exists(legacyCodegenPath)) {
    if (opts?.dryRun) {
      logError(
        `Command would delete legacy codegen file: ${legacyCodegenPath}}`
      );
    } else {
      logError(`Deleting legacy codegen file: ${legacyCodegenPath}}`);
      ctx.fs.unlink(legacyCodegenPath);
    }
  }
  const codegenDir = path.join(functionsDir2, "_generated");
  ctx.fs.mkdir(codegenDir, { allowExisting: true, recursive: true });
  return codegenDir;
}
export async function doCodegen(ctx, functionsDir2, typeCheckMode, opts) {
  const { projectConfig } = await readProjectConfig(ctx);
  const codegenDir = await prepareForCodegen(ctx, functionsDir2, opts);
  await withTmpDir(async (tmpDir) => {
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
        recursivelyDelete(ctx, path.join(codegenDir, file.name), opts);
      }
    }
    await typeCheckFunctionsInMode(ctx, typeCheckMode, functionsDir2);
  });
}
export async function doInitialComponentCodegen(ctx, tmpDir, componentDirectory, opts) {
  const { projectConfig } = await readProjectConfig(ctx);
  const isPublishedPackage = componentDirectory.definitionPath.endsWith(".js") && !componentDirectory.isRoot;
  if (isPublishedPackage) {
    if (opts?.verbose) {
      logMessage(
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
      recursivelyDelete(ctx, path.join(codegenDir, file.name), opts);
    }
  }
}
export async function doFinalComponentCodegen(ctx, tmpDir, rootComponent, componentDirectory, startPushResponse, opts) {
  const { projectConfig } = await readProjectConfig(ctx);
  const isPublishedPackage = componentDirectory.definitionPath.endsWith(".js") && !componentDirectory.isRoot;
  if (isPublishedPackage) {
    return;
  }
  const codegenDir = path.join(componentDirectory.path, "_generated");
  ctx.fs.mkdir(codegenDir, { allowExisting: true, recursive: true });
  const hasSchemaFile = schemaFileExists(ctx, componentDirectory.path);
  let dataModelContents;
  if (hasSchemaFile) {
    if (projectConfig.codegen.staticDataModel) {
      dataModelContents = await staticDataModelDTS(
        ctx,
        startPushResponse,
        rootComponent,
        componentDirectory
      );
    } else {
      dataModelContents = dynamicDataModelDTS();
    }
  } else {
    dataModelContents = noSchemaDataModelDTS();
  }
  const dataModelDTSPath = path.join(codegenDir, "dataModel.d.ts");
  await writeFormattedFile(
    ctx,
    tmpDir,
    dataModelContents,
    "typescript",
    dataModelDTSPath,
    opts
  );
  const serverDTSPath = path.join(codegenDir, "server.d.ts");
  const serverContents = await componentServerDTS(componentDirectory);
  await writeFormattedFile(
    ctx,
    tmpDir,
    serverContents,
    "typescript",
    serverDTSPath,
    opts
  );
  const apiDTSPath = path.join(codegenDir, "api.d.ts");
  const apiContents = await componentApiDTS(
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
    const apiCjsDTSPath = path.join(codegenDir, "api_cjs.d.cts");
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
  const readmePath = path.join(functionsDir2, "README.md");
  if (skipIfExists && ctx.fs.exists(readmePath)) {
    logVerbose(`Not overwriting README.md.`);
    return;
  }
  await writeFormattedFile(
    ctx,
    tmpDir,
    readmeCodegen(),
    "markdown",
    readmePath,
    opts
  );
}
async function doTsconfigCodegen(ctx, tmpDir, functionsDir2, skipIfExists, opts) {
  const tsconfigPath = path.join(functionsDir2, "tsconfig.json");
  if (skipIfExists && ctx.fs.exists(tsconfigPath)) {
    logVerbose(`Not overwriting tsconfig.json.`);
    return;
  }
  await writeFormattedFile(
    ctx,
    tmpDir,
    tsconfigCodegen(),
    "json",
    tsconfigPath,
    opts
  );
}
function schemaFileExists(ctx, functionsDir2) {
  let schemaPath = path.join(functionsDir2, "schema.ts");
  let hasSchemaFile = ctx.fs.exists(schemaPath);
  if (!hasSchemaFile) {
    schemaPath = path.join(functionsDir2, "schema.js");
    hasSchemaFile = ctx.fs.exists(schemaPath);
  }
  return hasSchemaFile;
}
async function doDataModelCodegen(ctx, tmpDir, functionsDir2, codegenDir, opts) {
  const hasSchemaFile = schemaFileExists(ctx, functionsDir2);
  const schemaContent = hasSchemaFile ? dynamicDataModelDTS() : noSchemaDataModelDTS();
  await writeFormattedFile(
    ctx,
    tmpDir,
    schemaContent,
    "typescript",
    path.join(codegenDir, "dataModel.d.ts"),
    opts
  );
  return ["dataModel.d.ts"];
}
async function doServerCodegen(ctx, tmpDir, codegenDir, opts) {
  const serverContent = serverCodegen();
  await writeFormattedFile(
    ctx,
    tmpDir,
    serverContent.JS,
    "typescript",
    path.join(codegenDir, "server.js"),
    opts
  );
  await writeFormattedFile(
    ctx,
    tmpDir,
    serverContent.DTS,
    "typescript",
    path.join(codegenDir, "server.d.ts"),
    opts
  );
  return ["server.js", "server.d.ts"];
}
async function doInitialComponentServerCodegen(ctx, isRoot, tmpDir, codegenDir, opts) {
  await writeFormattedFile(
    ctx,
    tmpDir,
    componentServerJS(),
    "typescript",
    path.join(codegenDir, "server.js"),
    opts
  );
  const serverDTSPath = path.join(codegenDir, "server.d.ts");
  if (!ctx.fs.exists(serverDTSPath)) {
    await writeFormattedFile(
      ctx,
      tmpDir,
      componentServerStubDTS(isRoot),
      "typescript",
      path.join(codegenDir, "server.d.ts"),
      opts
    );
  }
  return ["server.js", "server.d.ts"];
}
async function doInitialComponentDataModelCodegen(ctx, tmpDir, componentDirectory, codegenDir, opts) {
  const hasSchemaFile = schemaFileExists(ctx, componentDirectory.path);
  const dataModelContext = hasSchemaFile ? dynamicDataModelDTS() : noSchemaDataModelDTS();
  const dataModelPath = path.join(codegenDir, "dataModel.d.ts");
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
  const apiJS = componentApiJs();
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiJS,
    "typescript",
    path.join(codegenDir, "api.js"),
    opts
  );
  const apiDTSPath = path.join(codegenDir, "api.d.ts");
  const apiStubDTS = componentApiStubDTS();
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
    const apiCjsJS = rootComponentApiCJS();
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiCjsJS,
      "typescript",
      path.join(codegenDir, "api_cjs.cjs"),
      opts
    );
    const cjsStubPath = path.join(codegenDir, "api_cjs.d.cts");
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
  const absModulePaths = await entryPoints(ctx, functionsDir2);
  const modulePaths = absModulePaths.map((p) => path.relative(functionsDir2, p));
  const apiContent = apiCodegen(modulePaths);
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiContent.JS,
    "typescript",
    path.join(codegenDir, "api.js"),
    opts
  );
  await writeFormattedFile(
    ctx,
    tmpDir,
    apiContent.DTS,
    "typescript",
    path.join(codegenDir, "api.d.ts"),
    opts
  );
  const writtenFiles = ["api.js", "api.d.ts"];
  if (generateCommonJSApi) {
    const apiCjsContent = apiCjsCodegen(modulePaths);
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiCjsContent.JS,
      "typescript",
      path.join(codegenDir, "api_cjs.cjs"),
      opts
    );
    await writeFormattedFile(
      ctx,
      tmpDir,
      apiCjsContent.DTS,
      "typescript",
      path.join(codegenDir, "api_cjs.d.cts"),
      opts
    );
    writtenFiles.push("api_cjs.cjs", "api_cjs.d.cts");
  }
  return writtenFiles;
}
async function writeFormattedFile(ctx, tmpDir, contents, filetype, destination, options) {
  const formattedContents = await prettier.format(contents, {
    parser: filetype,
    pluginSearchDirs: false
  });
  if (options?.debug) {
    logOutput(`# ${path.resolve(destination)}`);
    logOutput(formattedContents);
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
    logOutput(`Command would write file: ${destination}`);
    return;
  }
  const tmpPath = tmpDir.writeUtf8File(formattedContents);
  ctx.fs.swapTmpFile(tmpPath, destination);
}
//# sourceMappingURL=codegen.js.map
