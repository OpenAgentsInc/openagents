
import { readFile } from "node:fs/promises";
import path from "node:path";
import { getPromiseRegistryGrounding } from "./promise-registry";

let instructionsPromise: Promise<string> | null = null;

export function getSarahInstructions() {
  const agentPath = path.join(
    path.dirname(new URL(import.meta.url).pathname),
    "../../agent/instructions.md",
  );
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
