
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { getPromiseRegistryGrounding } from "./promise-registry";
import {
  compileSarahSystemPrompt,
  sarahBlueprintEnabled,
} from "./sarah-blueprint";

let instructionsPromise: Promise<string> | null = null;

function agentInstructionsPath() {
  const configured = process.env.SARAH_AGENT_DIR?.trim();
  if (configured) {
    return path.join(configured, "instructions.md");
  }
  return path.join(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../agent/instructions.md",
  );
}

export function getSarahInstructions() {
  const agentPath = agentInstructionsPath();
  instructionsPromise ??= readFile(agentPath, "utf8").then((instructions) =>
    instructions.trim(),
  );

  return instructionsPromise;
}

export async function getSarahRealtimeInstructions(crmContext?: string | null) {
  const [instructions, registryGrounding] = await Promise.all([
    getSarahInstructions(),
    getPromiseRegistryGrounding(),
  ]);

  // KHS-5 (#8604): when the blueprint is armed (SARAH_BLUEPRINT=1) the
  // compiled typed-fact system prompt leads the composition — persona,
  // playbook, and knowledge come from Sarah's Blueprint (versioned,
  // per-fact provenance) instead of only the flat pasted KB. instructions.md
  // stays as the operational tool-protocol layer. Flag-off (default) keeps
  // the current file-based path byte-identical for safe rollout.
  if (sarahBlueprintEnabled()) {
    const compiled = await compileSarahSystemPrompt().catch(() => null);
    if (compiled) {
      return [compiled, instructions, registryGrounding, crmContext]
        .filter(Boolean)
        .join("\n\n");
    }
  }

  return [instructions, registryGrounding, crmContext]
    .filter(Boolean)
    .join("\n\n");
}
