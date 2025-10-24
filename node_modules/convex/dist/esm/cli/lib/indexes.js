"use strict";
import chalk from "chalk";
import path from "path";
import { bundleSchema } from "../../bundler/index.js";
import {
  changeSpinner,
  logFailure,
  logFinishedStep,
  logError
} from "../../bundler/log.js";
import {
  poll,
  logAndHandleFetchError,
  deploymentFetch,
  deprecationCheckWarning
} from "./utils/utils.js";
import { deploymentDashboardUrlPage } from "./dashboard.js";
export async function pushSchema(ctx, origin, adminKey, schemaDir, dryRun, deploymentName) {
  if (!ctx.fs.exists(path.resolve(schemaDir, "schema.ts")) && !ctx.fs.exists(path.resolve(schemaDir, "schema.js"))) {
    return {};
  }
  const bundles = await bundleSchema(ctx, schemaDir, []);
  changeSpinner("Checking for index or schema changes...");
  let data;
  const fetch = deploymentFetch(ctx, {
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
    deprecationCheckWarning(ctx, res);
    data = await res.json();
  } catch (err) {
    logFailure(`Error: Unable to run schema validation on ${origin}`);
    return await logAndHandleFetchError(ctx, err);
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
  const depFetch = deploymentFetch(ctx, {
    deploymentUrl: origin,
    adminKey
  });
  const fetch = async () => {
    try {
      const resp = await depFetch(path2, { method: "GET" });
      const data2 = await resp.json();
      return data2;
    } catch (err) {
      logFailure(
        `Error: Unable to build indexes and run schema validation on ${origin}`
      );
      return await logAndHandleFetchError(ctx, err);
    }
  };
  const start = Date.now();
  setSchemaProgressSpinner(null, start, deploymentName);
  const data = await poll(fetch, (data2) => {
    setSchemaProgressSpinner(data2, start, deploymentName);
    return data2.indexes.every(
      (index) => index.backfill.state === "done" || index.staged
    ) && data2.schemaState.state !== "pending";
  });
  switch (data.schemaState.state) {
    case "failed":
      logFailure("Schema validation failed");
      logError(chalk.red(`${data.schemaState.error}`));
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
      changeSpinner("Schema validation complete.");
      break;
    case "active":
      break;
  }
  return data.schemaState;
}
function setSchemaProgressSpinner(data, start, deploymentName) {
  if (!data) {
    changeSpinner("Pushing code to your deployment...");
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
          const dashboardUrl = deploymentDashboardUrlPage(
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
  changeSpinner(msg);
}
export function addProgressLinkIfSlow(msg, deploymentName, start) {
  if (Date.now() - start > 1e4) {
    const dashboardUrl = deploymentDashboardUrlPage(
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
    logFinishedStep(
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
    logFinishedStep(
      `${dryRun ? "Would add" : "Added"} table indexes:
${indexDiff}`
    );
  }
  if (addedStaged.length > 0) {
    let indexDiff = "";
    for (const index of addedStaged) {
      const progressLink = deploymentDashboardUrlPage(
        deploymentName,
        `/data?table=${index.table}&showIndexes=true`
      );
      indexDiff += `  [+] ${formatIndex(toDeveloperIndexConfig(index))}, see progress: ${progressLink}
`;
    }
    indexDiff = indexDiff.slice(0, -1);
    logFinishedStep(
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
    logFinishedStep(`${text}:
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
    logFinishedStep(`${text}:
${indexDiff}`);
  }
}
export function toIndexMetadata(index) {
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
export function toDeveloperIndexConfig(index) {
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
export function formatIndex(index) {
  const [tableName, indexName] = index.name.split(".");
  return `${tableName}.${chalk.bold(indexName)} ${chalk.gray(formatIndexFields(index))}`;
}
function formatIndexFields(index) {
  switch (index.type) {
    case "database":
      return "  " + index.fields.map((f) => chalk.underline(f)).join(", ");
    case "search":
      return `${chalk.cyan("(text)")}   ${chalk.underline(index.searchField)}${formatFilterFields(index.filterFields)}`;
    case "vector":
      return `${chalk.cyan("(vector)")}   ${chalk.underline(index.vectorField)} (${index.dimensions} dimensions)${formatFilterFields(index.filterFields)}`;
    default:
      index;
      return "";
  }
}
function formatFilterFields(filterFields) {
  if (filterFields.length === 0) {
    return "";
  }
  return `, filter${filterFields.length === 1 ? "" : "s"} on ${filterFields.map((f) => chalk.underline(f)).join(", ")}`;
}
//# sourceMappingURL=indexes.js.map
