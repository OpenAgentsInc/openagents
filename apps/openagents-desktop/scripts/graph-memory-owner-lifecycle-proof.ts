import { randomBytes } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
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

try {
  const result = await Effect.runPromise(
    runGraphMemoryOwnerLifecycleProof({
      databasePath: path.join(root, "graph-memory.sqlite"),
      archivePath: path.join(root, "owner-export.graph.json"),
      safeStorage,
      custodyRung: "standalone_proof_process_wrapping_key",
    }),
  );
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
} finally {
  wrappingKey.fill(0);
  rmSync(root, { force: true, recursive: true });
}
