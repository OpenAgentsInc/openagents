/**
 * The harness config that points children at this session's gateway.
 *
 * opencode reads its provider list from a config file named by
 * `OPENCODE_CONFIG`, so lending a session's grant to children means writing one
 * provider entry whose base URL is the loopback gateway. Written per session
 * into a private directory rather than into the reader's own
 * `~/.config/opencode`: the port changes every launch, two sessions must not
 * fight over one file, and nothing here should survive the process that needs
 * it.
 *
 * The file carries no credential. The gateway holds the grant, and the key
 * below exists only because an OpenAI-compatible client refuses to send a
 * request without one.
 */

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { CHILD_PROVIDER } from "./coder-child-gateway.js";

/** The config body, as a value, so a test can read it without a file. */
export function childHarnessConfig(options: {
  readonly baseUrl: string;
  readonly model: string;
}): Record<string, unknown> {
  return {
    $schema: "https://opencode.ai/config.json",
    provider: {
      [CHILD_PROVIDER]: {
        npm: "@ai-sdk/openai-compatible",
        name: "OpenAgents",
        options: {
          baseURL: options.baseUrl,
          // The gateway authenticates with the thread's grant and ignores
          // this, but a client that has no key at all will not send a request.
          apiKey: "openagents-thread-grant",
        },
        models: {
          [options.model]: { name: options.model, tool_call: true },
        },
      },
    },
  };
}

export interface ChildHarnessFile {
  readonly path: string;
  remove(): void;
}

/** Write the config to a private directory and hand back its path. */
export function writeChildHarnessConfig(options: {
  readonly baseUrl: string;
  readonly model: string;
}): ChildHarnessFile {
  const directory = mkdtempSync(join(tmpdir(), "openagents-child-"));
  const path = join(directory, "opencode.json");
  writeFileSync(path, `${JSON.stringify(childHarnessConfig(options), undefined, 2)}\n`, {
    mode: 0o600,
  });
  return {
    path,
    remove: () => {
      rmSync(directory, { recursive: true, force: true });
    },
  };
}
