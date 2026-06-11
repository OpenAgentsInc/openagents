#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = resolve(dirname(scriptPath), "..");
const defaultOpenApiPath = "workers/api/src/openagents-openapi.ts";
const defaultSourceRoot = "workers/api/src";
const defaultAllowlistPath =
  "scripts/public-projection-freshness-allowlist.json";

const timestampPattern = /\b(generatedAt|lastRebuiltAt)\b/;
const stalenessPattern =
  /\b(maxStalenessSeconds|maxStalenessMs|staleness|stalenessContract|freshnessContract|live_at_read|rebuilds?\s+on|rebuildsOn)\b/i;
const issueRefPattern = /^(?:OpenAgentsInc\/openagents#|#)\d+$/;

const listFiles = (dir) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);

    return entry.isDirectory() ? listFiles(path) : [path];
  });

const read = (path) => readFileSync(path, "utf8");
const readJson = (path) => JSON.parse(read(path));

const stripStringQuotes = (value) =>
  value.replace(/^['"`]/, "").replace(/['"`]$/, "");

const endpointConstantsFromSources = (sourceRoot) => {
  const constants = new Map();
  const files = listFiles(sourceRoot).filter((path) => /\.tsx?$/.test(path));
  const constantPattern =
    /export\s+const\s+([A-Za-z0-9_]*Endpoint[A-Za-z0-9_]*)\s*=\s*(['"`]\/api\/public\/[^'"`]+['"`])/g;

  for (const file of files) {
    const text = read(file);

    for (const match of text.matchAll(constantPattern)) {
      constants.set(match[1], stripStringQuotes(match[2]));
    }
  }

  return constants;
};

const findMatchingBrace = (text, openBraceIndex) => {
  let depth = 0;
  let quote = null;
  let escaped = false;

  for (let index = openBraceIndex; index < text.length; index += 1) {
    const char = text[index];

    if (quote !== null) {
      if (escaped) {
        escaped = false;
      } else if (char === "\\") {
        escaped = true;
      } else if (char === quote) {
        quote = null;
      }
      continue;
    }

    if (char === '"' || char === "'" || char === "`") {
      quote = char;
      continue;
    }

    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;

      if (depth === 0) {
        return index;
      }
    }
  }

  return -1;
};

const extractRouteBlocks = (openApiSource, endpointConstants) => {
  const blocks = [];
  const routeKeyPattern =
    /(^|\n)\s*(?:'([^']+)'|"([^"]+)"|\[([A-Za-z0-9_]+)\])\s*:\s*\{/g;
  let match;

  while ((match = routeKeyPattern.exec(openApiSource)) !== null) {
    const rawPath = match[2] ?? match[3] ?? endpointConstants.get(match[4]);

    if (rawPath === undefined) {
      continue;
    }

    const openBraceIndex = match.index + match[0].lastIndexOf("{");
    const closeBraceIndex = findMatchingBrace(openApiSource, openBraceIndex);

    if (closeBraceIndex === -1) {
      continue;
    }

    blocks.push({
      path: rawPath,
      text: openApiSource.slice(openBraceIndex, closeBraceIndex + 1),
    });
    routeKeyPattern.lastIndex = closeBraceIndex + 1;
  }

  return blocks;
};

