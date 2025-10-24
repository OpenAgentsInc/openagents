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
var indexes_exports = {};
__export(indexes_exports, {
  addProgressLinkIfSlow: () => addProgressLinkIfSlow,
  formatIndex: () => formatIndex,
  pushSchema: () => pushSchema,
  toDeveloperIndexConfig: () => toDeveloperIndexConfig,
  toIndexMetadata: () => toIndexMetadata
});
module.exports = __toCommonJS(indexes_exports);
var import_chalk = __toESM(require("chalk"), 1);
var import_path = __toESM(require("path"), 1);
var import_bundler = require("../../bundler/index.js");
var import_log = require("../../bundler/log.js");
var import_utils = require("./utils/utils.js");
var import_dashboard = require("./dashboard.js");
async function pushSchema(ctx, origin, adminKey, schemaDir, dryRun, deploymentName) {
  if (!ctx.fs.exists(import_path.default.resolve(schemaDir, "schema.ts")) && !ctx.fs.exists(import_path.default.resolve(schemaDir, "schema.js"))) {
    return {};
  }
  const bundles = await (0, import_bundler.bundleSchema)(ctx, schemaDir, []);
  (0, import_log.changeSpinner)("Checking for index or schema changes...");
  let data;
  const fetch = (0, import_utils.deploymentFetch)(ctx, {
    deploymentUrl: origin,
    adminKey
  });
  try {
    const res = await fetch("/api/prepare_schema", {
      method: "POST",
      body: JSON.stringify({
        bundle: bundles[0],
        adminKey,
        dryRun
      })
    });
    (0, import_utils.deprecationCheckWarning)(ctx, res);
    data = await res.json();
  } catch (err) {
    (0, import_log.logFailure)(`Error: Unable to run schema validation on ${origin}`);
    return await (0, import_utils.logAndHandleFetchError)(ctx, err);
  }
  logIndexChanges(data, dryRun, deploymentName);
  const schemaId = data.schemaId;
  const schemaState = await waitForReadySchema(
    ctx,
    origin,
    adminKey,
    schemaId,
    deploymentName
  );
  return { schemaId, schemaState };
}
async function waitForReadySchema(ctx, origin, adminKey, schemaId, deploymentName) {
  const path2 = `api/schema_state/${schemaId}`;
  const depFetch = (0, import_utils.deploymentFetch)(ctx, {
    deploymentUrl: origin,
    adminKey
  });
  const fetch = async () => {
    try {
      const resp = await depFetch(path2, { method: "GET" });
      const data2 = await resp.json();
      return data2;
    } catch (err) {
      (0, import_log.logFailure)(
        `Error: Unable to build indexes and run schema validation on ${origin}`
      );
      return await (0, import_utils.logAndHandleFetchError)(ctx, err);
    }
  };
  const start = Date.now();
  setSchemaProgressSpinner(null, start, deploymentName);
  const data = await (0, import_utils.poll)(fetch, (data2) => {
    setSchemaProgressSpinner(data2, start, deploymentName);
    return data2.indexes.every(
      (index) => index.backfill.state === "done" || index.staged
    ) && data2.schemaState.state !== "pending";
  });
  switch (data.schemaState.state) {
    case "failed":
      (0, import_log.logFailure)("Schema validation failed");
      (0, import_log.logError)(import_chalk.default.red(`${data.schemaState.error}`));
      return await ctx.crash({
        exitCode: 1,
        errorType: {
          "invalid filesystem or db data": data.schemaState.tableName ? {
            tableName: data.schemaState.tableName
          } : null
        },
        printedMessage: null
        // TODO - move logging into here
      });
    case "overwritten":
      return await ctx.crash({
        exitCode: 1,
        errorType: "fatal",
        printedMessage: `Schema was overwritten by another push.`
      });
    case "validated":
      (0, import_log.changeSpinner)("Schema validation complete.");
      break;
    case "active":
      break;
  }
  return data.schemaState;
}
function setSchemaProgressSpinner(data, start, deploymentName) {
  if (!data) {
    (0, import_log.changeSpinner)("Pushing code to your deployment...");
    return;
  }
  const indexesCompleted = data.indexes.filter(
    (index) => index.backfill.state === "done"
  ).length;
  const numIndexes = data.indexes.length;
  const indexesDone = indexesCompleted === numIndexes;
  const schemaDone = data.schemaState.state !== "pending";
  if (indexesDone && schemaDone) {
    return;
  }
  let msg = "Pushing your code to your Convex deployment...";
  if (!indexesDone && !schemaDone) {
    msg = addProgressLinkIfSlow(
      `Backfilling indexes (${indexesCompleted}/${numIndexes} ready) and checking that documents match your schema...`,
      deploymentName,
      start
    );
  } else if (!indexesDone) {
    if (Date.now() - start > 1e4) {
      for (const index of data.indexes) {
        if (index.backfill.state === "in_progress") {
          const dashboardUrl = (0, import_dashboard.deploymentDashboardUrlPage)(
            deploymentName,
            `/data?table=${index.table}&showIndexes=true`
          );
          msg = `Backfilling index ${index.name} (${indexesCompleted}/${numIndexes} ready), see progress: ${dashboardUrl}`;
          break;
        }
      }
    } else {
      msg = `Backfilling indexes (${indexesCompleted}/${numIndexes} ready)...`;
    }
  } else {
    msg = addProgressLinkIfSlow(
      "Checking that documents match your schema...",
      deploymentName,
      start
    );
  }
  (0, import_log.changeSpinner)(msg);
}
function addProgressLinkIfSlow(msg, deploymentName, start) {
  if (Date.now() - start > 1e4) {
    const dashboardUrl = (0, import_dashboard.deploymentDashboardUrlPage)(
      deploymentName,
      `/data?showSchema=true`
    );
    msg = msg.concat(`
See progress here: ${dashboardUrl}`);
  }
  return msg;
}
function logIndexChanges(indexes, dryRun, deploymentName) {
  if (indexes.dropped.length > 0) {
    let indexDiff = "";
    for (const index of indexes.dropped) {
      indexDiff += `  [-] ${formatIndex(toDeveloperIndexConfig(index))}
`;
    }
    indexDiff = indexDiff.slice(0, -1);
    (0, import_log.logFinishedStep)(
      `${dryRun ? "Would delete" : "Deleted"} table indexes:
${indexDiff}`
    );
  }
  const addedStaged = indexes.added.filter((index) => index.staged);
  const addedEnabled = indexes.added.filter((index) => !index.staged);
  if (addedEnabled.length > 0) {
    let indexDiff = "";
    for (const index of addedEnabled) {
      indexDiff += `  [+] ${formatIndex(toDeveloperIndexConfig(index))}
`;
    }
    indexDiff = indexDiff.slice(0, -1);
    (0, import_log.logFinishedStep)(
      `${dryRun ? "Would add" : "Added"} table indexes:
${indexDiff}`
    );
  }
  if (addedStaged.length > 0) {
    let indexDiff = "";
    for (const index of addedStaged) {
      const progressLink = (0, import_dashboard.deploymentDashboardUrlPage)(
        deploymentName,
        `/data?table=${index.table}&showIndexes=true`
      );
      indexDiff += `  [+] ${formatIndex(toDeveloperIndexConfig(index))}, see progress: ${progressLink}
`;
    }
    indexDiff = indexDiff.slice(0, -1);
    (0, import_log.logFinishedStep)(
      `${dryRun ? "Would add" : "Added"} staged table indexes:
${indexDiff}`
    );
  }
  if (indexes.enabled && indexes.enabled.length > 0) {
    let indexDiff = "";
    for (const index of indexes.enabled) {
      indexDiff += `  [*] ${formatIndex(toDeveloperIndexConfig(index))}
`;
    }
    indexDiff = indexDiff.slice(0, -1);
    const text = dryRun ? `These indexes would be enabled` : `These indexes are now enabled`;
    (0, import_log.logFinishedStep)(`${text}:
${indexDiff}`);
  }
  if (indexes.disabled && indexes.disabled.length > 0) {
    let indexDiff = "";
    for (const index of indexes.disabled) {
      indexDiff += `  [*] ${formatIndex(toDeveloperIndexConfig(index))}
`;
    }
    indexDiff = indexDiff.slice(0, -1);
    const text = dryRun ? `These indexes would be staged` : `These indexes are now staged`;
    (0, import_log.logFinishedStep)(`${text}:
${indexDiff}`);
  }
}
function toIndexMetadata(index) {
  function extractFields(index2) {
    if (index2.type === "database") {
      return index2.fields;
    } else if (index2.type === "search") {
      return {
        searchField: index2.searchField,
        filterFields: index2.filterFields
      };
    } else if (index2.type === "vector") {
      return {
        dimensions: index2.dimensions,
        vectorField: index2.vectorField,
        filterFields: index2.filterFields
      };
    } else {
      index2;
      return [];
    }
  }
  const [table, indexName] = index.name.split(".");
  return {
    table,
    name: indexName,
    fields: extractFields(index),
    backfill: {
      state: "done"
    },
    staged: index.staged ?? false
  };
}
function toDeveloperIndexConfig(index) {
  const name = `${index.table}.${index.name}`;
  const commonProps = { name, staged: index.staged };
  const { fields } = index;
  if (Array.isArray(fields)) {
    return {
      ...commonProps,
      type: "database",
      fields
    };
  } else if ("searchField" in fields) {
    return {
      ...commonProps,
      type: "search",
      searchField: fields.searchField,
      filterFields: fields.filterFields
    };
  } else if ("vectorField" in fields) {
    return {
      ...commonProps,
      type: "vector",
      vectorField: fields.vectorField,
      dimensions: fields.dimensions,
      filterFields: fields.filterFields
    };
  } else {
    fields;
    return { ...commonProps, type: "database", fields: [] };
  }
}
function formatIndex(index) {
  const [tableName, indexName] = index.name.split(".");
  return `${tableName}.${import_chalk.default.bold(indexName)} ${import_chalk.default.gray(formatIndexFields(index))}`;
}
function formatIndexFields(index) {
  switch (index.type) {
    case "database":
      return "  " + index.fields.map((f) => import_chalk.default.underline(f)).join(", ");
    case "search":
      return `${import_chalk.default.cyan("(text)")}   ${import_chalk.default.underline(index.searchField)}${formatFilterFields(index.filterFields)}`;
    case "vector":
      return `${import_chalk.default.cyan("(vector)")}   ${import_chalk.default.underline(index.vectorField)} (${index.dimensions} dimensions)${formatFilterFields(index.filterFields)}`;
    default:
      index;
      return "";
  }
}
function formatFilterFields(filterFields) {
  if (filterFields.length === 0) {
    return "";
  }
  return `, filter${filterFields.length === 1 ? "" : "s"} on ${filterFields.map((f) => import_chalk.default.underline(f)).join(", ")}`;
}
//# sourceMappingURL=indexes.js.map
