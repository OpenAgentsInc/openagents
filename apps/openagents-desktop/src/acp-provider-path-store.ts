/** Main-only persisted alternate executable candidates. Paths never cross support export. */
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname, isAbsolute } from "node:path";

import type { AcpProviderId } from "./acp-provider-contract.ts";

type Document = Readonly<{
  schema: "openagents.desktop.acp-paths.v1";
  paths: Partial<Record<AcpProviderId, string>>;
}>;

export const openAcpProviderPathStore = (filePath: string) => {
  let document: Document = { schema: "openagents.desktop.acp-paths.v1", paths: {} };
  const load = async (): Promise<void> => {
    try {
      const raw = JSON.parse(await readFile(filePath, "utf8")) as unknown;
      const value = raw as { schema?: unknown; paths?: Record<string, unknown> };
      if (
        value.schema !== "openagents.desktop.acp-paths.v1" ||
        typeof value.paths !== "object" ||
        value.paths === null
      )
        return;
      const paths: Partial<Record<AcpProviderId, string>> = {};
      for (const provider of ["grok", "cursor"] as const) {
        const candidate = value.paths[provider];
        if (typeof candidate === "string" && candidate.length <= 1_024 && isAbsolute(candidate))
          paths[provider] = candidate;
      }
      document = { schema: "openagents.desktop.acp-paths.v1", paths };
    } catch {
      document = { schema: "openagents.desktop.acp-paths.v1", paths: {} };
    }
  };
  const save = async (provider: AcpProviderId, candidate: string): Promise<void> => {
    if (!isAbsolute(candidate) || candidate.length > 1_024)
      throw new TypeError("invalid alternate executable path");
    document = { ...document, paths: { ...document.paths, [provider]: candidate } };
    await mkdir(dirname(filePath), { recursive: true, mode: 0o700 });
    const temporary = `${filePath}.tmp`;
    await writeFile(temporary, `${JSON.stringify(document)}\n`, { mode: 0o600 });
    await rename(temporary, filePath);
  };
  const clear = async (provider: AcpProviderId): Promise<void> => {
    const { [provider]: _removed, ...paths } = document.paths;
    document = { ...document, paths };
    if (Object.keys(paths).length === 0) {
      await rm(filePath, { force: true });
      return;
    }
    const temporary = `${filePath}.tmp`;
    await writeFile(temporary, `${JSON.stringify(document)}\n`, { mode: 0o600 });
    await rename(temporary, filePath);
  };
  return Object.freeze({
    load,
    get: (provider: AcpProviderId) => document.paths[provider],
    save,
    clear,
  });
};