const firstResponseSchema = (block) => {
  const match = block.match(/#\/components\/schemas\/([A-Za-z0-9_]+)/);

  return match?.[1];
};

const firstOperationId = (block) => {
  const match = block.match(/operationId:\s*['"`]([^'"`]+)['"`]/);

  return match?.[1];
};

const publicOpenApiInventory = (openApiSource, endpointConstants) =>
  extractRouteBlocks(openApiSource, endpointConstants)
    .filter((block) => block.path.startsWith("/api/public/"))
    .map((block) => ({
      operationId: firstOperationId(block.text) ?? "unknownOperation",
      schema: firstResponseSchema(block.text) ?? "UnknownSchema",
      sourceKind: "openapi-public-route",
      surface: `${block.path} -> ${firstResponseSchema(block.text) ?? "UnknownSchema"}`,
    }));

const forumInventory = (openApiSource) => {
  const wantedSchemas = new Set([
    "ForumAgentPublicProfileResponse",
    "ForumBoardIndex",
    "ForumContextActivity",
    "ForumCreatorEarningsResponse",
    "ForumDirectTipResponse",
    "ForumForum",
    "ForumLaunchStatus",
    "ForumPostDetail",
    "ForumPostList",
    "ForumReceiptLookupResponse",
    "ForumSearch",
    "ForumTipLeaderboardsResponse",
    "ForumTopicDetail",
    "ForumTopicList",
    "ForumWorkRequestListResponse",
    "ForumWorkRequestOffersResponse",
    "ForumWorkRequestStatusResponse",
    "OrangeCheckNostrExportResponse",
  ]);

  return extractRouteBlocks(openApiSource, new Map())
    .filter((block) => block.path.startsWith("/api/forum"))
    .map((block) => ({
      operationId: firstOperationId(block.text) ?? "unknownOperation",
      schema: firstResponseSchema(block.text) ?? "UnknownSchema",
      sourceKind: "openapi-forum-route",
      surface: `${block.path} -> ${firstResponseSchema(block.text) ?? "UnknownSchema"}`,
    }))
    .filter((item) => wantedSchemas.has(item.schema));
};

export const buildInventory = ({ openApiSource, sourceRoot } = {}) => {
  const resolvedSourceRoot = sourceRoot ?? resolve(repoRoot, defaultSourceRoot);
  const resolvedOpenApiSource =
    openApiSource ?? read(resolve(repoRoot, defaultOpenApiPath));
  const endpoints = endpointConstantsFromSources(resolvedSourceRoot);

  const unique = new Map();

  for (const item of [
    ...publicOpenApiInventory(resolvedOpenApiSource, endpoints),
    ...forumInventory(resolvedOpenApiSource),
  ]) {
    unique.set(item.surface, item);
  }

  return [...unique.values()].sort((left, right) =>
    left.surface.localeCompare(right.surface),
  );
};

const extractSchemaBody = (text, declarationIndex) => {
  const openParen = text.indexOf("(", declarationIndex);
  const openBrace = text.indexOf("{", declarationIndex);
  const start = openParen === -1 ? openBrace : Math.min(openParen, openBrace);

  if (start === -1) {
    return text.slice(declarationIndex, declarationIndex + 600);
  }

  const close =
    text[start] === "{"
      ? findMatchingBrace(text, start)
      : text.indexOf("\n\n", declarationIndex);

  return text.slice(
    declarationIndex,
    close === -1
      ? declarationIndex + 1600
      : Math.min(close + 1, declarationIndex + 2400),
  );
};

const schemaAliases = (schema) => {
  const aliases = new Set([schema]);

  if (schema.startsWith("Public") && !schema.endsWith("Projection")) {
    aliases.add(`${schema}Projection`);
  }

  if (schema.endsWith("Response")) {
    aliases.add(schema.replace(/Response$/, ""));
  }

  if (schema.endsWith("Receipt")) {
    aliases.add(`${schema}Detail`);
  }

  if (schema === "PublicLaunchDashboard") {
    aliases.add("PublicLaunchDashboardProjection");
  }

  if (schema === "NexusPylonPublicReceipt") {
    aliases.add("NexusPylonPublicReceiptDetail");
  }

  return [...aliases];
};

export const schemaSourcesFromTree = (sourceRoot) => {
  const files = listFiles(sourceRoot).filter((path) => /\.tsx?$/.test(path));
  const sources = new Map();

  for (const file of files) {
    const text = read(file);

    for (const match of text.matchAll(
      /export\s+(?:const|class|type|interface)\s+([A-Za-z0-9_]+)/g,
    )) {
      const name = match[1];
      const existing = sources.get(name) ?? "";
      sources.set(
        name,
        `${existing}\n${file}\n${extractSchemaBody(text, match.index)}`,
      );
    }
  }

  return sources;
};

export const openApiSchemaDescriptions = (openApiSource) => {
  const sources = new Map();

  for (const match of openApiSource.matchAll(
    /([A-Za-z0-9_]+):\s*objectSummary\(\s*(['"`])([\s\S]*?)\2\s*,?\s*\)/g,
  )) {
    sources.set(match[1], match[3]);
  }

  return sources;
};

