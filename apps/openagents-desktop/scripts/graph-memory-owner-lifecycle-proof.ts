import { randomBytes } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { Effect } from "effect";

import { runGraphMemoryOwnerLifecycleProof } from "../src/desktop-graph-memory-owner-lifecycle.js";
import type { SafeStorageLike } from "../src/desktop-session-vault.js";

const root = mkdtempSync(path.join(tmpdir(), "openagents-graph-memory-owner-proof-"));
const wrappingKey = randomBytes(32);
const safeStorage: SafeStorageLike = {
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => "standalone_proof_process_wrapping_key",
  encryptString: (plaintext) => {
    const bytes = Buffer.from(plaintext, "utf8");
    return Buffer.from(bytes.map((byte, index) => byte ^ wrappingKey[index % wrappingKey.length]!));
  },
  decryptString: (encrypted) => {
    const bytes = Buffer.from(encrypted);
    return Buffer.from(
      bytes.map((byte, index) => byte ^ wrappingKey[index % wrappingKey.length]!),
    ).toString("utf8");
  },
};

const writeAggregate = (value: unknown): void => {
  const encoded = `${JSON.stringify(value, null, 2)}\n`;
  const outputArg = process.argv.find((argument) => argument.startsWith("--output="));
  if (outputArg === undefined) {
    process.stdout.write(encoded);
    return;
  }
  const outputRef = outputArg.slice("--output=".length);
  const allowedPrefix = "apps/openagents-desktop/benchmarks/graph-memory/";
  if (!outputRef.startsWith(allowedPrefix) || !outputRef.endsWith(".json")) {
    throw new Error("The lifecycle output must be a graph-memory benchmark JSON file.");
  }
  const repoRoot = path.resolve(import.meta.dirname, "../../..");
  const outputPath = path.join(repoRoot, outputRef);
  mkdirSync(path.dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, encoded, { mode: 0o644 });
};

try {
  const result = await Effect.runPromise(
    runGraphMemoryOwnerLifecycleProof({
      databasePath: path.join(root, "graph-memory.sqlite"),
      archivePath: path.join(root, "owner-export.graph.json"),
      safeStorage,
      custodyRung: "standalone_proof_process_wrapping_key",
    }),
  );
  writeAggregate(result);
} finally {
  wrappingKey.fill(0);
  rmSync(root, { force: true, recursive: true });
}
