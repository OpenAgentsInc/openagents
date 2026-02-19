#!/usr/bin/env bun

import { execSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultFixturePath = path.resolve(
  scriptDir,
  "../test/fixtures/openclaw/tool_policy_parity_cases.json",
);

const openclawPath = process.env.OPENCLAW_PATH ?? "/Users/christopherdavid/code/openclaw";
const fixturePath = process.env.FIXTURE_PATH ?? defaultFixturePath;

const openclawModulePath = path.join(openclawPath, "src/agents/tool-policy.ts");
const openclaw = await import(pathToFileURL(openclawModulePath).href);
const openclawPipelineModulePath = path.join(openclawPath, "src/agents/tool-policy-pipeline.ts");
const openclawPipeline = await import(pathToFileURL(openclawPipelineModulePath).href);

const {
  normalizeToolName,
  expandToolGroups,
  buildPluginToolGroups,
  expandPolicyWithPluginGroups,
  stripPluginOnlyAllowlist,
} = openclaw;

const { buildDefaultToolPolicyPipelineSteps, applyToolPolicyPipeline } = openclawPipeline;

const raw = await readFile(fixturePath, "utf8");
const fixture = JSON.parse(raw);

fixture.meta = fixture.meta ?? {};
fixture.meta.upstream = fixture.meta.upstream ?? {};
fixture.meta.upstream.commit = execSync(`git -C ${JSON.stringify(openclawPath)} rev-parse HEAD`)
  .toString()
  .trim();
fixture.meta.upstream.captured_at = new Date().toISOString();
fixture.meta.upstream.capture_command =
  "bun apps/openagents-runtime/scripts/capture_openclaw_tool_policy_parity.mjs";

for (const testCase of fixture.cases ?? []) {
  const input = testCase.input ?? {};

  let output;
  switch (testCase.operation) {
    case "normalize_tool_name": {
      output = normalizeToolName(String(input.name ?? ""));
      break;
    }
    case "expand_tool_groups": {
      output = expandToolGroups(Array.isArray(input.list) ? input.list : []);
      break;
    }
    case "build_plugin_tool_groups": {
      output = buildPluginToolGroups({
        tools: Array.isArray(input.tools) ? input.tools : [],
        toolMeta: (tool) => {
          const pluginId = tool.plugin_id ?? tool.pluginId;
          return typeof pluginId === "string" ? { pluginId } : undefined;
        },
      });
      break;
    }
    case "expand_policy_with_plugin_groups": {
      output = expandPolicyWithPluginGroups(
        input.policy ?? {},
        toOpenClawGroups(input.groups ?? {}),
      );
      break;
    }
    case "strip_plugin_only_allowlist": {
      output = stripPluginOnlyAllowlist(
        input.policy ?? {},
        toOpenClawGroups(input.groups ?? {}),
        new Set(Array.isArray(input.core_tools) ? input.core_tools : []),
      );
      break;
    }
    case "build_default_tool_policy_pipeline_steps": {
      output = buildDefaultToolPolicyPipelineSteps(input ?? {});
      break;
    }
    case "apply_tool_policy_pipeline": {
      const warnings = [];
      const tools = Array.isArray(input.tools) ? input.tools : [];
      const steps = Array.isArray(input.steps) ? input.steps : [];

      const filtered = applyToolPolicyPipeline({
        tools,
        steps,
        toolMeta: (tool) => {
          const pluginId = tool.plugin_id ?? tool.pluginId;
          return typeof pluginId === "string" ? { pluginId } : undefined;
        },
        warn: (message) => {
          warnings.push(String(message));
        },
      });

      output = {
        tools: filtered.map((tool) => tool.name),
        warnings,
      };
      break;
    }
    default: {
      throw new Error(`Unknown operation: ${testCase.operation}`);
    }
  }

  testCase.expected_openclaw = toJsonSafe(output);
}

await writeFile(fixturePath, `${JSON.stringify(fixture, null, 2)}\n`, "utf8");

console.log(
  `Captured ${fixture.cases?.length ?? 0} OpenClaw parity cases from ${openclawModulePath}`,
);
console.log(`Updated fixture: ${fixturePath}`);

function toOpenClawGroups(groups) {
  const all = Array.isArray(groups.all) ? groups.all : [];
  const byPluginObj =
    groups.by_plugin && typeof groups.by_plugin === "object"
      ? groups.by_plugin
      : groups.byPlugin && typeof groups.byPlugin === "object"
        ? groups.byPlugin
        : {};

  return {
    all,
    byPlugin: new Map(Object.entries(byPluginObj)),
  };
}

function toJsonSafe(value) {
  if (value instanceof Map) {
    return Object.fromEntries(
      Array.from(value.entries()).map(([k, v]) => [k, toJsonSafe(v)]),
    );
  }

  if (Array.isArray(value)) {
    return value.map((item) => toJsonSafe(item));
  }

  if (value && typeof value === "object") {
    const result = {};
    for (const [key, item] of Object.entries(value)) {
      if (item !== undefined) {
        result[key] = toJsonSafe(item);
      }
    }
    return result;
  }

  return value;
}