const sourceForSchema = (schema, sources) =>
  schemaAliases(schema)
    .map((alias) => sources.get(alias))
    .filter(Boolean)
    .join("\n");

export const normalizeAllowlist = (allowlist) => {
  const problems = [];
  const surfaces = new Set();

  for (const [index, entry] of allowlist.entries()) {
    if (typeof entry.surface !== "string" || entry.surface.trim() === "") {
      problems.push(`allowlist[${index}] is missing surface`);
    }

    if (
      typeof entry.issueRef !== "string" ||
      !issueRefPattern.test(entry.issueRef)
    ) {
      problems.push(
        `allowlist[${index}] ${entry.surface ?? "<missing surface>"} must carry an OpenAgents issue ref`,
      );
    }

    if (typeof entry.reason !== "string" || entry.reason.trim().length < 20) {
      problems.push(
        `allowlist[${index}] ${entry.surface ?? "<missing surface>"} must explain the grandfathered surface`,
      );
    }

    if (typeof entry.surface === "string") {
      surfaces.add(entry.surface);
    }
  }

  return { problems, surfaces };
};

export const analyzeInventory = ({ allowlist, inventory, schemaSources }) => {
  const allowlistResult = normalizeAllowlist(allowlist);
  const problems = [...allowlistResult.problems];
  const results = [];

  for (const item of inventory) {
    const source = sourceForSchema(item.schema, schemaSources);
    const hasTimestamp = timestampPattern.test(source);
    const hasStaleness = stalenessPattern.test(source);
    const allowlisted = allowlistResult.surfaces.has(item.surface);
    const missing = [
      ...(hasTimestamp ? [] : ["generatedAt/lastRebuiltAt"]),
      ...(hasStaleness ? [] : ["maxStalenessSeconds/staleness contract"]),
    ];

    results.push({
      ...item,
      allowlisted,
      hasStaleness,
      hasTimestamp,
      missing,
    });

    if (missing.length > 0 && !allowlisted) {
      problems.push(
        `${item.surface} is missing ${missing.join(" and ")}; add fields or allowlist it with an issue ref`,
      );
    }
  }

  const inventorySurfaces = new Set(inventory.map((item) => item.surface));
  for (const surface of allowlistResult.surfaces) {
    if (!inventorySurfaces.has(surface)) {
      problems.push(`allowlist entry is stale or misspelled: ${surface}`);
    }
  }

  return { problems, results };
};

export const loadAnalysis = ({
  allowlistPath = resolve(repoRoot, defaultAllowlistPath),
  openApiPath = resolve(repoRoot, defaultOpenApiPath),
  sourceRoot = resolve(repoRoot, defaultSourceRoot),
} = {}) => {
  const openApiSource = read(openApiPath);
  const schemaSources = new Map([
    ...schemaSourcesFromTree(sourceRoot),
    ...openApiSchemaDescriptions(openApiSource),
  ]);

  return analyzeInventory({
    allowlist: existsSync(allowlistPath) ? readJson(allowlistPath) : [],
    inventory: buildInventory({ openApiSource, sourceRoot }),
    schemaSources,
  });
};

const main = () => {
  const { problems, results } = loadAnalysis();

  if (problems.length > 0) {
    console.error("Public projection freshness check failed:");
    for (const problem of problems) {
      console.error(`- ${problem}`);
    }
    process.exit(1);
  }

  const compliantCount = results.filter(
    (result) => result.hasTimestamp && result.hasStaleness,
  ).length;
  const allowlistedCount = results.filter(
    (result) => result.allowlisted,
  ).length;

  console.log(
    `Public projection freshness checked: ${results.length} surfaces, ${compliantCount} compliant, ${allowlistedCount} grandfathered.`,
  );
};

if (process.argv[1] === scriptPath) {
  main();
}
