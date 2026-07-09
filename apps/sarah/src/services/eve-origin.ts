
import { readFile } from "node:fs/promises";
import path from "node:path";

type EveDevServerState = {
  origin?: string;
};

export async function getEveOrigin(fallbackOrigin: string) {
  if (process.env.EVE_HOST) return process.env.EVE_HOST;

  try {
    const state = JSON.parse(
      await readFile(
        path.join(process.cwd(), ".eve", "next-dev-server.json"),
        "utf8",
      ),
    ) as EveDevServerState;

    if (state.origin) return state.origin;
  } catch {
    // The file only exists while the Eve dev sidecar is running.
  }

  return fallbackOrigin;
}
