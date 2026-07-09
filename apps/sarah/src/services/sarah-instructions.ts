
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { getPromiseRegistryGrounding } from "./promise-registry";

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

  return [instructions, registryGrounding, crmContext]
    .filter(Boolean)
    .join("\n\n");
}
